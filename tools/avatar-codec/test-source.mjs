// Real source-WASM -> libjxl-WASM round trips. Only metadata and frame hashes
// survive between frames; the fixture inputs themselves are deliberately tiny.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

const directory = resolve(process.argv[2] ?? 'target/avatar-codec');
const fixtures = resolve(process.argv[3] ?? `${directory}/fixtures`);
const source = await import(pathToFileURL(`${directory}/source/cords_avatar_source.js`));
source.initSync({ module: await readFile(`${directory}/source/cords_avatar_source_bg.wasm`) });
const { default: createJxl } = await import(pathToFileURL(`${directory}/cords-jxl.mjs`));
const wasmBinary = await readFile(`${directory}/cords-jxl.wasm`);
const hash = pixels => createHash('sha256').update(pixels).digest('hex');

function check(module, result) {
  const address = module._cords_error();
  let end = address;
  while (module.HEAPU8[end]) end++;
  assert.ok(result >= 0, new TextDecoder().decode(module.HEAPU8.subarray(address, end)));
  return result;
}
function alloc(module, size) {
  const pointer = check(module, module._cords_alloc(size));
  assert.notEqual(pointer, 0, 'WASM allocation failed');
  return pointer;
}
function scan(bytes, tickRate, expected, geometry) {
  const decoder = new source.SourceDecoder(bytes);
  const frames = [];
  try {
    assert.equal(decoder.animated, true);
    assert.equal(decoder.loops, 3);
    for (;;) {
      const frame = decoder.next_frame();
      if (!frame) break;
      try {
        if (geometry) frame.crop_resize(...geometry);
        const ticks = BigInt(frame.numerator) * BigInt(tickRate);
        assert.equal(ticks % BigInt(frame.denominator), 0n, 'fixture timing must be exact');
        const pixels = frame.take_pixels();
        if (expected) assert.deepEqual([...pixels], expected[frames.length]);
        frames.push({ ticks: Number(ticks / BigInt(frame.denominator)), hash: hash(pixels) });
      } finally {
        frame.free();
      }
    }
    return { frames, bits: decoder.bits, width: geometry?.[4] ?? decoder.width,
      height: geometry?.[5] ?? decoder.height };
  } finally {
    decoder.free();
  }
}

async function roundTrip(name, tickRate, durations, expected, geometry) {
  const bytes = new Uint8Array(await readFile(`${fixtures}/${name}`));
  const metadata = scan(bytes, tickRate, expected, geometry);
  assert.deepEqual(metadata.frames.map(frame => frame.ticks), durations);
  const module = await createJxl({ wasmBinary });
  const size = metadata.width * metadata.height * 4 * metadata.bits / 8;
  const input = alloc(module, size);
  const output = alloc(module, 65536);
  const decoder = new source.SourceDecoder(bytes);
  const chunks = [];
  try {
    check(module, module._cords_encode_begin(metadata.width, metadata.height, metadata.bits,
      1, tickRate, 1, decoder.loops));
    for (let index = 0; index < metadata.frames.length; index++) {
      const frame = decoder.next_frame();
      assert.ok(frame);
      try {
        if (geometry) frame.crop_resize(...geometry);
        const pixels = frame.take_pixels();
        assert.equal(hash(pixels), metadata.frames[index].hash);
        module.HEAPU8.set(pixels, input);
        check(module, module._cords_encode_frame(input, size, durations[index],
          +(index === durations.length - 1)));
        do {
          const count = check(module, module._cords_encode_read(output, 65536));
          if (count) chunks.push(module.HEAPU8.slice(output, output + count));
        } while (module._cords_encode_status() === 2);
      } finally {
        frame.free();
      }
    }
    assert.equal(decoder.next_frame(), undefined);
  } finally {
    decoder.free();
    module._cords_reset();
    module._cords_free(input);
    module._cords_free(output);
  }
  const encoded = Buffer.concat(chunks);
  assert.ok(encoded.length <= 5_000_000);
  const compressed = alloc(module, encoded.length);
  const pixels = alloc(module, size);
  module.HEAPU8.set(encoded, compressed);
  let index = 0;
  try {
    check(module, module._cords_decode_begin(compressed, encoded.length, 0));
    for (;;) {
      const status = check(module, module._cords_decode_next());
      if (status === 0) break;
      if (status === 1) {
        assert.deepEqual(Array.from({ length: 7 }, (_, field) => module._cords_info(field)),
          [metadata.width, metadata.height, metadata.bits, 1, tickRate, 1, 3]);
      }
      if (status === 2) assert.equal(module._cords_info(7), durations[index]);
      if (status === 3) check(module, module._cords_decode_pixels(pixels, size));
      if (status === 4) {
        assert.equal(hash(module.HEAPU8.subarray(pixels, pixels + size)), metadata.frames[index].hash);
        index++;
      }
    }
    assert.equal(index, metadata.frames.length);
  } finally {
    module._cords_reset();
    module._cords_free(compressed);
    module._cords_free(pixels);
    assert.equal(module._cords_used(), 0);
  }
}

test('GIF source WASM preserves zero delay, order, disposal and repeat semantics in JXL', () =>
  roundTrip('animated.gif', 100, [0, 1, 3], [
    [255, 0, 0, 255, 0, 0, 255, 255],
    [0, 255, 0, 255, 0, 0, 255, 255],
    [255, 0, 0, 255, 0, 0, 255, 255],
  ]));
test('APNG source WASM preserves rational timing, default image, disposal and blend in JXL', () =>
  roundTrip('animated.png', 420, [0, 7, 60, 0], [
    [255, 0, 0, 255, 0, 0, 255, 255],
    [127, 128, 0, 255, 0, 0, 255, 255],
    [255, 0, 0, 255, 0, 255, 0, 255],
    [255, 0, 0, 255, 0, 0, 0, 0],
  ]));
test('16-bit APNG stays 16-bit through the actual WASM codecs', () =>
  roundTrip('animated16.png', 420, [0, 7, 60, 0]));
test('animated WebP source WASM preserves zero delay, loops, alpha and order in JXL', () =>
  roundTrip('animated.webp', 1000, [0, 17, 33], [0, 100, 200].map(red =>
    [red, 20, 30, 255, 0, 40, 255, 128])));

for (const [name, tickRate, durations] of [
  ['animated.gif', 100, [0, 1, 3]],
  ['animated.png', 420, [0, 7, 60, 0]],
  ['animated16.png', 420, [0, 7, 60, 0]],
  ['animated.webp', 1000, [0, 17, 33]],
]) {
  test(`${name}: every cropped/scaled frame survives lossless WASM encoding`, () =>
    roundTrip(name, tickRate, durations, undefined, [0, 0, 2, 1, 1, 1]));
}
