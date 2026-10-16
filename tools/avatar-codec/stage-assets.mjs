// Ship only the reviewed codec artifacts. No network or implicit native decoder.
import { createHash } from 'node:crypto';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const source = resolve(process.env.CORDS_AVATAR_CODEC_DIR ?? `${root}/assets/avatar-codec`);
const output = `${root}/bins/cords-client/ui/public/avatar-codec`;
const hashes = {
  'cords-jxl.mjs': 'f96769ba1754c6303bd5f299c9060cb2fd1763401156561608e19c4403b015e5',
  'cords-jxl.wasm': '5c98d827b926297039c5da9a7881a6bfaffe922ab7d2f2532fe4641a5b64f5c4',
  'source/cords_avatar_source.js': '4b74799f32dd020a5e75ed154a69d2025d547a16ffa72c75fa89673a2e452801',
  'source/cords_avatar_source_bg.wasm': 'b937428e9570b25d962d742f89cf6cc9e2917046d22e49f653b83209721f3fe9',
};
// Check everything before writing anything. An incomplete build must fail loudly.
const assets = await Promise.all(Object.entries(hashes).map(async ([path, hash]) => {
  const bytes = await readFile(`${source}/${path}`);
  if (createHash('sha256').update(bytes).digest('hex') !== hash)
    throw new Error(`Avatar codec hash mismatch: ${path}. Rebuild and review codec pins.`);
  return [path, bytes];
}));
for (const [path, bytes] of assets) {
  await mkdir(dirname(`${output}/${path}`), { recursive: true });
  await writeFile(`${output}/${path}`, bytes);
}
await writeFile(`${output}/package.json`, '{"type":"module"}\n');
console.log('Staged four pinned avatar codec assets');
