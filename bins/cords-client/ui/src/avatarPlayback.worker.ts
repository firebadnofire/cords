import { loadJxlCodec } from './avatarCodecs';
import { AvatarJxlReader } from './avatarJxlReader';

let running = false;
self.onmessage = async (
  event: MessageEvent<{ input: ArrayBuffer; animate: boolean; size: number }>,
) => {
  if (running) return;
  running = true;
  try {
    const { input, animate, size } = event.data;
    if (
      !input.byteLength ||
      input.byteLength > 5_000_000 ||
      !Number.isInteger(size) ||
      size < 1 ||
      size > 256
    )
      throw new Error('Invalid avatar playback request');
    const module = await loadJxlCodec(128 * 1024 * 1024);
    let reader: AvatarJxlReader | undefined;
    let cycles = 0;
    let cycleTicks = 0;
    let nextTime = performance.now();
    const open = () => {
      reader?.dispose();
      reader = new AvatarJxlReader(module, new Uint8Array(input), 128 * 1024 * 1024, true);
    };
    open();
    const render = () => {
      self.postMessage({ phase: 'decode' });
      let frame = reader!.next();
      // Zero-duration compositing steps stay ordered, with no invented delay.
      while (frame && frame.duration === 0 && !frame.last) frame = reader!.next();
      if (!frame) {
        cycles++;
        if (!animate || cycleTicks === 0 || (reader!.info.loops && cycles >= reader!.info.loops)) {
          reader?.dispose();
          self.postMessage({ done: true });
          return;
        }
        open();
        cycleTicks = 0;
        frame = reader!.next();
        while (frame && frame.duration === 0 && !frame.last) frame = reader!.next();
      }
      if (!frame) throw new Error('Missing avatar frame');
      cycleTicks += frame.duration;
      const info = reader!.info;
      const width = Math.max(
        1,
        Math.round(info.width * Math.min(size / info.width, size / info.height, 1)),
      );
      const height = Math.max(1, Math.round((info.height * width) / info.width));
      const output = new Uint8ClampedArray(width * height * 4);
      const stride = info.bits / 8;
      for (let y = 0; y < height; y++)
        for (let x = 0; x < width; x++) {
          const source =
            (Math.min(info.height - 1, Math.floor(((y + 0.5) * info.height) / height)) *
              info.width +
              Math.min(info.width - 1, Math.floor(((x + 0.5) * info.width) / width))) *
            4 *
            stride;
          for (let channel = 0; channel < 4; channel++)
            output[(y * width + x) * 4 + channel] =
              frame.pixels[source + channel * stride + stride - 1];
        }
      self.postMessage(
        { width, height, pixels: output.buffer, phase: 'idle' },
        { transfer: [output.buffer] },
      );
      if (!animate || !info.animated) {
        reader?.dispose();
        self.postMessage({ done: true });
        return;
      }
      nextTime +=
        (frame.duration * info.ticksPerSecondDenominator * 1000) / info.ticksPerSecondNumerator;
      setTimeout(
        () => {
          try {
            render();
          } catch (error) {
            reader?.dispose();
            self.postMessage({ error: String(error) });
          }
        },
        Math.max(0, nextTime - performance.now()),
      );
    };
    render();
  } catch (error) {
    self.postMessage({ error: String(error) });
  }
};
