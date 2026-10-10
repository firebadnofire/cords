# Cords project dissection

## Snapshot and evidence boundary

This document describes the checked-out repository at tag `v0.2.0`, commit
`13b62cc`, audited on October 9, 2026. Before this documentation update the
checkout was clean and contained 188 tracked files in 10 Cargo workspace members.

The repository is authoritative for what exists. `AGENTS.md`, `KEY-SYSTEM.md`,
and accepted ADRs are authoritative for intended architecture and security
boundaries. `CHECKPOINT.md` records maturity; a checked box requires working,
tested behavior, not merely a type, route, screen, or placeholder.

Evidence has deliberately narrow scope:

- The October 3 reports demonstrate two independent clients, the real server and
  PostgreSQL, certificate-validated HTTP/WSS, MLS messaging, reconnect, selected
  crash boundaries, and client/server/database restarts on an isolated LAN.
- The packaged-desktop report demonstrates an extracted Windows application
  exchanging real encrypted messages. It is separate from CLI lifecycle proof.
- Local builds, workflow contract tests, archive inspection, and YAML review do
  not prove hosted CI, publication, production signing, target-native execution on
  every platform, public-Web-PKI operation, or production readiness.
- `cargo deny check` remains blocked by RUSTSEC-2026-0173 in the OpenMLS/libcrux
  dependency tree. There is intentionally no advisory suppression.

## Current implementation

Cords is a Rust workspace with a Tauri v2/Svelte desktop client and an Axum/Tokio
server. The implemented vertical slice is encrypted server text channels:

```text
Svelte UI
  -> narrow Tauri commands
  -> cords-client-core
       -> cords-identity (account/device authority)
       -> cords-crypto (OpenMLS and local AEAD)
       -> encrypted SQLite state
       -> certificate-validated HTTPS/WSS
  -> cords-server-core
       -> signed discovery and persistent server identity
       -> challenge authentication, membership, capabilities, ownership
       -> opaque MLS/event relay and synchronization
       -> PostgreSQL
```

The server is a membership, policy, ordering, and ciphertext-delivery authority.
It is not an endpoint for message plaintext or MLS private state. The WebView is
a display surface; roots, device keys, tokens, wrapping keys, and MLS state stay
in Rust.

## Implemented flows

### Bootstrap, discovery, and trust

`cords-server` loads TOML plus `CORDS_*` overrides, validates a bare HTTPS public
origin, connects to PostgreSQL, optionally migrates or requires a current schema,
and refuses to pair an initialized database with a missing or different signing
key. It exposes separate liveness and storage-backed readiness endpoints.

The persistent Ed25519 server key signs deterministic discovery metadata at
`/.well-known/cords/server`. The client accepts an HTTPS origin, fetches and
verifies discovery, and pins the discovered signing identity. Users do not type a
fingerprint. Later authentication repeats discovery and rejects origin or signing
identity changes. Cords pinning complements normal TLS validation; it does not
replace Web PKI. Independent first-contact fingerprint comparison and future
fingerprint-bearing invites can strengthen URL-only first contact.

### Authentication and server ownership

Join and session challenges bind the server, operation, account, device,
authorization, random nonce, and expiry. Device signatures prove possession.
Challenges are persisted, short-lived, single-use, and protected by a bounded
authentication budget and per-device outstanding limit. Sessions are device-bound,
expiring, stored by hash, and invalidated by device revocation.

ADR 0003 defines first-owner bootstrap. A fresh server persists explicit `UNCLAIMED`
state, generates a random 256-bit base64url code, logs it once for the operator, and
stores only a server-bound verifier. Ordinary joining is refused until a client
redeems that code with device and account-root proofs. One PostgreSQL transaction
creates the first membership with `server.manage` and `channel.manage`, clears the
verifier, and marks the server `CLAIMED`. Local operator commands can rotate an
unused code or explicitly initialize a populated legacy database without selecting
an existing account. Ownership transfer and deletion are not implemented.

### Identity and local custody

Each installation creates separate account-root, device-signing, and MLS signing
keys. Root-signed device authorization and revocation statements are deterministic,
domain-separated, generation-aware, predecessor-linked, expiring where applicable,
and checked for rollback or substitution. Account fingerprints derive from the
root public key. A root revocation is terminal for the device identifier and
atomically invalidates its memberships and sessions.

The client uses SQLite. A random master key is protected by an OS credential
facility or Argon2id passphrase wrapping; it is not stored unwrapped beside the
database. MLS snapshots and message-cache plaintext use separate authenticated
encryption contexts. The Svelte UI receives public/display-safe view models only.
Additional-device enrollment, recovery, root rotation, and history transfer remain
unimplemented.

### Channels, MLS, delivery, and synchronization

Stable capabilities, not display roles, gate channel creation, reading, writing,
management, MLS commits, and KeyPackage publication. The server persists public
contacts, KeyPackages, channel bindings, epoch rosters, pending removals, Welcomes,
ordered ciphertext events, idempotency results, and memberships. It never receives
MLS private keys or message plaintext.

`cords-crypto::conversation::ConversationCrypto` contains all OpenMLS-specific
types. It uses the mandatory X25519/AES-128-GCM/SHA-256/Ed25519 suite and supports
KeyPackages, group creation, Welcome processing, add/remove commits, application
protection, roster inspection, and snapshot/restore. Revocation immediately stops
server delivery; cryptographic removal follows through an authenticated MLS commit.
Historical public roster snapshots support catch-up across epochs.

The client durably stores tentative MLS results, cursors, outbox work, ciphertext,
deduplication state, and encrypted display cache before exposing success. HTTP
history is authoritative. Authenticated `cords.v1.cbor` WebSocket messages only
announce route progress and trigger HTTP synchronization. Per-route sequences find
gaps; idempotency prevents duplicate durable and visible messages. Acceptance
covers offline catch-up, socket reconnect, pending-commit recovery, client process
restart, server restart, and PostgreSQL restart.

### Desktop client

`App.svelte` coordinates real native state. The identity-centric locked shell reads a non-secret
device registry and opens independent per-account SQLite vaults through native create/unlock/
migration/session commands. The UI is split into account picker, rail, sidebar,
header, timeline/composer, details, identity, settings, administration, modal,
avatar, and image-editor components. Unlock precedes the workspace. Server trust,
ownership claim, authentication, channels, MLS roster operations, sending,
synchronization, device revocation, and encrypted preferences are wired to Rust.

Donor-derived settings/admin surfaces without backend authority remain visible only
as explicit lime “Not implemented yet” states. They must not simulate success or
fabricate server data. DMs, replies, edits, deletes, reactions, attachments, calls,
invites, roles, moderation, audit, integrations, recovery, and ownership transfer
remain incomplete.

## Repository map

### Root and governance

- `AGENTS.md`: normative product and contributor specification.
- `KEY-SYSTEM.md`: normative key roles, lifecycle, and compromise boundaries.
- `CHECKPOINT.md`: ordered maturity audit.
- `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`: workspace, lockfile, and Rust
  1.97.1 toolchain.
- `deny.toml`: license, ban, source, and advisory policy.
- `README.md`: project entry points, checks, release model, and proof boundary.
- `.gitignore`, `.gitattributes`, `.dockerignore`, `.gitleaks.toml`: repository,
  archive, and secret-scanning behavior.

### Executables

- `bins/cords-server`: production server with `serve`, `migrate`, and
  `healthcheck` commands.
- `bins/cords-client/src-tauri`: desktop shell. `conversation.rs` exposes the
  narrow action/view API; `images.rs` validates native image loading; `main.rs`
  registers those commands. Configuration, build metadata, and icon families live
  beside them.
- `bins/cords-client/ui`: Svelte/TypeScript frontend. `App.svelte` coordinates
  state; `src/components` contains the donor-derived surfaces. `model.ts`,
  `status.ts`, and `images.ts` isolate mapping, availability, and crop-bound logic
  for tests. Vite, Svelte, ESLint, Prettier, and Vitest config is package-local.
- `bins/cords-dev`: line-oriented acceptance driver over the real client core,
  not an alternate protocol or client implementation.

### Shared crates

- `crates/cords-protocol`: normalized HTTPS origins, signed discovery, protocol
  negotiation, deterministic CBOR, IDs, identity/auth/membership wire structures,
  relay envelopes, messages, notifications, and fixed vectors.
- `crates/cords-identity`: roots/devices, authorization, contacts, import/export,
  revocation, generation transitions, expiry, and rollback validation.
- `crates/cords-crypto`: OpenMLS adapter and local XChaCha20-Poly1305/Argon2id
  protection. OpenMLS types do not escape this crate.
- `crates/cords-client-core`: device account registry, independent vault creation/unlocking,
  non-destructive legacy migration, discovery/pinning,
  authentication, ownership, KeyPackages, channels, roster changes, outbox,
  synchronization, encrypted history, preferences, and notification reconnection.
- `crates/cords-server-core`: persistent identity, discovery/health,
  authentication, ownership, authorization, channels, MLS coordination, history,
  and WebSocket notifications. `messaging/lifecycle.rs` isolates device/roster
  transitions; `messaging/tests.rs` provides PostgreSQL-backed integration tests.
- `crates/cords-storage`: PostgreSQL/SQLite connection, migration, readiness,
  schema, and server-identity binding primitives.
- `crates/cords-test-vectors`: embedded stable protocol fixtures.

### Schemas and vectors

PostgreSQL migrations progress from Phase 0 through server-identity binding,
messaging/authentication, revocation and epoch rosters, pending policy removals,
and singleton server ownership. SQLite progresses from Phase 0 to encrypted client
installation, trust, MLS, outbox, cursor, ciphertext, cache, and preference state.
Migration filenames are ordered under `migrations/postgres` and
`migrations/sqlite`. `test-vectors/identity-v1.json` and
`server-metadata-v1.json` pin protocol-owned encodings and signatures.

### Deployment, operations, and release

- `deploy/Dockerfile` builds the server image. Compose variants cover local,
  production-example, and isolated milestone layouts. Caddy terminates TLS/WSS;
  PostgreSQL and backend HTTP remain private in the documented topology.
- `docs/op-guide.md` covers OCI Podman/Docker and unpacked Linux tarball hosting.
  `docs/encrypted-milestone.md` is the isolated acceptance/operator procedure.
- `build-scripts` builds Linux server/client tarballs, server OCI archives,
  Windows native/cross clients, and macOS artifacts using shared fail-closed
  helpers. `scripts` contains acceptance, desktop-peer, packaging, publishing,
  signing, and release-contract tooling.
- `.forgejo/workflows/ci.yml` is commit/PR CI. `release.yml` is tag-only (`v*` or
  `V*`) plus manual orchestration for Linux x86_64/arm64, Windows x86_64/arm64,
  universal macOS DMG, Linux server tarballs, SHA-256 manifest plus detached GPG
  signature, and Cosign-signed multi-architecture OCI publication. Pubcode is
  canonical and GitHub is mirrored afterward. Source presence and local contract
  tests are not hosted-run or publication proof.
- `packaging` and Tauri icon directories contain desktop integration assets, not
  runtime authority.

### Documentation

- ADR 0001 records deterministic CBOR.
- ADR 0002 records native identity, OpenMLS, durable state, revocation, and epoch
  history.
- ADR 0003 records URL-only trust and authenticated one-time ownership claiming.
- `docs/protocol/phase-0.md` preserves the original bootstrap protocol.
- `docs/ui-integration.md` maps donor UI surfaces to Svelte and records unavailable
  controls and performance observations.
- `docs/reports` contains bounded historical evidence, not a declaration that all
  current production checkpoints pass.

## Network surface

Foundation routes are `/health/live`, `/health/ready`,
`/.well-known/cords/server`, and `/api/v1/capabilities`. Under `/api/v1`, routes
cover join/session challenges, ownership claim, device revocation, members,
KeyPackages, channels, channel binding/roster/commits/Welcomes/pending removals,
route events/history, and authenticated WebSocket notifications.

The service applies a request-body ceiling before route processing. Additional
field and cryptographic parser limits exist, though not every limit is operator
configurable. Public failures use bounded status/error categories rather than
serializing database or cryptographic internals.

## Validation and known gaps

The tree contains unit, vector, PostgreSQL integration, frontend, release-contract,
multi-client LAN, and packaged Windows evidence. The October 3 report records
passing Rust format/Clippy/tests, explicit PostgreSQL tests, frontend
format/check/lint/test/build, gitleaks, license/bans/sources, image and portable
Windows builds, real WSS messaging, zero acceptance-marker matches in database
dumps, and restart recovery. Old evidence must not be presented as a current CI
rerun.

The October 9 documentation audit reran `cargo fmt --all --check`, strict workspace
Clippy, workspace tests, the frontend format/check/lint/test/build sequence, and the
release-signing contract successfully. The workspace run passed 18 Rust tests and
left 11 PostgreSQL-dependent tests ignored because `CORDS_TEST_DATABASE_URL` was
not configured; the October 3 real-PostgreSQL reports remain the evidence for those
paths. Vitest passed 20 tests. The production frontend output was 145.47 kB of
JavaScript and 33.11 kB of CSS before gzip. `cargo deny check` was also rerun and
failed only its advisory category on RUSTSEC-2026-0173; bans, licenses, and sources
passed.

The nearest material gaps are:

1. Resolve/replace the unmaintained OpenMLS transitive dependency without weakening
   policy.
2. Obtain green hosted CI and tag-release evidence on every supported runner;
   verify Pubcode publication, GitHub mirroring, GPG, and Cosign outputs.
3. Complete production admission/membership administration, auditability,
   backup/restore and upgrade exercises, server-key rotation, and public PKI proof.
4. Complete device enrollment, recovery, root rotation, history transfer, and
   manager succession after creator revocation.
5. Implement DMs, lifecycle events, encrypted attachments, moderation, then calls.
6. Extend failure injection, fuzzing, mixed-version compatibility, accessibility,
   and long-history runtime/performance validation.

## Rules for future work

- Preserve account/device/server/MLS separation and the Rust/WebView boundary.
- Never replace normal certificate validation with Cords key pinning.
- Never regenerate a signing key over initialized server database state.
- Never invent plaintext, mock identity, or simulated security success for UI.
- Back up PostgreSQL and the signing-key volume together. Do not roll client MLS
  state backward as an application recovery technique.
- Record durable trust, encoding, ownership, or key-lifecycle changes in an ADR.
- Update this document and `CHECKPOINT.md` with implementation maturity, always
  distinguishing source, local tests, packaged runtime, hosted CI, publication,
  and production-deployment proof.
