// Actual emitted WASM through the production command loop. No fixture frame arrays
// enter production; this test retains only the small compressed result and hashes.
import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import { AvatarCodecRuntime } from '../../bins/cords-client/ui/src/avatarCodecRuntime.ts';
import { AvatarJxlReader } from '../../bins/cords-client/ui/src/avatarJxlReader.ts';

const directory = resolve(process.argv[2] ?? 'target/avatar-codec');
const { default: createJxl } = await import(pathToFileURL(`${directory}/cords-jxl.mjs`).href);
const wasmBinary = await readFile(`${directory}/cords-jxl.wasm`);
const source = await import(pathToFileURL(`${directory}/source/cords_avatar_source.js`).href);
source.initSync({ module: await readFile(`${directory}/source/cords_avatar_source_bg.wasm`) });
const codecs = {
  source: (input: Uint8Array) => new source.SourceDecoder(input),
  jxl: (ceiling: number) => createJxl({ wasmBinary, wasmMemory: new WebAssembly.Memory({ initial: 512, maximum: ceiling / 65536 }) }),
};
const geometry = { x: 0, y: 0, width: 2, height: 1, outWidth: 1, outHeight: 1 };

for (const name of ['animated.gif', 'animated.png', 'animated16.png', 'animated.webp']) {
  test(`${name}: command loop streams, verifies, previews and recrops every frame`, async () => {
    const phases: string[] = [];
    const runtime = new AvatarCodecRuntime(codecs, (phase) => phases.push(phase));
    const input = new Uint8Array(await readFile(`${directory}/fixtures/${name}`));
    const opened = await runtime.command('open', null, 0, input.buffer);
    const meta = opened.value;
    assert.ok('durations' in meta);
    assert.ok(meta.durations.length > 1);
    const preview = await runtime.command('preview', geometry, 4);
    assert.equal(preview.buffer?.byteLength, 4);
    const encoded = await runtime.command('encode', geometry, 5_000_000);
    assert.ok('length' in encoded.value);
    const bytes = new Uint8Array(encoded.buffer!, 0, encoded.value.length as number);
    if (process.argv[3]) {
      await mkdir(resolve(process.argv[3]), { recursive: true });
      await writeFile(resolve(process.argv[3], `${name}.jxl`), bytes);
    }
    assert.ok(bytes.byteLength > 0);
    assert.ok(runtime.peakHostBytes <= 48 * 1024 * 1024);
    assert.equal(phases.length % 2, 0);
    for (let i = 0; i < phases.length; i += 2) assert.deepEqual(phases.slice(i, i + 2), ['decode', 'process']);
    const module = await codecs.jxl(128 * 1024 * 1024);
    const reader = new AvatarJxlReader(module, bytes, 128 * 1024 * 1024, true);
    try {
      assert.equal(reader.info.width, 1);
      assert.equal(reader.info.loops, meta.loops);
      const durations: number[] = [];
      for (;;) { const frame = reader.next(); if (!frame) break; durations.push(frame.duration); }
      assert.deepEqual(durations, meta.durations);
    } finally { reader.dispose(); }
    const second = new AvatarCodecRuntime(codecs, () => {});
    await second.command('open', null, 0, bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.length));
    const recoded = await second.command('encode', { ...geometry, width: 1 }, 5_000_000);
    assert.ok('length' in recoded.value && recoded.value.length as number > 0);
  });
}

test('invalid geometry, output limits and malformed inputs fail explicitly', async () => {
  const runtime = new AvatarCodecRuntime(codecs, () => {});
  const bytes = new Uint8Array(await readFile(`${directory}/fixtures/animated.gif`));
  await assert.rejects(runtime.command('encode', geometry, 1), /Open/);
  await runtime.command('open', null, 0, bytes.buffer);
  await assert.rejects(runtime.command('open', null, 0, bytes.buffer), /repeated/);
  await assert.rejects(runtime.command('encode', { ...geometry, x: -1 }, 5_000_000), /geometry/);
  await assert.rejects(runtime.command('encode', geometry, 5_000_001), /capacity/);
  await assert.rejects(runtime.command('preview', geometry, 3), /reserved/);
  await assert.rejects(runtime.command('encode', geometry, 1), /output limit/);
  const result = await runtime.command('encode', geometry, 5_000_000);
  assert.ok('length' in result.value);
  const broken = new AvatarCodecRuntime(codecs, () => {});
  await assert.rejects(broken.command('open', null, 0, new Uint8Array([1, 2, 3]).buffer));
});
