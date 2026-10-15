// Actual pinned WASM through the production incremental TypeScript reader.
// Node 24: node --experimental-transform-types tools/avatar-codec/test-reader.mts <assets>
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import { AvatarJxlReader } from '../../bins/cords-client/ui/src/avatarJxlReader.ts';
import { create, decode, encode } from './test-codec.mjs';

const directory = resolve(process.argv[2] ?? 'target/avatar-codec');
const { default: createJxl } = await import(pathToFileURL(`${directory}/cords-jxl.mjs`).href);
const wasmBinary = await readFile(`${directory}/cords-jxl.wasm`);
const { default: createFixtures } = await import(pathToFileURL(`${directory}/cords-fixtures.mjs`).href);
const fixtures = await createFixtures({ wasmBinary: await readFile(`${directory}/cords-fixtures.wasm`) });
const decoderBytes = 128 * 1024 * 1024;

function fixture(bits: number, scenario: number): Uint8Array {
  const size = fixtures._fixture_build(bits, scenario);
  assert.ok(size > 0);
  return fixtures.HEAPU8.slice(fixtures._fixture_data(), fixtures._fixture_data() + size);
}

for (const bits of [8, 16]) {
  for (const scenario of [0, 1, 2, 3, 4, 5]) {
    test(`production reader: ${bits}-bit JXL scenario ${scenario}, exact zero timing and incremental canvas`, async () => {
      const memory = new WebAssembly.Memory({ initial: 512, maximum: 2048 });
      const module = await createJxl({ wasmBinary, wasmMemory: memory });
      assert.equal(module.HEAPU8.buffer, memory.buffer);
      const bytes = fixture(bits, scenario);
      // Tiny fixture-only oracle; production never collects decoded frames.
      const expected = decode(module, bytes, true);
      const reader = new AvatarJxlReader(module, bytes, decoderBytes);
      try {
        assert.deepEqual(reader.info, {
          width: scenario === 1 ? 1 : 2, height: scenario === 1 ? 2 : 1,
          bits, animated: true, ticksPerSecondNumerator: 420,
          ticksPerSecondDenominator: 1, loops: 3,
        });
        const durations: number[] = [];
        let backing: ArrayBufferLike | undefined;
        let offset = 0;
        for (;;) {
          const frame = reader.next();
          if (!frame) break;
          assert.equal(frame.index, durations.length);
          assert.equal(frame.last, frame.index === 3);
          assert.equal(frame.pixels.length, 2 * 4 * bits / 8);
          assert.deepEqual(frame.pixels, expected.frames[frame.index].pixels);
          // One reused canvas. Retain no frame copies between iterations.
          if (backing) {
            assert.equal(frame.pixels.buffer, backing);
            assert.equal(frame.pixels.byteOffset, offset);
          }
          backing = frame.pixels.buffer;
          offset = frame.pixels.byteOffset;
          if (frame.index === 0 && scenario !== 2) {
            const samples = bits === 8 ? frame.pixels : new Uint16Array(backing, offset, 8);
            const max = bits === 8 ? 255 : 65535;
            assert.deepEqual([...samples], [max, 0, 0, max, 0, 0, max, max]);
          }
          durations.push(frame.duration);
        }
        assert.deepEqual(durations, [0, 7, 0, 60]);
        assert.equal(reader.next(), undefined);
      } finally {
        reader.dispose();
        reader.dispose();
        assert.equal(module._cords_used(), 0);
      }
      assert.throws(() => reader.next(), /disposed/);
    });
  }
}

test('production reader rejects truncated animation and frees the decoder and canvas', async () => {
  const bytes = fixture(8, 0);
  const module = await create();
  assert.throws(() => {
    const reader = new AvatarJxlReader(module, bytes.subarray(0, bytes.length - 3), 256 * 1024 * 1024);
    try { while (reader.next()) { /* drain without retaining frames */ } }
    finally { reader.dispose(); }
  }, /Malformed|truncated/);
  assert.equal(module._cords_used(), 0);
});

test('production reader rejects invalid reservations before leaking an allocation', async () => {
  const module = await create();
  assert.throws(() => new AvatarJxlReader(module, fixture(8, 0), 1), /memory ceiling/);
  assert.equal(module._cords_used(), 0);
  assert.throws(() => new AvatarJxlReader(module, new Uint8Array(5_000_001), decoderBytes, true), /compressed image limit/);
  assert.equal(module._cords_used(), 0);
});

test('production reader keeps static transparency, precision and static metadata', async () => {
  const module = await create();
  const pixels = new Uint8Array([11, 23, 45, 0, 50, 60, 70, 128]);
  const { bytes } = encode(module, { animated: false, ticks: [0], pixel: () => pixels });
  const reader = new AvatarJxlReader(module, bytes, 256 * 1024 * 1024, true);
  try {
    assert.equal(reader.info.animated, false);
    assert.equal(reader.info.loops, 1);
    assert.deepEqual(reader.next()?.pixels, pixels);
    assert.equal(reader.next(), undefined);
  } finally {
    reader.dispose();
    assert.equal(module._cords_used(), 0);
  }
});
