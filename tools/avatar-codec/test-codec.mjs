import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import { brotliCompressSync } from 'node:zlib';

const directory = resolve(process.argv[2] ?? 'target/avatar-codec');
const { default: createModule } = await import(pathToFileURL(`${directory}/cords-jxl.mjs`));
const wasmBinary = await readFile(`${directory}/cords-jxl.wasm`);
const create = () => createModule({ wasmBinary });
// Shared only by the extended layer-fixture suite, which also runs these tests.
export { create, decode, encode };

test('JXL decoder accepts a hard 128 MiB imported memory beside the encoder reservation', async () => {
  const memory = new WebAssembly.Memory({ initial: 512, maximum: 2048 });
  const module = await createModule({ wasmBinary, wasmMemory: memory });
  assert.equal(module.HEAPU8.buffer, memory.buffer);
  const encoder = await create();
  assert.notEqual(encoder.HEAPU8.buffer, memory.buffer);
  const original = encode(encoder, { ticks: [0, 3, 1], numerator: 7, loops: 2 });
  const decoded = decode(module, original.bytes, true);
  assert.deepEqual(decoded.frames.map(frame => frame.duration), [0, 3, 1]);
  assert.equal(module._cords_alloc(129 * 1024 * 1024), 0);
  assert.equal(module._cords_used(), 0);
  const pages = memory.buffer.byteLength / 65536;
  memory.grow(2048 - pages);
  assert.equal(memory.buffer.byteLength, 128 * 1024 * 1024);
  assert.throws(() => memory.grow(1), RangeError);
});

function message(module) {
  const address = module._cords_error();
  let end = address;
  while (module.HEAPU8[end]) end += 1;
  return new TextDecoder().decode(module.HEAPU8.subarray(address, end));
}
function check(module, result) {
  assert.ok(result >= 0, message(module));
  return result;
}
function allocate(module, length) {
  const pointer = module._cords_alloc(length);
  assert.ok(pointer, message(module));
  return pointer;
}
// The fixture harness retains encoded chunks/expected fixture pixels only.
// It feeds and drains one frame at a time; production workers need host accounting.
function encode(module, { width = 2, height = 1, bits = 8, ticks = [1], numerator = 100,
  denominator = 1, loops = 0, animated = true, pixel = () => new Uint8Array([31, 20, 19, 0, 255, 64, 20, 127]) }) {
  const length = width * height * 4 * (bits / 8);
  const input = allocate(module, length);
  const output = allocate(module, 65536);
  const chunks = [];
  check(module, module._cords_encode_begin(width, height, bits, +animated, numerator, denominator, loops));
  try {
    for (let index = 0; index < ticks.length; index += 1) {
      const pixels = pixel(index);
      assert.equal(pixels.byteLength, length);
      module.HEAPU8.set(pixels, input);
      check(module, module._cords_encode_frame(input, length, ticks[index], +(index === ticks.length - 1)));
      do {
        const count = check(module, module._cords_encode_read(output, 65536));
        if (count) chunks.push(module.HEAPU8.slice(output, output + count));
      } while (module._cords_encode_status() === 2);
    }
    return { bytes: Buffer.concat(chunks), peak: module._cords_peak(), heap: module.HEAPU8.byteLength };
  } finally {
    module._cords_reset();
    module._cords_free(input);
    module._cords_free(output);
  }
}
function decode(module, bytes, coalescing = false) {
  const input = allocate(module, bytes.length);
  module.HEAPU8.set(bytes, input);
  let output = 0;
  let basic;
  const frames = [];
  check(module, module._cords_decode_begin(input, bytes.length, +coalescing));
  try {
    let header;
    for (;;) {
      const status = check(module, module._cords_decode_next());
      if (status === 0) break;
      if (status === 1) {
        basic = Array.from({ length: 7 }, (_, index) => module._cords_info(index));
        output = allocate(module, basic[0] * basic[1] * 4 * (basic[2] / 8));
      }
      if (status === 2) header = { duration: module._cords_info(7), last: module._cords_info(9) };
      if (status === 3) check(module, module._cords_decode_pixels(output, basic[0] * basic[1] * 4 * (basic[2] / 8)));
      if (status === 4) frames.push({ ...header, pixels: module.HEAPU8.slice(output, output + basic[0] * basic[1] * 4 * (basic[2] / 8)) });
    }
    return { basic, frames };
  } finally {
    module._cords_reset();
    if (output) module._cords_free(output);
    module._cords_free(input);
  }
}

test('static lossless RGBA retains transparent RGB, partial alpha and dimensions', async () => {
  const module = await create();
  const pixels = new Uint8Array([31, 20, 19, 0, 255, 64, 20, 127]);
  const encoded = encode(module, { animated: false, ticks: [0], pixel: () => pixels });
  const result = decode(module, encoded.bytes);
  assert.deepEqual(result.basic.slice(0, 4), [2, 1, 8, 0]);
  assert.deepEqual(result.frames[0].pixels, pixels);
  assert.equal(module._cords_used(), 0);
});

test('animation preserves duplicate frames, zero durations, ordering, rational ticks and finite loops', async () => {
  const module = await create();
  const ticks = [0, 1, 1, 0, 2, 0];
  const pixel = index => new Uint8Array([index === 2 ? 1 : index, 50, 80, 255, 22, 33, 44, 0]);
  const encoded = encode(module, { ticks, numerator: 30000, denominator: 1001, loops: 3, pixel });
  const result = decode(module, encoded.bytes);
  assert.deepEqual(result.basic.slice(3), [1, 30000, 1001, 3]);
  assert.deepEqual(result.frames.map(frame => frame.duration), ticks);
  assert.equal(result.frames.length, ticks.length);
  for (let index = 0; index < ticks.length; index += 1) assert.deepEqual(result.frames[index].pixels, pixel(index));
  assert.equal(result.frames.at(-1).last, 1);
  assert.equal(module._cords_used(), 0);
});

test('native coalescing exposes every regular layer including zero-duration frames', async () => {
  const module = await create();
  const ticks = [0, 1, 0, 2, 0];
  const pixel = index => new Uint8Array([index * 30, 50, 80, 255, 22, 33, 44, 127]);
  const { bytes } = encode(module, { ticks, pixel });
  const result = decode(module, bytes, true);
  assert.deepEqual(result.frames.map(frame => frame.duration), ticks);
  result.frames.forEach((frame, index) => assert.deepEqual(frame.pixels, pixel(index)));
});

test('16-bit integer pixels round trip without reducing sample precision', async () => {
  const module = await create();
  const pixels = new Uint8Array(new Uint16Array([65535, 30100, 2010, 12345, 55000, 20000, 30000, 0]).buffer);
  const { bytes } = encode(module, { bits: 16, ticks: [1, 2], loops: 0, pixel: () => pixels });
  const result = decode(module, bytes);
  assert.equal(result.basic[2], 16);
  assert.equal(result.basic[6], 0);
  result.frames.forEach(frame => assert.deepEqual(frame.pixels, pixels));
});

test('incremental encoder memory does not grow with retained composited frames', async () => {
  const module = await create();
  const width = 128, height = 128;
  const pixels = new Uint8Array(width * height * 4).fill(255);
  const first = encode(module, { width, height, ticks: [1, 1], pixel: () => pixels });
  const many = encode(module, { width, height, ticks: Array(250).fill(1), pixel: () => pixels });
  assert.ok(many.peak < first.peak + 1_000_000, `${first.peak} -> ${many.peak}`);
  assert.ok(many.heap <= 256 * 1024 * 1024);
  assert.equal(module._cords_used(), 0);
});

test('allocation budget failures are explicit and cleanup is repeatable', async () => {
  const module = await create();
  check(module, module._cords_budget(1024));
  assert.equal(module._cords_alloc(1025), 0);
  assert.match(message(module), /budget/);
  const pointer = allocate(module, 1024);
  assert.equal(module._cords_alloc(1), 0);
  module._cords_free(pointer);
  module._cords_reset();
  module._cords_reset();
  assert.equal(module._cords_used(), 0);
});

test('malformed and truncated JXL fail instead of returning a partial animation', async () => {
  const module = await create();
  const { bytes } = encode(module, { ticks: [1, 2, 3] });
  assert.throws(() => decode(module, bytes.subarray(0, Math.floor(bytes.length / 2))), /Malformed|truncated/);
  assert.throws(() => decode(module, new Uint8Array([0xff, 0x0a, 0, 0])), /Malformed|truncated/);
  assert.equal(module._cords_used(), 0);
});

test('invalid dimensions, oversized canvases and duration are rejected', async () => {
  const module = await create();
  assert.equal(module._cords_encode_begin(1025, 1, 8, 0, 1, 1, 0), -1);
  assert.equal(module._cords_encode_begin(0, 1, 8, 0, 1, 1, 0), -1);
  assert.throws(() => encode(module, { ticks: [121], numerator: 1 }), /duration/);
  assert.equal(module._cords_used(), 0);
});

function box(type, contents) {
  const header = Buffer.alloc(8);
  header.writeUInt32BE(contents.length + 8);
  header.write(type, 4, 4, 'ascii');
  return Buffer.concat([header, contents]);
}
function container(codestream, metadata) {
  if (codestream[0] !== 0xff) {
    // Level 10 already uses the standard signature + 20-byte ftyp box.
    assert.equal(codestream.subarray(4, 8).toString('ascii'), 'JXL ');
    assert.equal(codestream.readUInt32BE(12), 20);
    return Buffer.concat([codestream.subarray(0, 32), metadata, codestream.subarray(32)]);
  }
  return Buffer.concat([
    box('JXL ', Buffer.from([13, 10, 135, 10])),
    box('ftyp', Buffer.from('6a786c20000000006a786c20', 'hex')),
    metadata,
    box('jxlc', codestream),
  ]);
}
test('JXL container metadata is bounded before and after Brotli decompression', async () => {
  const module = await create();
  const { bytes } = encode(module, { ticks: [0, 1, 2] });
  const valid = container(bytes, box('Exif', Buffer.alloc(32)));
  assert.equal(decode(module, valid).frames.length, 3);
  assert.throws(() => decode(module, container(bytes, box('Exif', Buffer.alloc(1_048_577)))), /metadata/);
  const compressed = Buffer.concat([Buffer.from('Exif'), brotliCompressSync(Buffer.alloc(1_048_577))]);
  assert.throws(() => decode(module, container(bytes, box('brob', compressed))), /metadata/);
  const twoBoxes = Buffer.concat([box('Exif', Buffer.alloc(600_000)), box('xml ', Buffer.alloc(600_000))]);
  assert.throws(() => decode(module, container(bytes, twoBoxes)), /metadata/);
  assert.equal(module._cords_used(), 0);
});
