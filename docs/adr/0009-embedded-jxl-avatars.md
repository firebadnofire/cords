# ADR 0009: Embedded lossless static and animated JXL avatars

- Status: Accepted feature requirements; implementation gates incomplete
- Date: 2026-10-13
- Scope: Profile image processing, presentation, signed cards and distribution only

## Decision

Implement the [approved amended plan](../plans/embedded-jxl-avatars.md) in gated
functional units. Embedded JXL is the default local-upload result. HTTPS sources
remain an advanced option. This supersedes ADR 0004's URL-only shared-picture and
PNG-preview format choices where the avatar feature changes them; account vault,
locked-picker privacy, identity and password isolation remain unchanged.

Build libjxl 0.12.0 from its pinned upstream commit using Emscripten 4.0.20. Bundle
the resulting local WASM with clients and use bounded WASM validation on servers.
The bridge supplies incremental frame input/output and explicit lossless mode,
including alpha. It has no filesystem or network imports and no threaded runtime.
Its linear memory is capped at 256 MiB; this is part of, not additional to, each
operation's aggregate 512 MiB budget. Native allocations stay within the WASM
boundary; the Rust workspace's unsafe-code policy is not relaxed.
For JXL-to-JXL processing, the decoder imports a memory capped at 128 MiB while
the encoder retains its 256 MiB ceiling. This gives the same 384 MiB heap
reservation as source-WASM decoding beside JXL encoding. Host buffers, bindings
and caches share the remaining allowance; codec allocation failure stays explicit.

Zero-duration source frames remain distinct compositing steps. Preserve exact
rational metadata, including JXL zero-duration layers. Reject unrepresentable
timing rather than flattening, dropping frames or imposing browser minimum delays.
Metadata fidelity and best-effort playback scheduling are distinct guarantees.

The pinned libjxl decoder normally suppresses intermediate zero-duration layers
when coalescing. A narrow build-time patch exposes every regular layer while
retaining libjxl's native canvas blending and reference handling. It generates a
copy of `decode.cc`; the verified upstream checkout stays untouched. A source hash
and exact context guard fail the build when upstream changes. Maintain fixtures
for zero-duration cropped layers, reference canvases and 8/16-bit alpha blending
alongside canonical full-frame round trips before updating this dependency.

The profile schema, canonical/signature vectors, revision persistence and server
admission controls must pass their gates before runtime publication is activated.
Do not activate future ADR 0007 identity machinery as part of this feature. Profile
ordering uses authorized device signatures and durable device-scoped revisions,
not a server-owned identity, root key reuse, or a global multi-device consensus claim.

## Tradeoffs and validation

Bundled WASM avoids dependence on inconsistent WebView JXL support and confines
native codec memory to linear memory. It adds build dependencies and a codec asset
to packages. Workers/runtime interrupts must enforce deadlines because allocation
bounds alone do not bound computation. Codec callbacks do not account for host
buffers; aggregate host reservations and WASM memory ceilings are also mandatory.

The codec is not accepted from compilation alone. Test incremental memory use,
allocator exhaustion, frame/pixel equality, exact metadata, zero frames, malicious
inputs and deadlines before profile/UI integration. The final gate requires real
cross-client/server publication and packaged desktop playback on each platform.
