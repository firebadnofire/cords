import type { AvatarCodecs, SourceReader } from './avatarCodecRuntime';
import type { AvatarJxlEncoderModule } from './avatarJxlEncoder';

type SourceBindings = {
  default(options: { module_or_path: URL }): Promise<unknown>;
  SourceDecoder: new (input: Uint8Array) => SourceReader;
};
type JxlBindings = {
  default(options: {
    locateFile(path: string): string;
    wasmMemory: WebAssembly.Memory;
  }): Promise<AvatarJxlEncoderModule>;
};

export async function loadJxlCodec(ceiling: number): Promise<AvatarJxlEncoderModule> {
  const base = new URL('/avatar-codec/', globalThis.location.href);
  const jxl = (await import(/* @vite-ignore */ new URL('cords-jxl.mjs', base).href)) as JxlBindings;
  return jxl.default({
    locateFile: () => new URL('cords-jxl.wasm', base).href,
    wasmMemory: new WebAssembly.Memory({ initial: 512, maximum: ceiling / 65536 }),
  });
}

/** Only build-verified assets bundled with the application. No server-selected URL. */
export async function loadAvatarCodecs(): Promise<AvatarCodecs> {
  const base = new URL('/avatar-codec/', globalThis.location.href);
  const sourceUrl = new URL('source/cords_avatar_source.js', base).href;
  const jxlUrl = new URL('cords-jxl.mjs', base).href;
  const source = (await import(/* @vite-ignore */ sourceUrl)) as SourceBindings;
  await source.default({ module_or_path: new URL('source/cords_avatar_source_bg.wasm', base) });
  const jxl = (await import(/* @vite-ignore */ jxlUrl)) as JxlBindings;
  return {
    source: (input) => new source.SourceDecoder(input),
    jxl: (ceiling) =>
      jxl.default({
        locateFile: () => new URL('cords-jxl.wasm', base).href,
        wasmMemory: new WebAssembly.Memory({ initial: 512, maximum: ceiling / 65536 }),
      }),
  };
}
