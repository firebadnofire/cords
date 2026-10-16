# Local avatar codec gate

This source build produces the libjxl encoder/decoder and incremental Rust source
decoder. Reviewed artifacts are checked in under `assets/avatar-codec/`; ordinary
client/server builds verify these pins and need no codec toolchain or download.
The Svelte worker editor/player and native profile endpoints use these modules.
The binding requirements and remaining packaged/platform gates are in
[the implementation plan](../../docs/plans/embedded-jxl-avatars.md).

## Build

Use Linux or WSL, Python 3, Git, Rust/rustup, CMake 3.31.6 and Ninja 1.11.1.
The Rust codec toolchain is pinned to 1.97.1. Install CMake/Ninja in a dedicated
Python virtual environment instead of changing system packages:

```sh
python3 -m venv /absolute/codec-cache/build-tools
/absolute/codec-cache/build-tools/bin/pip install cmake==3.31.6 ninja==1.11.1.4
export PATH="/absolute/codec-cache/build-tools/bin:$HOME/.cargo/bin:$PATH"
rustup toolchain install 1.97.1 --profile minimal
python3 tools/avatar-codec/build.py --cache /absolute/codec-cache --output /absolute/codec-output --jobs 4 --fixtures
```

The build checks pinned Emscripten/libjxl commits and submodule revisions before
using their sources. Emscripten 4.0.20 installs inside the selected cache; the
matching wasm-bindgen CLI installs there too. Rust dependencies use Cargo.lock
with `--locked`. An existing checkout with a different revision or tracked changes
fails with an actionable error; the script never resets it. Builds may be rerun.
The SDK/tool installation requires network access; image processing requires none.

`cords-jxl.manifest.json` records tool/dependency pins, source hashes, and the sizes
and SHA-256 hashes of both modules and their JavaScript bindings. Input changes
during a build invalidate the build. Use separate output directories and compare
the manifest `assets` fields to check repeated artifact output. Repeated local
builds are distinct evidence from independent-host/toolchain reproducibility.

## Validate

From the repository root, export tiny fixtures with the maintained Rust encoders,
then test the actual emitted WASM:

```sh
CORDS_AVATAR_FIXTURES=/absolute/codec-fixtures cargo test -p cords-avatar-source --lib
cargo clippy -p cords-avatar-source --all-targets -- -D warnings
node tools/avatar-codec/test-codec.mjs /absolute/codec-output
node tools/avatar-codec/test-layers.mjs /absolute/codec-output
node tools/avatar-codec/test-source.mjs /absolute/codec-output /absolute/codec-fixtures
```

The tests exercise transparent samples, 16-bit precision, exact/zero durations,
loops, duplicates, APNG default images, disposal/blending, incremental encoding,
crop/resize, malformed data, allocator failures and compressed metadata bombs.
Native tests additionally cover orientation and RGB ICC/gamma conversion. Fixture
pixel collections in tests are tiny; production source decoding returns one frame
at a time. The round-trip harness retains metadata and hashes between source frames.

## Processing boundary and limitations

Each JXL instance handles one operation. Call `cords_reset` and free caller-owned
buffers on failure/disposal; reset is repeatable. Drain output after each input
frame. Signal the final frame before draining it. Explicit lossless mode, alpha
distance zero and invisible RGB preservation are mandatory. The guarded decoder
patch exposes regular zero-duration layers even with coalescing enabled, retaining
native reference-canvas and blend processing. `test-layers.mjs` exercises this with
cropped 8/16-bit RGBA layers, all five blend modes, orientation, linear color
conversion and finite rational animation timelines. Its generator
is validation-only and is excluded from normal builds and shipped assets.

Source decoding applies orientation and supports RGB ICC profiles, standard RGB
chromaticities and PNG gamma conversion to sRGB at the source's 8/16-bit precision.
Gray/CMYK ICC, PNG CICP and nonstandard PNG chromaticities without an RGB ICC profile
currently fail explicitly. Geometric processing uses center-sampled nearest
neighbor at the original integer precision, preserving transparent samples.

The source module's linear memory has a hard 128 MiB maximum; JXL has 256 MiB.
JXL imports memory: instantiate an uploaded-JXL decoder with
`wasmMemory: new WebAssembly.Memory({ initial: 512, maximum: 2048 })` to impose
a hard 128 MiB limit beside the 256 MiB encoder. The actual WASM tests verify
decoding with both instances alive and explicit allocation/growth failure above
that decoder cap. Never reserve two default 256 MiB JXL instances for an upload.
These maxima must be reserved together against the operation's 512 MiB cap,
including host inputs, frame/output copies and any concurrent cache use. Rust
codec allocation limits and JXL allocator callbacks do not replace that aggregate
reservation or a terminable worker's deadline. The host worker/controller, broader
adversarial layer fixtures, and production distribution/rendering are still
required. Do not interpret these tests as upload/UI/protocol acceptance.

The TypeScript `avatarWorkerSession` supervisor now provides external termination,
shared heap/buffer reservations, serial commands and a single deadline. It also
requires encode workers to report each synchronous decode's entry/exit for the
15-second watchdog. Its tests include a real worker stuck in WebAssembly. The
codec command loop, editor integration and per-allocation worker accounting are
not yet connected; this supervisor alone is not the complete aggregate-memory gate.

The production TypeScript `avatarJxlReader` now wraps the actual coalesced decoder
incrementally. It preserves raw duration ticks/timebase/loops, returns one borrowed
WASM canvas at a time, enforces incoming-avatar dimensions, and cleans up on decode
failure. It requires an already reserved, exclusively owned module and the external
worker watchdog; it does not create a worker or complete the upload/player path.
Run its real-WASM adapter tests with Node 24 (the command also runs the ten core
codec tests):

```powershell
node --experimental-transform-types tools/avatar-codec/test-reader.mts target/avatar-codec
```

These tests require the fixture-enabled build above. Missing assets fail explicitly.
They compare every frame with the fixture decoder oracle, exercise an actual
128 MiB imported memory, and check truncated input, cleanup, static alpha and
over-limit incoming payloads. Tiny fixture arrays are test-only.

The production `avatarJxlEncoder` adapter now streams processed full-canvas frames
through the pinned encoder and drains into a caller-reserved output buffer capped
at 5,000,000 bytes. It retains no frame/chunk arrays, preserves raw timing and
precision, and releases WASM allocations on failure. It requires an exclusively
owned, reserved module and the external worker deadline. This adapter does not
activate upload, canonical profile publication, player rendering, or server
validation. Run its emitted-WASM tests with:

```powershell
node --experimental-transform-types tools/avatar-codec/test-encoder.mts target/avatar-codec
```

Ten tests cover source GIF/APNG/16-bit APNG/WebP plus identical per-frame crop,
rational/zero timing, duplicate frames, loops, transparency, output exhaustion,
invalid parameters, cleanup and the 1,000-frame limit.
