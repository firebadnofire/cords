import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";
import { create, decode, encode } from "./test-codec.mjs";

const directory = resolve(process.argv[2] ?? "target/avatar-codec");
const { default: createFixtures } = await import(
  pathToFileURL(`${directory}/cords-fixtures.mjs`)
);
const fixtures = await createFixtures({
  wasmBinary: await readFile(`${directory}/cords-fixtures.wasm`),
});

function fixtureBytes(bits, scenario) {
  const size = fixtures._fixture_build(bits, scenario);
  const error = fixtures._fixture_error();
  let end = error;
  while (fixtures.HEAPU8[end]) end++;
  assert.ok(
    size > 0,
    new TextDecoder().decode(fixtures.HEAPU8.subarray(error, end)),
  );
  const start = fixtures._fixture_data();
  return fixtures.HEAPU8.slice(start, start + size);
}

for (const bits of [8, 16]) {
  for (const scenario of [0, 1]) {
    test(`${bits}-bit cropped JXL layers preserve zero timing, references, blend and orientation ${scenario}`, async () => {
      const bytes = fixtureBytes(bits, scenario);
      const module = await create();
      const result = decode(module, bytes, true);
      assert.deepEqual(
        result.frames.map((frame) => frame.duration),
        [0, 7, 0, 60],
      );
      const width = scenario ? 1 : 2;
      const height = scenario ? 2 : 1;
      assert.deepEqual(result.basic, [width, height, bits, 1, 420, 1, 3]);
      const max = bits === 16 ? 65535 : 255;
      const half = bits === 16 ? 32768 : 128;
      const third = bits === 16 ? 49151 : 191;
      const expected = [
        [max, 0, 0, max, 0, 0, max, max],
        [max - half, half, 0, max, 0, 0, max, max],
        [max, 0, 0, max, 0, max, 0, max],
        [third, third + 1, 0, max, 0, 0, max, max],
      ];
      result.frames.forEach((frame, index) => {
        const samples =
          bits === 16
            ? [
                ...new Uint16Array(
                  frame.pixels.buffer,
                  frame.pixels.byteOffset,
                  frame.pixels.byteLength / 2,
                ),
              ]
            : [...frame.pixels];
        assert.deepEqual(samples, expected[index]);
      });
      // Re-encode all composited layers as full replacement frames; compare exact
      // processed pixels and the original rational timeline/finite loop semantics.
      const canonical = encode(module, {
        width,
        height,
        bits,
        ticks: [0, 7, 0, 60],
        numerator: 420,
        loops: 3,
        pixel: (index) => result.frames[index].pixels,
      });
      const verified = decode(module, canonical.bytes, true);
      assert.deepEqual(verified, result);
    });
  }
  for (const [scenario, name] of [
    [3, "ADD"],
    [4, "MUL"],
    [5, "MULADD"],
  ]) {
    test(`${bits}-bit cropped JXL ${name} layers retain processed pixels and exact metadata`, async () => {
      const module = await create();
      const result = decode(module, fixtureBytes(bits, scenario), true);
      const max = bits === 16 ? 65535 : 255;
      const half = bits === 16 ? 32768 : 128;
      // libjxl stage_write.cc uses ordered dithering for UINT8 output. At
      // (0,0), alpha's table offset is (5,7): round(128²/255 + 0.48833) = 65.
      // UINT16 output uses nearest integer without that dither. This is output
      // quantization of fractional compositing, not an encoder quality change.
      const quarter = bits === 16 ? 16384 : 65;
      const mixed =
        scenario === 3
          ? [max, max, 0, max]
          : scenario === 4
            ? [0, 0, 0, half]
            : [max, half, 0, max];
      const final = scenario === 4 ? [0, 0, 0, quarter] : [max, max, 0, max];
      const expected = [
        [max, 0, 0, max, 0, 0, max, max],
        [...mixed, 0, 0, max, max],
        [max, 0, 0, max, 0, max, 0, max],
        [...final, 0, 0, max, max],
      ];
      result.frames.forEach((frame, index) => {
        const samples =
          bits === 8
            ? [...frame.pixels]
            : [
                ...new Uint16Array(
                  frame.pixels.buffer,
                  frame.pixels.byteOffset,
                  frame.pixels.byteLength / 2,
                ),
              ];
        assert.deepEqual(samples, expected[index]);
      });
      assert.deepEqual(
        result.frames.map((frame) => frame.duration),
        [0, 7, 0, 60],
      );
      assert.deepEqual(result.basic, [2, 1, bits, 1, 420, 1, 3]);
      const canonical = encode(module, {
        bits,
        ticks: [0, 7, 0, 60],
        numerator: 420,
        loops: 3,
        pixel: (index) => result.frames[index].pixels,
      });
      assert.deepEqual(decode(module, canonical.bytes, true), result);
    });
  }
  test(`${bits}-bit non-XYB linear JXL converts color before canonical lossless re-encoding`, async () => {
    const module = await create();
    const result = decode(module, fixtureBytes(bits, 2), true);
    const first = result.frames[0].pixels;
    const red =
      bits === 8
        ? first[0]
        : new Uint16Array(
            first.buffer,
            first.byteOffset,
            first.byteLength / 2,
          )[0];
    // Linear-light 0.5 maps to approximately 0.735 sRGB. Check the conversion
    // independently of round-trip equality, which could otherwise pass unchanged
    // source samples. Exact processed samples must then survive re-encoding.
    assert.ok(
      red / (bits === 8 ? 255 : 65535) > 0.73 &&
        red / (bits === 8 ? 255 : 65535) < 0.74,
    );
    const canonical = encode(module, {
      bits,
      ticks: [0, 7, 0, 60],
      numerator: 420,
      loops: 3,
      pixel: (index) => result.frames[index].pixels,
    });
    assert.deepEqual(decode(module, canonical.bytes, true), result);
  });
}
