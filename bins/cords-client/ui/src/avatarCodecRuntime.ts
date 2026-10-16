import { AvatarJxlEncoder, type AvatarJxlEncoderModule } from './avatarJxlEncoder.ts';
import { AvatarJxlReader, type AvatarJxlInfo } from './avatarJxlReader.ts';
import { AvatarFrameBudget, avatarLimits, exactJxlTiming, validateCanvas } from './avatarPolicy.ts';

export interface SourceFrame {
  width: number;
  height: number;
  bits: number;
  numerator: number;
  denominator: number;
  crop_resize(
    x: number,
    y: number,
    width: number,
    height: number,
    outWidth: number,
    outHeight: number,
  ): void;
  take_pixels(): Uint8Array;
  free(): void;
}
export interface SourceReader {
  width: number;
  height: number;
  bits: number;
  animated: boolean;
  loops: number;
  next_frame(): SourceFrame | undefined;
  free(): void;
}
export interface AvatarCodecs {
  source(input: Uint8Array): SourceReader;
  jxl(ceiling: number): Promise<AvatarJxlEncoderModule>;
}
export type AvatarGeometry = Readonly<{
  x: number;
  y: number;
  width: number;
  height: number;
  outWidth: number;
  outHeight: number;
}>;
export type AvatarMetadata = AvatarJxlInfo & { durations: number[] };
type ScanFrame = { pixels?: Uint8Array; duration: number; last: boolean };

/** One operation owns one source and one encoder, never a decoded animation.
 * The supervisor reserves both heap maxima, 64 MiB runtime, retained source and
 * requested output before sending a command. This loop bounds its own transient
 * host allocations to 48 MiB (inside that runtime reservation); every large host
 * allocation is reserved before construction/copy/hash, independently of WASM.
 */
export class AvatarCodecRuntime {
  private input = new Uint8Array();
  private jxlInput = false;
  private metadata?: AvatarMetadata;
  private decoderModule?: AvatarJxlEncoderModule;
  private encoderModule?: AvatarJxlEncoderModule;
  private scratch = 0;
  private scratchPeak = 0;

  constructor(
    private readonly codecs: AvatarCodecs,
    private readonly phase: (value: 'decode' | 'process') => void,
  ) {}

  get peakHostBytes(): number {
    return this.scratchPeak;
  }

  async command(operation: string, value: unknown, capacity: number, input?: ArrayBuffer) {
    if (!Number.isSafeInteger(capacity) || capacity < 0 || capacity > avatarLimits.compressedBytes)
      throw new Error('Invalid codec output capacity');
    if (operation === 'open') {
      if (this.input.length || !input?.byteLength || input.byteLength > avatarLimits.inputBytes)
        throw new Error('Invalid or repeated codec input');
      this.input = new Uint8Array(input);
      this.jxlInput =
        (this.input[0] === 0xff && this.input[1] === 0x0a) ||
        (this.input[4] === 0x4a &&
          this.input[5] === 0x58 &&
          this.input[6] === 0x4c &&
          this.input[7] === 0x20);
      if (this.jxlInput) this.decoderModule = await this.codecs.jxl(128 * 1024 * 1024);
      this.metadata = await this.scan();
      return { value: this.metadata };
    }
    const metadata = this.metadata;
    if (!metadata) throw new Error('Open a profile picture first');
    if (operation === 'metadata') return { value: metadata };
    const geometry = this.geometry(value, metadata);
    if (operation === 'crop') return { value: geometry };
    if (operation === 'preview') {
      const bytes = geometry.outWidth * geometry.outHeight * 4;
      if (bytes > capacity) throw new Error('Preview exceeds the reserved response');
      const output = new Uint8Array(bytes);
      await this.frames(geometry, async (frame) => {
        const pixels = frame.pixels!;
        if (metadata.bits === 8) output.set(pixels);
        else for (let i = 0; i < output.length; i++) output[i] = pixels[i * 2 + 1];
        return false;
      });
      return {
        value: { width: geometry.outWidth, height: geometry.outHeight },
        buffer: output.buffer,
      };
    }
    if (operation !== 'encode') throw new Error('Unknown profile-picture command');
    if (!capacity) throw new Error('Reserve encoded output before encoding');
    this.encoderModule ??= await this.codecs.jxl(256 * 1024 * 1024);
    const output = new Uint8Array(capacity);
    const info = { ...metadata, width: geometry.outWidth, height: geometry.outHeight };
    const encoder = new AvatarJxlEncoder(this.encoderModule, info, output);
    const hashes: string[] = [];
    try {
      await this.frames(geometry, async (frame, index) => {
        hashes.push(await this.digest(frame.pixels!));
        encoder.push(frame.pixels!, metadata.durations[index], frame.last);
      });
      const length = encoder.finish().byteLength;
      encoder.dispose();
      // Verify the actual emitted bytes using the same module only after encoder
      // disposal. No third default 256 MiB instance, no retained decoded frames.
      let reader: AvatarJxlReader | undefined;
      try {
        reader = this.decode(
          () =>
            new AvatarJxlReader(
              this.encoderModule!,
              output.subarray(0, length),
              256 * 1024 * 1024,
              true,
            ),
        );
        for (let index = 0; index < hashes.length; index++) {
          const frame = this.decode(() => reader!.next());
          if (
            !frame ||
            frame.duration !== metadata.durations[index] ||
            frame.last !== (index === hashes.length - 1) ||
            (await this.digest(frame.pixels)) !== hashes[index]
          )
            throw new Error('Lossless JPEG XL frame verification failed');
        }
        if (
          this.decode(() => reader!.next()) !== undefined ||
          reader.info.loops !== metadata.loops ||
          reader.info.bits !== metadata.bits ||
          reader.info.ticksPerSecondNumerator !== metadata.ticksPerSecondNumerator ||
          reader.info.ticksPerSecondDenominator !== metadata.ticksPerSecondDenominator
        )
          throw new Error('JPEG XL timing verification failed');
      } finally {
        reader?.dispose();
      }
      // Transfer the already reserved backing buffer. Its length lives in metadata;
      // a 5 MiB slice/copy is unnecessary and would require another reservation.
      return { value: { ...info, length, peakHostBytes: this.scratchPeak }, buffer: output.buffer };
    } finally {
      encoder.dispose();
    }
  }

  private decode<T>(action: () => T): T {
    this.phase('decode');
    try {
      return action();
    } finally {
      this.phase('process');
    }
  }

  private reserve(bytes: number): () => void {
    if (!Number.isSafeInteger(bytes) || bytes < 0 || this.scratch + bytes > 48 * 1024 * 1024)
      throw new Error('Profile-picture host buffers exceed their reserved memory');
    this.scratch += bytes;
    this.scratchPeak = Math.max(this.scratchPeak, this.scratch);
    return () => {
      this.scratch -= bytes;
    };
  }

  private async digest(pixels: Uint8Array): Promise<string> {
    // WebCrypto may copy the input while hashing; reserve that host allocation.
    const release = this.reserve(pixels.byteLength);
    try {
      const hash = await crypto.subtle.digest('SHA-256', pixels as Uint8Array<ArrayBuffer>);
      return Array.from(new Uint8Array(hash), (byte) => byte.toString(16).padStart(2, '0')).join(
        '',
      );
    } finally {
      release();
    }
  }

  private geometry(value: unknown, info: AvatarJxlInfo): AvatarGeometry {
    if (!value || typeof value !== 'object') throw new Error('Missing crop geometry');
    const g = value as AvatarGeometry;
    for (const key of ['x', 'y', 'width', 'height', 'outWidth', 'outHeight'] as const)
      if (!Number.isSafeInteger(g[key]) || g[key] < 0) throw new Error('Invalid crop geometry');
    if (
      !g.width ||
      !g.height ||
      !g.outWidth ||
      !g.outHeight ||
      g.x + g.width > info.width ||
      g.y + g.height > info.height ||
      g.outWidth > Math.min(1024, g.width) ||
      g.outHeight > Math.min(1024, g.height)
    )
      throw new Error('Crop exceeds image/output bounds');
    return {
      x: g.x,
      y: g.y,
      width: g.width,
      height: g.height,
      outWidth: g.outWidth,
      outHeight: g.outHeight,
    };
  }

  private async scan(): Promise<AvatarMetadata> {
    if (this.jxlInput) {
      const reader = this.decode(
        () => new AvatarJxlReader(this.decoderModule!, this.input, 128 * 1024 * 1024),
      );
      try {
        const durations: number[] = [];
        for (;;) {
          const frame = this.decode(() => reader.next());
          if (!frame) break;
          durations.push(frame.duration);
        }
        return { ...reader.info, durations };
      } finally {
        reader.dispose();
      }
    }
    const reader = this.decode(() => this.codecs.source(this.input));
    try {
      validateCanvas(reader.width, reader.height);
      const budget = new AvatarFrameBudget();
      const delays: { numerator: bigint; denominator: bigint }[] = [];
      for (;;) {
        const frame = this.decode(() => reader.next_frame());
        if (!frame) break;
        try {
          budget.add(frame.width, frame.height);
          delays.push({
            numerator: BigInt(frame.numerator),
            denominator: BigInt(frame.denominator),
          });
        } finally {
          frame.free();
        }
      }
      const timing = exactJxlTiming(delays);
      if (reader.bits !== 8 && reader.bits !== 16) throw new Error('Unsupported source precision');
      return {
        width: reader.width,
        height: reader.height,
        bits: reader.bits,
        animated: reader.animated,
        loops: reader.animated ? reader.loops : 1,
        ...timing,
        durations: reader.animated ? [...timing.durations] : [0],
      };
    } finally {
      reader.free();
    }
  }

  private async frames(
    geometry: AvatarGeometry,
    consume: (frame: ScanFrame, index: number) => Promise<boolean | void>,
  ) {
    const metadata = this.metadata!;
    const bytes = geometry.outWidth * geometry.outHeight * 4 * (metadata.bits / 8);
    // Both the cropped host frame and WebCrypto's copy are accounted separately.
    const release = this.reserve(bytes);
    let source: SourceReader | undefined;
    let jxl: AvatarJxlReader | undefined;
    try {
      if (this.jxlInput)
        jxl = this.decode(
          () => new AvatarJxlReader(this.decoderModule!, this.input, 128 * 1024 * 1024),
        );
      else source = this.decode(() => this.codecs.source(this.input));
      for (let index = 0; index < metadata.durations.length; index++) {
        if (source) {
          const frame = this.decode(() => source!.next_frame());
          if (!frame) throw new Error('Source frame disappeared');
          try {
            frame.crop_resize(
              geometry.x,
              geometry.y,
              geometry.width,
              geometry.height,
              geometry.outWidth,
              geometry.outHeight,
            );
            const pixels = frame.take_pixels();
            if (pixels.byteLength !== bytes) throw new Error('Invalid processed frame size');
            if (
              (await consume(
                {
                  pixels,
                  duration: metadata.durations[index],
                  last: index === metadata.durations.length - 1,
                },
                index,
              )) === false
            )
              return;
          } finally {
            frame.free();
          }
        } else {
          const frame = this.decode(() => jxl!.next());
          if (!frame) throw new Error('JXL frame disappeared');
          const pixels = new Uint8Array(bytes);
          const stride = 4 * (metadata.bits / 8);
          for (let y = 0; y < geometry.outHeight; y++)
            for (let x = 0; x < geometry.outWidth; x++) {
              const sx =
                geometry.x + Math.floor(((2 * x + 1) * geometry.width) / (2 * geometry.outWidth));
              const sy =
                geometry.y + Math.floor(((2 * y + 1) * geometry.height) / (2 * geometry.outHeight));
              const offset = (sy * metadata.width + sx) * stride;
              pixels.set(
                frame.pixels.subarray(offset, offset + stride),
                (y * geometry.outWidth + x) * stride,
              );
            }
          if (
            (await consume({ pixels, duration: frame.duration, last: frame.last }, index)) === false
          )
            return;
        }
      }
      if (this.decode(() => (source ? source.next_frame() : jxl!.next())) !== undefined)
        throw new Error('Unexpected additional source frame');
    } finally {
      source?.free();
      jxl?.dispose();
      release();
    }
  }
}
