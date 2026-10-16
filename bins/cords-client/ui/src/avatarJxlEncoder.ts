import { AvatarFrameBudget, avatarLimits, validateCanvas } from './avatarPolicy.ts';
import type { AvatarJxlInfo, AvatarJxlModule } from './avatarJxlReader.ts';

export interface AvatarJxlEncoderModule extends AvatarJxlModule {
  _cords_encode_begin(
    width: number,
    height: number,
    bits: number,
    animated: number,
    numerator: number,
    denominator: number,
    loops: number,
  ): number;
  _cords_encode_frame(pointer: number, bytes: number, duration: number, last: number): number;
  _cords_encode_read(pointer: number, capacity: number): number;
  _cords_encode_status(): number;
}

/** Lossless streaming encoder for an exclusively owned, already reserved module.
 * The supervisor owns cancellation and the single processing deadline. Output is
 * caller-allocated/reserved; only one canvas and a 64 KiB drain buffer live here,
 * both inside the reserved WASM heap. No JS frame/chunk collections or resizing.
 */
export class AvatarJxlEncoder {
  private inputPointer = 0;
  private outputPointer = 0;
  private readonly frameBudget = new AvatarFrameBudget();
  private readonly pixelBytes: number;
  private readonly info: AvatarJxlInfo;
  private written = 0;
  private ticks = 0n;
  private finished = false;
  private disposed = false;
  private readonly chunkBytes = 65_536;

  constructor(
    private readonly module: AvatarJxlEncoderModule,
    info: AvatarJxlInfo,
    private readonly output: Uint8Array,
    private readonly memoryCeiling = 256 * 1024 * 1024,
  ) {
    validateCanvas(info.width, info.height);
    if (info.width > 1024 || info.height > 1024 || (info.bits !== 8 && info.bits !== 16))
      throw new Error('JXL output must be at most 1024 pixels with 8- or 16-bit samples');
    for (const value of [info.loops, info.ticksPerSecondNumerator, info.ticksPerSecondDenominator])
      if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff)
        throw new Error('Invalid JXL animation parameters');
    if (!info.ticksPerSecondNumerator || !info.ticksPerSecondDenominator)
      throw new Error('Invalid JXL animation timebase');
    if (!output.byteLength || output.byteLength > avatarLimits.compressedBytes)
      throw new Error('Reserve between 1 and 5,000,000 bytes for encoded JXL output');
    if (
      !Number.isSafeInteger(memoryCeiling) ||
      memoryCeiling <= 0 ||
      memoryCeiling > 256 * 1024 * 1024
    )
      throw new Error('Invalid JXL encoder memory ceiling');
    this.info = Object.freeze({ ...info });
    this.pixelBytes = info.width * info.height * 4 * (info.bits / 8);
    try {
      this.inputPointer = this.allocate(this.pixelBytes);
      this.outputPointer = this.allocate(this.chunkBytes);
      this.check(
        module._cords_encode_begin(
          info.width,
          info.height,
          info.bits,
          +info.animated,
          info.ticksPerSecondNumerator,
          info.ticksPerSecondDenominator,
          info.loops,
        ),
      );
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  /** Pixels must have the same already-applied geometry for every frame. */
  push(pixels: Uint8Array, duration: number, last: boolean): void {
    if (this.disposed || this.finished) throw new Error('JXL encoder is closed');
    try {
      if (
        pixels.byteLength !== this.pixelBytes ||
        !Number.isSafeInteger(duration) ||
        duration < 0 ||
        duration > 0xffff_ffff
      )
        throw new Error('Invalid JXL frame size or duration');
      if (!this.info.animated && (duration !== 0 || !last))
        throw new Error('A still JXL image requires one final zero-duration frame');
      this.frameBudget.add(this.info.width, this.info.height);
      if (this.info.animated) {
        this.ticks += BigInt(duration);
        if (
          this.ticks * BigInt(this.info.ticksPerSecondDenominator) >
          BigInt(avatarLimits.cycleSeconds) * BigInt(this.info.ticksPerSecondNumerator)
        )
          throw new Error('JXL animation cycle exceeds 120 seconds');
      }
      this.module.HEAPU8.set(pixels, this.inputPointer);
      this.check(
        this.module._cords_encode_frame(this.inputPointer, this.pixelBytes, duration, +last),
      );
      let status: number;
      do {
        const count = this.check(
          this.module._cords_encode_read(this.outputPointer, this.chunkBytes),
        );
        if (count > this.chunkBytes || count > this.output.byteLength - this.written)
          throw new Error(
            'Encoded JXL exceeds the reserved output limit; review a smaller lossless resize',
          );
        this.output.set(
          this.module.HEAPU8.subarray(this.outputPointer, this.outputPointer + count),
          this.written,
        );
        this.written += count;
        status = this.module._cords_encode_status();
        if (status !== 0 && status !== 2) throw new Error('Unexpected JXL encoder status');
        if (status === 2 && !count) throw new Error('JXL encoder made no output progress');
      } while (status === 2);
      this.finished = last;
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  /** Borrowed output, valid only after the final frame. No concatenation copy. */
  finish(): Uint8Array {
    if (this.disposed || !this.finished || !this.written)
      throw new Error('JXL encoding did not finish successfully');
    return this.output.subarray(0, this.written);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.module._cords_reset();
    if (this.outputPointer) this.module._cords_free(this.outputPointer);
    if (this.inputPointer) this.module._cords_free(this.inputPointer);
    this.inputPointer = this.outputPointer = 0;
  }

  private allocate(bytes: number): number {
    this.check(0);
    const pointer = this.module._cords_alloc(bytes);
    try {
      this.check(pointer);
      if (!pointer) throw new Error('JXL encoder allocation budget exceeded');
    } catch (error) {
      if (pointer) this.module._cords_free(pointer);
      throw error;
    }
    return pointer;
  }

  private check(result: number): number {
    if (this.module.HEAPU8.byteLength > this.memoryCeiling)
      throw new Error('JXL module exceeded its reserved memory ceiling');
    if (!Number.isSafeInteger(result)) throw new Error('Invalid JXL encoder result');
    if (result < 0) {
      const start = this.module._cords_error();
      const heap = this.module.HEAPU8;
      let end = start;
      while (end < heap.length && end - start < 4096 && heap[end]) end++;
      throw new Error(new TextDecoder().decode(heap.subarray(start, end)) || 'JXL encoding failed');
    }
    return result;
  }
}
