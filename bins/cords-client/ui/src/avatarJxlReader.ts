import { AvatarFrameBudget, avatarLimits, validateCanvas } from './avatarPolicy.ts';

/** ABI of the pinned, locally built libjxl module. A module belongs to one reader
 * or encoder at a time. Its imported memory must have the caller's hard ceiling. */
export interface AvatarJxlModule {
  HEAPU8: Uint8Array;
  _cords_alloc(bytes: number): number;
  _cords_free(pointer: number): void;
  _cords_error(): number;
  _cords_reset(): void;
  _cords_decode_begin(pointer: number, bytes: number, coalesce: number): number;
  _cords_decode_next(): number;
  _cords_decode_pixels(pointer: number, capacity: number): number;
  _cords_info(field: number): number;
}

export type AvatarJxlInfo = Readonly<{
  width: number;
  height: number;
  bits: 8 | 16;
  animated: boolean;
  ticksPerSecondNumerator: number;
  ticksPerSecondDenominator: number;
  loops: number;
}>;

export type AvatarJxlFrame = Readonly<{
  index: number;
  duration: number;
  last: boolean;
  /** Borrowed WASM view, valid only until the next call or dispose. Never collect
   * these views as animation frames: the decoder reuses this single canvas. */
  pixels: Uint8Array;
}>;

/** Incremental coalesced JXL reader for worker conversion, validation and playback.
 * This does not instantiate WASM or own a watchdog. The external supervisor must
 * reserve the module's whole memory ceiling and terminate a blocked decode.
 * All large buffers below live inside that reserved WASM memory. */
export class AvatarJxlReader {
  readonly info: AvatarJxlInfo;
  private inputPointer = 0;
  private pixelPointer = 0;
  private pixelBytes = 0;
  private frameBudget = new AvatarFrameBudget();
  private count = 0;
  private ticks = 0n;
  private finished = false;
  private disposed = false;
  private last = false;

  constructor(
    private readonly module: AvatarJxlModule,
    input: Uint8Array,
    private readonly memoryCeiling: number,
    incomingAvatar = false,
  ) {
    if (
      !Number.isSafeInteger(memoryCeiling) ||
      memoryCeiling <= 0 ||
      memoryCeiling > 256 * 1024 * 1024
    )
      throw new Error('Invalid JXL decoder memory ceiling');
    if (
      !input.length ||
      input.length > (incomingAvatar ? avatarLimits.compressedBytes : avatarLimits.inputBytes)
    )
      throw new Error('JXL input exceeds the compressed image limit');
    try {
      this.inputPointer = this.allocate(input.length);
      module.HEAPU8.set(input, this.inputPointer);
      this.check(module._cords_decode_begin(this.inputPointer, input.length, 1));
      if (this.check(module._cords_decode_next()) !== 1)
        throw new Error('JXL did not provide image metadata');
      const width = module._cords_info(0);
      const height = module._cords_info(1);
      validateCanvas(width, height);
      if (incomingAvatar && (width > 1024 || height > 1024))
        throw new Error('Embedded JXL profile picture exceeds 1024 pixels');
      const bits = module._cords_info(2);
      if (bits !== 8 && bits !== 16) throw new Error('Unsupported JXL sample precision');
      const animated = module._cords_info(3) !== 0;
      this.info = Object.freeze({
        width,
        height,
        bits,
        animated,
        ticksPerSecondNumerator: animated ? module._cords_info(4) : 1,
        ticksPerSecondDenominator: animated ? module._cords_info(5) : 1,
        loops: animated ? module._cords_info(6) : 1,
      });
      for (const [key, value] of Object.entries(this.info)) {
        if (
          typeof value === 'number' &&
          (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff)
        )
          throw new Error(`Invalid JXL ${key}`);
      }
      if (!this.info.ticksPerSecondNumerator || !this.info.ticksPerSecondDenominator)
        throw new Error('Invalid JXL animation timebase');
      this.pixelBytes = width * height * 4 * (bits / 8);
      this.pixelPointer = this.allocate(this.pixelBytes);
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  next(): AvatarJxlFrame | undefined {
    if (this.disposed) throw new Error('JXL reader is disposed');
    if (this.finished) return undefined;
    let duration: number | undefined;
    let last = false;
    try {
      for (;;) {
        const status = this.check(this.module._cords_decode_next());
        if (status === 0) {
          if (!this.count || !this.last || duration !== undefined)
            throw new Error('JXL ended without a complete final frame');
          this.finished = true;
          return undefined;
        }
        if (status === 2) {
          if (duration !== undefined || this.last) throw new Error('Invalid JXL frame order');
          this.frameBudget.add(this.info.width, this.info.height);
          duration = this.module._cords_info(7);
          last = this.module._cords_info(9) === 1;
          if (!Number.isSafeInteger(duration) || duration < 0 || duration > 0xffff_ffff)
            throw new Error('Invalid JXL frame duration');
          // Zero durations remain zero; playback scheduling is a separate policy.
          if (this.info.animated) {
            this.ticks += BigInt(duration);
            if (
              this.ticks * BigInt(this.info.ticksPerSecondDenominator) >
              BigInt(avatarLimits.cycleSeconds) * BigInt(this.info.ticksPerSecondNumerator)
            )
              throw new Error('JXL animation cycle exceeds 120 seconds');
          }
        } else if (status === 3) {
          if (duration === undefined) throw new Error('JXL requested pixels before a frame header');
          this.check(this.module._cords_decode_pixels(this.pixelPointer, this.pixelBytes));
        } else if (status === 4) {
          if (duration === undefined) throw new Error('JXL returned pixels before a frame header');
          this.last = last;
          return {
            index: this.count++,
            duration,
            last,
            pixels: this.module.HEAPU8.subarray(
              this.pixelPointer,
              this.pixelPointer + this.pixelBytes,
            ),
          };
        } else throw new Error('Unexpected JXL decoder event');
      }
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.module._cords_reset();
    if (this.pixelPointer) this.module._cords_free(this.pixelPointer);
    if (this.inputPointer) this.module._cords_free(this.inputPointer);
    this.pixelPointer = this.inputPointer = 0;
  }

  private allocate(bytes: number): number {
    this.check(0);
    const pointer = this.module._cords_alloc(bytes);
    try {
      this.check(pointer);
    } catch (error) {
      if (pointer) this.module._cords_free(pointer);
      throw error;
    }
    if (!pointer) throw new Error('JXL allocation budget exceeded');
    return pointer;
  }

  private check(result: number): number {
    if (this.module.HEAPU8.byteLength > this.memoryCeiling)
      throw new Error('JXL module exceeded its reserved memory ceiling');
    if (result < 0) {
      const start = this.module._cords_error();
      const heap = this.module.HEAPU8;
      let end = start;
      while (end < heap.length && end - start < 4096 && heap[end]) end++;
      throw new Error(new TextDecoder().decode(heap.subarray(start, end)) || 'JXL decoding failed');
    }
    return result;
  }
}
