# Embedded static and animated JPEG XL avatars

Status: implementation in progress; no feature acceptance gate is complete.
Current evidence and remaining codec work: [2026-10-14 implementation report](../reports/embedded-jxl-avatars-20261014.md).
Picker shape correction and current missing publication/rendering path:
[2026-10-14 follow-up](../reports/avatar-picker-shape-20261014.md).
Approved scope: local upload, animated cropping, lossless embedded JXL, signed
profile distribution, and playback in the existing Rust/Svelte/Tauri application.
User amendment (2026-10-14): remove URL profile pictures; local files are the only
profile-picture source. Preserve concurrent account and role work and the separate
Jewel behavior. First replace the profile editor's URL controls with a local picker,
stop publishing/fetching profile URLs, and validate selection/crop/failure behavior.
The existing private still-image preview is explicitly local-only until the JXL
pipeline is connected; do not publish it as a substitute for lossless JXL.

## Plan and execution gates

Implement and validate the following units in order. Stop and diagnose a failed
gate before integrating the next unit. Do not mark CHECKPOINT.md complete from a
type, build, mock, or partial implementation.

1. **Codec:** reproducibly build pinned, bundled libjxl WASM; add incremental
   source decoders for PNG/JPEG/GIF/APNG/WebP/JXL. Test lossless pixel round trips,
   exact animation timing/loops, frame order, blending/disposal, alpha, orientation,
   color management, cropping/scaling and bounded allocation failure.
   Evaluate a guarded, generated-source change to libjxl's event selection so
   coalescing exposes regular zero-duration layers. Keep its native compositing
   and reference handling; validate against raw-layer metadata and expected
   composited pixels before activating this path. Do not change encoded timing.
   Next validate a worker supervisor independently of editor integration: permit
   one active upload, reserve both WASM heap ceilings and host buffers before
   worker creation/transfer, terminate on cancellation/crash/decode timeout, and
   keep one monotonic deadline through idle crop time and every command/retry.
   Returned frame buffers require explicit bounded ownership/release. Worker
   commands must reserve output capacity before decoding; no unsolicited frames.
   For JXL-to-JXL streaming, import a caller-bounded decoder memory with a 128 MiB
   ceiling beside the 256 MiB encoder. Test the actual import and growth failure;
   do not instantiate two default 256 MiB memories under the shared budget. This
   keeps the same 384 MiB heap reservation as source-WASM-to-JXL processing.
2. **Signed profiles:** update the unreleased client/server baseline together;
   authenticate embedded avatars with existing device signatures and
   deterministic CBOR. Add durable profile ordering and bounded server validation.
3. **Distribution:** separate profile presentation from hot-path authorization,
   exchange device-bound card hashes on member/request pages, fetch full signed
   cards when changed, and publish idempotently to joined servers. Validate with
   real PostgreSQL and independent clients across two servers and restarts.
4. **UI and caches:** upload during creation/editing, animated drag/zoom crop,
   confirmation/removal, common avatar player, account-isolated
   bounded caches, accessibility and visibility pausing. Test the actual packaged
   upload-to-remote-render workflow on Windows, Linux and macOS without native JXL.
5. **Packaging/documentation:** bundle local codec assets, narrow CSP additions,
   pinned builds, security/dependency checks, protocol and user documentation.

## Required processing contract

- Lossless means equality with processed pixels. Normalize orientation and apply
  profile-aware color processing; preserve supported integer precision and alpha.
  Reject unsupported representations explicitly. No third-party conversion/fetch.
- Stream frames through decoding, compositing, cropping, scaling, encoding and
  verification. Never collect all composited frames. The **512 MiB aggregate**
  includes all decoder/encoder allocations, WASM linear memory, retained source,
  intermediate/output buffers and transferred buffers across participating workers.
  Shared backing memory is counted once; copied memory is counted separately.
  Dispose prior attempts before retrying. Reserve memory before allocation and
  reject when the aggregate reservation would exceed the cap.
- Coalesce source disposal/blending faithfully and transform every source frame
  with the same geometry. Preserve frame order, duplicates, loop semantics and
  exact rational durations, including zero durations. Zero-duration frames still
  contribute compositing state. Never impose a minimum delay. If a source sequence
  cannot be represented exactly in JXL, reject it with an explicit timing error;
  never flatten it, drop frames, or invent/round timing.
- Exact metadata preservation is independent of best-effort presentation clocks.
  Timers, display refresh, hidden-window suspension and decode load cannot promise
  exact wall-clock playback. They must not rewrite the encoded timeline. Test
  metadata/pixels exactly and scheduling behavior separately.
- Maximum encoded JXL is **5,000,000 bytes**, before canonical padded Base64.
  SHA-256 covers the encoded bytes. Never use lossy compression or truncate output.
- Maximum output long edge is 1024, without upscaling. Retry oversized lossless
  encodes at 80% dimensions down to a 64-pixel long edge, preserving aspect ratio.
  Confirm final dimensions/preview. Failure/cancellation preserves the saved avatar.
- Limits: 50,000,000 input bytes; 1000 source frames; 8192 per input dimension;
  32 million canvas pixels; 100 million cumulative canvas pixels; 120 seconds per
  cycle; 1 MiB metadata. Infinite loop counts remain supported. Incoming embedded
  images additionally have at most 1024 by 1024 dimensions. Use checked arithmetic.
- One monotonic **120-second processing deadline** starts before opening/decoding
  the selected source and covers preview processing, crop/encode, every resize
  retry and final verification, including time in the upload/crop workflow. After
  expiry, require an explicit new upload operation. A decode has a maximum 15
  seconds, capped by the operation's remaining budget. Worker replacement and
  retries never reset the deadline. Cancellation
  terminates workers and releases reservations. Upload jobs run serially.

## Signed profile and validation contract

- Only an embedded source: `{encoding: base64, mime: image/jxl,
sha256, data}`, with shape presentation. Embedded crops are baked. URL sources are rejected.
  Signature authenticates the complete card. Strict Base64, digest, actual format,
  dimensions and bounded decoding checks apply to untrusted incoming images.
- No legacy signed-card reader, downgrade, alternate signed representation or
  fabricated migration signature. Reject incompatible servers before publication.
  URL selection, publication and fetching are removed from the current client.
- Signed per-account/device profile revisions increase on an actual profile
  change, using checked arithmetic and atomic durable saves. Reuse one signed
  revision across servers, authentication renewal and retries. Do not use timestamps
  as ordering. Persist highest observed revision and canonical card hash across
  restart and across servers in that account's sealed storage. Older revisions
  cannot replace known state; identical repeats are idempotent; equal-revision
  different-content records are conflicts, not last-write-wins. Device revocation
  and account identity validation remain authoritative. This is local observed-state
  replay protection, not proof of unseen freshness or global multi-device consensus.
- Server image validation has explicit admission rate limits, bounded concurrency,
  a per-operation deadline/compute allowance and allocation accounting that includes
  codec/WASM memory. Verify identity/signature/size/hash before decoding. A successful
  digest validation cache never bypasses profile signatures, revisions or revocation.
- Only single-card endpoints receive 8 MiB request limits. Base64 can be 6,666,668
  characters. Keep chat-envelope limits and paginated response limits bounded.
  Preserve offline profile saves, explicit per-server failures/retries and sealed
  account isolation. Do not resend multi-megabyte cards with every chat event.

## Presentation and cache contract

Bundled local WASM canvas playback is required; native animated JXL is not assumed.
Animate visible avatars; pause offscreen/hidden windows. Honor OS and application
reduced motion plus a pause-animations setting; offer accessible explicit playback
in profile/editing views. Finite loops do not expire while paused. Derived still
notification images never replace the canonical animation.

Hash-based caches are bounded to 64 MiB encoded memory, 128 MiB decoded memory and
128 MiB sealed disk. These are ceilings, not additional allowances above the shared
processing memory cap. Evict with LRU; stream large animations. Clear workers and
memory on lock/account switch; respect picker privacy. Profile images never fetch external URLs.
Invalid avatars fall back to initials with an unavailable state and bounded retries.

## Validation

Codec fixtures cover each source format, zero and rational durations, finite and
infinite loops, frame duplicates/order, transparency, disposal/blending, separate
APNG default images, orientation/profiles and transformed-pixel equality. Security
tests cover memory peaks/allocator failures, malformed/truncated input, timing
overflow/unrepresentability, decompression bombs, deadlines spanning retries,
signature/hash/MIME/Base64 errors and the exact compressed-size boundary.

Protocol and integration tests cover restart-persistent rollback/conflict rejection,
cross-server stale replay, idempotency, cache tampering, validation floods and
bounded concurrency, publication/replacement/removal/offline saves, scoped transport
limits and isolated account switches. UI tests cover actual upload/crop/save/player,
reduced motion, pausing, errors and rejection of URL sources. Report packaged/platform/CI proof
separately from local tests; do not claim unavailable gates passed.
