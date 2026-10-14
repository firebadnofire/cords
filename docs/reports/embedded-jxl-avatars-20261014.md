# Embedded JXL avatars: 2026-10-14 implementation evidence

## Plan

Continue the [approved amended plan](../plans/embedded-jxl-avatars.md) in its stated
gate order. This change establishes and exercises the codec build before signed
profile or upload/playback integration. No feature acceptance gate is complete.
Existing account-settings and role-policy changes remain in the workspace.

## Implementation

- `tools/avatar-codec` builds pinned libjxl 0.12.0/Emscripten 4.0.20 and the
  Rust source decoder with Rust 1.97.1/wasm-bindgen 0.2.126. It verifies source
  revisions, uses Cargo.lock, records source/asset hashes, refuses changed build
  inputs and can be rerun without resetting checkouts. Generated assets currently
  live outside tracked source; production packages do not yet include them.
- `crates/cords-avatar-source` decodes PNG/JPEG/GIF/APNG/WebP incrementally.
  APNG uses raw PNG frame controls and a precision-preserving compositor, including
  disposal, blending and a separate default image. GIF repeat counts map to total
  plays; absent loop extensions mean one play. Source zero delays stay zero.
  User-input-dependent GIF timing is rejected explicitly.
- Orientation and supported RGB ICC/PNG gamma conversion run before geometric
  processing. Integer samples remain 8 or 16 bits; alpha is retained. Unsupported
  color representations fail explicitly. Every composited frame can receive the
  same crop and nearest-neighbor resize without upscaling or precision reduction.
- The JXL bridge explicitly requests lossless color/alpha and invisible RGB
  preservation, drains encoder output per frame, validates frame/duration/pixel
  limits and uses a bounded allocator. Decoding enables color management, checks
  extra channels and bounds metadata including Brotli-decompressed container data.
- Source/JXL linear memories are capped at 128/256 MiB respectively. These caps
  are components of the required shared 512 MiB budget. JXL now imports memory:
  uploaded-JXL decoding can use a hard 128 MiB ceiling beside a 256 MiB encoder,
  preserving a 384 MiB combined heap reservation with space for host buffers.
- A source-hash/context-guarded build patch exposes regular zero-duration layers
  while retaining libjxl's coalescing/reference/blending implementation. It generates
  a separate source copy; verified upstream files are never modified. A validation-only
  fixture generator exercises cropped reference layers, rotation and linear color
  profiles at both 8-bit and 16-bit precision. It is excluded from shipped assets.
- `avatarPolicy.ts` defines checked resource/size limits, exact rational timing,
  shared allocation reservations, one deadline spanning retries and the 64-pixel
  retry floor. These primitives are tested but are not wired into an upload worker.
- `avatarWorkerSession.ts` adds the host supervisor: serial uploads/commands,
  pre-transfer heap/input/output reservations shared with caches, an external
  watchdog, per-decode phase deadlines, and one overall clock that includes idle
  crop time and can start before file reading. Cancellation, crashes and malformed
  responses terminate the worker. Returned buffers retain their reservations until
  consumers release them, even after worker termination. This is not yet connected
  to the codec command loop or editor; internal worker allocations still need their
  own reservations before the aggregate-memory gate can be accepted.
- The existing static importer now explicitly rejects animation controls rather
  than flattening APNG/animated WebP. GIF/JXL also return a clear unavailable error
  on this old path. This is a temporary fail-closed boundary, not animated upload
  support. Existing SVG sanitization and its tests remain intact.
- Dependency checking identified four rustls 0.23.45 advisories. The lockfile now
  selects 0.23.46, the recommended patch for all four, without new policy exceptions.

## Validation

Confirmed in this workspace:

- Rust source decoder: 17 passing unit tests, including the optional fixture-export
  test; clippy for all targets passes with warnings denied; source formatting passes.
- Client core: all 15 library tests pass after the rustls update.
- Production UI: all 18 test files / 76 tests pass. The focused image/policy tests
  pass with existing SVG cases retained. Svelte/TypeScript check has zero errors or
  warnings; changed-file ESLint/Prettier checks and the Vite production build pass.
- `cargo deny check advisories licenses bans sources` passes under the existing
  repository policy. Existing platform exceptions are unchanged. The first run
  failed on RUSTSEC-2026-0346 through RUSTSEC-2026-0349; the patch update resolved them.
- Actual emitted WASM: twenty-two JXL tests and eight source-to-JXL tests pass in local
  WSL. They cover precise/zero timing, finite/infinite loops, duplicate/order/pixel
  preservation, transparent samples, 16-bit samples, compositing, crop/resize,
  malformed data, metadata bombs, allocator failure and incremental encoder memory.
  New cases check cropped reference-layer blending, rotated dimensions/pixels,
  actual linear-to-sRGB conversion followed by exact processed-sample round trips,
  and hard allocation/growth failure with a 128 MiB imported decoder memory.
  All five JXL blend modes are covered at 8/16-bit precision. The 8-bit multiply
  fixture accounts for libjxl's ordered integer-output dithering, verified from
  its pinned `stage_write.cc` table; encoded processed pixels still round trip exactly.
- The same 30 actual WASM tests pass independently on `192.168.86.54` using Node
  in `/home/william/.cache/cords-avatar-check-20261014`. Existing Cords/PostgreSQL
  services were not changed. This is Linux WASM runtime evidence, not desktop,
  independent source-build, PostgreSQL, or two-client acceptance evidence.
- Two local CMake output directories produce identical codec asset hashes with
  the same pinned toolchain/source cache. This does not establish clean independent
  source/toolchain reproducibility on all supported operating systems.
- All 22 worker/policy focused tests pass, including termination of a real Node worker
  stuck inside synchronous WebAssembly. These verify the host watchdog, not browser
  packaging or a complete codec upload worker. See the UI test log for current totals.

Final generated WASM SHA-256 values:

| Asset                                | SHA-256                                                            |
| ------------------------------------ | ------------------------------------------------------------------ |
| `cords-jxl.wasm`                     | `5c98d827b926297039c5da9a7881a6bfaffe922ab7d2f2532fe4641a5b64f5c4` |
| `source/cords_avatar_source_bg.wasm` | `b937428e9570b25d962d742f89cf6cc9e2917046d22e49f653b83209721f3fe9` |

## Remaining work

The user still cannot upload, crop and publish an animated JXL avatar. Finish the
codec gate before advancing the signed-profile baseline:

1. Broaden adversarial/JXL layer fixtures beyond the tested cropped/reference
   sequences, including malformed layer data. Keep all blend modes and the guarded
   patch under test on dependency updates.
2. Connect the tested supervisor and real codec command loop to the shared 512 MiB reservation ledger and
   the single 120-second deadline, including source opening, previews, crop waiting,
   all resizing attempts and validation. Exercise cancellation, allocation traps,
   deadlines, serial jobs and bounded output without retaining decoded animations.
3. Broaden adversarial/color/JXL fixtures and verify encoded results against each
   processed source frame and exact metadata under those actual runtime controls.

Signed schema/revision/replay protection, server-side WASM/rate/concurrency controls,
profile references/endpoints/publication, sealed preference/cache synchronization,
animated crop/player/accessibility integration and packaging remain outstanding.
Real PostgreSQL/two-server/two-client/offline-restart acceptance and packaged Windows,
Linux and macOS playback have not been performed. No acceptance checkboxes changed.
