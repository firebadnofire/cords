# Public channels and channel succession validation — 2026-10-10

## Plan

Preserve account-root/device separation, TLS validation, server pinning, MLS and
existing authorization. Add immutable confidentiality and signed succession to the
protocol/storage first, enforce them on the server and client, then expose explicit
creation/replacement/acknowledgement/archive workflows. Validate each unit and the
real client/server boundary. ADR 0006 records the scoped architectural changes.

## Implementation

- Encrypted remains the creation default. Public routes carry application-layer
  plaintext with independently verified device signatures and root-certified
  contacts. Existing authenticated read/write, revocation and ownership gates apply.
- Signed channel identity permanently binds server, ID, mode and identity fields.
  PostgreSQL schemas 11–12 preserve encrypted legacy rows and permanent tombstones;
  they prohibit mode mutation, ID deletion and MLS state on public routes.
- Atomic device-signed replacement creates a distinct empty route, authenticates
  both identities and retires the original. Incoming and outgoing succession records
  remain separate so replacements can form a chain without rewriting prior evidence.
- Client vaults retain trusted identity/lifecycle records across restart and server
  removal. Conflicts and rollback fail closed. Background synchronization discovers
  authenticated retirement and succession after an offline period.
- Encrypted-to-public successors require durable explicit acknowledgement before
  sending. Selecting a route does not acknowledge it. Retired routes are read-only.
- Partial channel archives retain existing sealed message/relay cache rows and
  confidentiality metadata after server removal. The UI clears decrypted archive
  state on account lock/switch. No history, old secrets or attachments move to a successor.
- Desktop controls expose permanent mode, explicit replacement/retirement, public
  warnings, acknowledgement and partial read-only archives. Core/server enforce the
  same gates independently of the UI.

## Validation

Environment: Windows/PowerShell/MSVC, with a dedicated PostgreSQL 17 test container
on the available SSH host and a loopback-only tunnel. The disposable database was
used through `CORDS_TEST_DATABASE_URL`; no production service was migrated or deployed.
The task's disposable container and SSH forward were removed after validation.

- `cargo test --workspace --all-features -- --include-ignored --test-threads=1`:
  **53 passed**, including all explicitly enabled PostgreSQL/TLS tests.
- Final affected core/server rerun: **11 client-core and 24 server-core tests
  passed**; the existing WebSocket closure test hit intermittent Windows reset
  `10054`. Its subsequent exact isolated rerun passed (**1 test**).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- UI check, lint, build, formatting check and **32 tests across 10 files**: passed.
- `cargo deny check`: **failed** on the existing unmaintained `proc-macro-error2`
  advisory **RUSTSEC-2026-0173**. Bans, licenses and sources passed. No advisory
  suppression or unrelated dependency change was added.

The new real PostgreSQL tests check valid plaintext acceptance, tampering,
unauthorized signatures, cross-route replay, immutable modes in both directions,
deletion rejection, stale/invalid/replayed succession, idempotency, empty successors,
retired-write refusal and public/encrypted/public replacement chains.

The new TLS 1.3 integration uses two independent identities and encrypted vaults,
certificate validation with test-specific trust, and the actual client-core/server
path. It checks encrypted messaging, an offline client reconnecting after succession,
durable acknowledgement across restart, messages from both public authors, no
automatic history transfer, denied replacement authority, sealed local storage and
read-only archives surviving server removal. A separate restart test verifies that
conflicting signed confidentiality and lifecycle rollback cannot replace trusted state.

Both an initial parallel regression and the final affected serial rerun encountered
the existing Windows WebSocket reset. Isolated reruns and the full serial workspace
run passed. The server sends a Close after rejecting the revoked session; the test
requires a clean close on that branch and surfaces a TCP reset as an error. This
intermittent transport-close failure remains unresolved; no rejection assertion or
production transport behavior was weakened to conceal it. A disk-space failure
during compilation was resolved by moving only generated incremental build cache
to `D:\codex-build-cache\cords-public-20261010\incremental-preserved` and using
command-local temporary storage there. The cache remains recoverable; source,
databases and vaults were not removed.

## Security considerations and remaining evidence

Public signatures authenticate content; they do not make it confidential. Public
servers can read, copy, index and retain bodies. TLS certificate validation and
server identity pinning remain required. Public access currently uses admitted
membership and existing capabilities; anonymous reads and finer per-channel role
overrides depend on unfinished authorization infrastructure.

Archives are partial received history, not complete history exports, attachment
recovery, retained MLS transcripts or independent server/DM backups. Broader archive
and backup work remains deferred. Removing a local account still removes its vault.

No post-change packaged/native desktop smoke, production upgrade, release
publication or original process-kill acceptance-harness run is claimed. The TLS
integration hosts the actual server/client-core in-process and does not prove the
packaged desktop boundary. Upgrade client/server together; back up PostgreSQL,
its matching signing key and client vaults before forward-only schema migration.
Rollback requires matching pre-migration artifacts, not older binaries against
new metadata or a rollback of live MLS state.
