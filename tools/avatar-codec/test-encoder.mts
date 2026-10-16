// Node 24: node --experimental-transform-types tools/avatar-codec/test-encoder.mts <assets>
// Actual emitted WASM through production adapters; arrays collected only for tiny test oracles.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import { AvatarJxlEncoder } from '../../bins/cords-client/ui/src/avatarJxlEncoder.ts';
import { AvatarJxlReader } from '../../bins/cords-client/ui/src/avatarJxlReader.ts';
import { exactJxlTiming } from '../../bins/cords-client/ui/src/avatarPolicy.ts';

const directory = resolve(process.argv[2] ?? 'target/avatar-codec');
const { default: createJxl } = await import(pathToFileURL(`${directory}/cords-jxl.mjs`).href);
const wasmBinary = await readFile(`${directory}/cords-jxl.wasm`);
const source = await import(pathToFileURL(`${directory}/source/cords_avatar_source.js`).href);
source.initSync({ module: await readFile(`${directory}/source/cords_avatar_source_bg.wasm`) });
const ceiling = 256 * 1024 * 1024;
const info = { width: 2, height: 1, bits: 8 as const, animated: true,
  ticksPerSecondNumerator: 30_000, ticksPerSecondDenominator: 1001, loops: 3 };

for (const bits of [8, 16] as const) {
  test(`${bits}-bit encoder streams duplicate and zero-duration frames with exact rational timing`, async () => {
    const module = await createJxl({ wasmBinary });
    const output = new Uint8Array(5_000_000);
    const encoder = new AvatarJxlEncoder(module, { ...info, bits }, output);
    const ticks = [0, 1, 1, 0, 2, 0];
    const pixels = (i: number) => bits === 8
      ? new Uint8Array([i === 2 ? 1 : i, 50, 80, 255, 22, 33, 44, 0])
      : new Uint8Array(new Uint16Array([65535, 30100, 2010 + i, 12345, 55000, 20000, 30000, 0]).buffer);
    let encoded: Uint8Array;
    try {
      assert.throws(() => encoder.finish(), /did not finish/);
      for (let i = 0; i < ticks.length; i++) encoder.push(pixels(i), ticks[i], i === ticks.length - 1);
      encoded = encoder.finish();
      assert.equal(encoded.buffer, output.buffer, 'finish must not make a compressed-output copy');
      assert.throws(() => encoder.push(pixels(0), 1, true), /closed/);
    } finally { encoder.dispose(); encoder.dispose(); }
    assert.equal(module._cords_used(), 0);
    const reader = new AvatarJxlReader(module, encoded!, ceiling, true);
    try {
      assert.deepEqual(reader.info, { ...info, bits });
      for (let i = 0; i < ticks.length; i++) {
        const frame = reader.next()!;
        assert.equal(frame.duration, ticks[i]);
        assert.equal(frame.last, i === ticks.length - 1);
        assert.deepEqual(frame.pixels, pixels(i));
      }
      assert.equal(reader.next(), undefined);
    } finally { reader.dispose(); }
    assert.equal(module._cords_used(), 0);
  });
}

for (const name of ['animated.gif', 'animated.png', 'animated16.png', 'animated.webp']) {
  test(`${name}: source frames and a consistent crop pass through the production encoder`, async () => {
    const bytes = new Uint8Array(await readFile(`${directory}/fixtures/${name}`));
    let decoder = new source.SourceDecoder(bytes);
    const delays: { numerator: bigint; denominator: bigint }[] = [];
    const hashes: string[] = [];
    const { createHash } = await import('node:crypto');
    const hash = (pixels: Uint8Array) => createHash('sha256').update(pixels).digest('hex');
    const bits = decoder.bits;
    const loops = decoder.loops;
    try {
      for (;;) {
        const frame = decoder.next_frame();
        if (!frame) break;
        try {
          delays.push({ numerator: BigInt(frame.numerator), denominator: BigInt(frame.denominator) });
          frame.crop_resize(0, 0, 2, 1, 1, 1);
          hashes.push(hash(frame.take_pixels()));
        } finally { frame.free(); }
      }
    } finally { decoder.free(); }
    const timing = exactJxlTiming(delays);
    const module = await createJxl({ wasmBinary });
    const encoder = new AvatarJxlEncoder(module, { ...info, width: 1, bits, loops, ...timing }, new Uint8Array(5_000_000));
    decoder = new source.SourceDecoder(bytes);
    let encoded: Uint8Array;
    try {
      for (let i = 0; i < delays.length; i++) {
        const frame = decoder.next_frame()!;
        try {
          frame.crop_resize(0, 0, 2, 1, 1, 1);
          encoder.push(frame.take_pixels(), timing.durations[i], i === delays.length - 1);
        } finally { frame.free(); }
      }
      assert.equal(decoder.next_frame(), undefined);
      encoded = encoder.finish();
    } finally { decoder.free(); encoder.dispose(); }
    const reader = new AvatarJxlReader(module, encoded!, ceiling, true);
    try {
      assert.equal(reader.info.bits, bits);
      assert.equal(reader.info.loops, loops);
      assert.equal(reader.info.ticksPerSecondNumerator, timing.ticksPerSecondNumerator);
      assert.equal(reader.info.ticksPerSecondDenominator, timing.ticksPerSecondDenominator);
      for (let i = 0; i < delays.length; i++) {
        const frame = reader.next()!;
        assert.equal(frame.duration, timing.durations[i]);
        assert.equal(hash(frame.pixels), hashes[i]);
      }
      assert.equal(reader.next(), undefined);
    } finally { reader.dispose(); }
    assert.equal(module._cords_used(), 0);
  });
}

test('output exhaustion invalidates the encoder and releases every WASM allocation', async () => {
  const module = await createJxl({ wasmBinary });
  const encoder = new AvatarJxlEncoder(module, info, new Uint8Array(1));
  assert.throws(() => encoder.push(new Uint8Array(8), 0, true), /reserved output limit/);
  assert.throws(() => encoder.finish(), /did not finish/);
  encoder.dispose();
  assert.equal(module._cords_used(), 0);
});

test('invalid duration, input size, cycle and reservations fail with cleanup', async () => {
  const module = await createJxl({ wasmBinary });
  for (const [pixels, duration] of [[new Uint8Array(7), 0], [new Uint8Array(8), -1],
    [new Uint8Array(8), 3_600_001]] as const) {
    const encoder = new AvatarJxlEncoder(module, { ...info, ticksPerSecondDenominator: 1 }, new Uint8Array(100_000));
    assert.throws(() => encoder.push(pixels, duration, true), /Invalid JXL frame|cycle exceeds/);
    assert.equal(module._cords_used(), 0);
  }
  assert.throws(() => new AvatarJxlEncoder(module, info, new Uint8Array(5_000_001)), /Reserve/);
  assert.throws(() => new AvatarJxlEncoder(module, info, new Uint8Array(100), 1), /memory ceiling/);
  assert.equal(module._cords_used(), 0);
});

test('still encoder preserves hidden transparent samples and prevents partial still sequences', async () => {
  const module = await createJxl({ wasmBinary });
  const pixels = new Uint8Array([11, 23, 45, 0, 50, 60, 70, 128]);
  const encoder = new AvatarJxlEncoder(module, { ...info, animated: false }, new Uint8Array(100_000));
  encoder.push(pixels, 0, true);
  const encoded = encoder.finish();
  encoder.dispose();
  const reader = new AvatarJxlReader(module, encoded, ceiling, true);
  try {
    assert.deepEqual(reader.next()?.pixels, pixels);
    assert.equal(reader.next(), undefined);
  } finally { reader.dispose(); }
  const invalid = new AvatarJxlEncoder(module, { ...info, animated: false }, new Uint8Array(100_000));
  assert.throws(() => invalid.push(pixels, 0, false), /one final/);
  assert.equal(module._cords_used(), 0);
});

test('zero-duration compositing steps still count toward the 1,000-frame limit', async () => {
  const module = await createJxl({ wasmBinary });
  const encoder = new AvatarJxlEncoder(module, info, new Uint8Array(5_000_000));
  const pixels = new Uint8Array(8);
  for (let i = 0; i < 1000; i++) encoder.push(pixels, 0, false);
  assert.throws(() => encoder.push(pixels, 0, true), /frame or cumulative/);
  assert.throws(() => encoder.finish(), /did not finish/);
  assert.equal(module._cords_used(), 0);
});
