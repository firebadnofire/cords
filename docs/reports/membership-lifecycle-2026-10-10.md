# Membership lifecycle validation — 2026-10-10

## Plan

Inspect and preserve existing one-time ownership bootstrap, identity certification, client vault,
authorization and MLS boundaries. Implement admission/card presentation and server actions together
with the explicitly requested multi-server departure/burn/succession behavior. Validate each unit
before checking the integrated client/server path. Decisions and operator rules are in ADR 0005.

## Implementation

- Explicit pending/rejected/removed admission states, bounded member/request pagination and
  device-signed self-declared nickname/PNG card verified against the root-certified device.
- App-specific server context menu and sidebar/settings Leave/Archive actions. Local-only removal
  is a separately warned confirmation. Archive retains metadata, not conversations or active MLS.
- Server-scoped encrypted caches/MLS snapshots; legacy cache rekeying at unlock; socket detachment
  on server/account changes; durable idempotent departures and burn-delivery backoff/errors.
- Immutable successor designation, countersigned acceptance, server-clock 30-day maturation,
  atomic owner burn/transfer or lockdown, operator-only recovery and permanent burn tombstones.
- Terminal permission refusals no longer poison the outbox. Ownership state changes expire stale
  credentials for device-proof renewal; background work never automatically submits a reinvite.
- PostgreSQL schema 10 retains genuine signed departure evidence and successor history. The
  removal queue now references known device contacts rather than requiring a device-revocation
  record for voluntary departure. Presentation images are excluded from MLS roster snapshots.

## Validation

Environment: Windows/PowerShell/MSVC, Rust 1.97.1; a separately created PostgreSQL 17 container on
the available test host, accessed through a loopback SSH tunnel. No production database or server
container was used, migrated, restarted or deleted.

- Server core: **23 passed**, including **19 explicitly enabled real PostgreSQL/TLS tests**.
  The independent TLS test was rerun successfully after the final roster-presentation separation.
- Client core: **8 passed**, including independent server cache isolation, restart, local removal,
  metadata-only archive, persisted burn retry/backoff and legacy authenticated-cache rekeying.
- Identity: **4 passed**; protocol: **7 passed** plus **1 signed-vector integration test**.
- Storage: **1 passed**; Tauri Rust: **2 passed**.
- UI: **25 passed**, including pending admission without raw HTTP errors, background lockdown
  without automatic dialogs/recovery attempts, signed-card presentation, and distinct context-menu
  leave/archive callbacks. These UI tests mock only the narrow Tauri presentation boundary.
- Svelte/TypeScript check, ESLint, production Vite build, Rust formatting, Git whitespace check
  and selected production/test-target Clippy checks with `-D warnings`: passed.
- `cargo deny check`: **failed**, unchanged `proc-macro-error2` unmaintained advisory
  **RUSTSEC-2026-0173**. Bans, licenses and sources passed. No advisory suppression was added.

The integrated regression uses real certificate-validated TLS 1.3, HTTPS and WebSocket with two
independently generated client identities/vaults and two independently pinned server identities.
It verifies profile-card admission/approval, denied permissions without outbox poisoning, encrypted
message delivery and absence of its plaintext marker in server relay envelopes, independent server
switching, signed departure plus MLS removal, archive/owner lockdown, identity burn and restart of
the burned local vault. The ephemeral certificate is trusted only by the test clients.

The test revealed and fixed an outbox permission-refusal bug and the legacy removal-queue foreign
key mismatch. A Windows linker failure occurred when rebuilding an executable still running an
earlier test; the active test was allowed to finish and the rebuild/rerun passed.

## Remaining evidence boundaries

No post-change packaged/native desktop smoke, live deployment upgrade, release publication or
original process-kill/server/PostgreSQL-restart acceptance script run is claimed. The new TLS test
is in-process client-core/server hosting, not proof of the packaged desktop boundary. Update both
client and server and run that smoke before deployment acceptance. Back up the matching database,
server signing key and client vaults before applying forward-only migrations; older binaries are
not a valid schema-10 rollback. Restrict recovery-code logs and preserve unlocked burned vaults
until pending notifications have been delivered.
