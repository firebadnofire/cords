# Cords project dissection

## Encrypted milestone working-tree update — October 3, 2026

The historical snapshot below is preserved. Start with the current
[guide](docs/encrypted-milestone.md), [report](docs/reports/encrypted-milestone.md)
and ADR 0002 for the encrypted-channel slice. Sources under `bins/` are no longer
hidden by a blanket ignore rule. This work is uncommitted, not a released or
hosted-CI-validated revision.

`cords-protocol/src/messaging.rs` owns signed identity/authentication/channel
contracts; `cords-identity` owns root/device authority; `cords-crypto` owns MLS
and local AEAD protection. `cords-client-core/src/client.rs` provides the shared
native installation, durable outbox and synchronization implementation.
`cords-server-core/src/messaging.rs` and `messaging/lifecycle.rs` implement
PostgreSQL-backed authentication, memberships, coordination, history and WSS.
`bins/cords-dev` orchestrates this core without reimplementing the protocol.
Tauri's `conversation.rs` and `App.svelte` show real conversations; the previous
UI is preserved as `Discovery.svelte`.

PostgreSQL migrations advance through schema 5: identity binding, messaging,
revocation/epoch snapshots and queued policy removals. SQLite schema 2 contains
protected installation state and separately encrypted cache records.
`scripts/encrypted-acceptance.py` performs actual client/server/PostgreSQL restarts
and dump scans. `scripts/lifecycle-acceptance.py` verifies offline epoch processing,
member removal and root revocation. No broad implementation phase is implied complete.

Both requested native build scripts were exercised: the Linux OCI script on `.54`
and the Windows portable ZIP script locally. The ZIP now carries SQLite migrations
and its extracted release application exchanged real MLS messages through the
script-built server image. The 22-part report records artifact hashes and separates
this UI proof from the full CLI restart/marker-scan acceptance. Frontend audit is
clean after a compatible lint dependency update; the Rust advisory gate still
rejects the unmaintained transitive `proc-macro-error2` dependency.

This document is an onboarding map for agents and contributors working in the Cords repository. It describes the repository at commit `baaff7f` (`main`, tagged `v0.0.2`) as it actually exists on September 29, 2026, then relates that implementation to the long-term product specification in `AGENTS.md`.

The most important rule when using this document is to distinguish three different kinds of truth:

1. **Tracked implementation** is code or configuration present in the current Git tree.
2. **Declared Phase 0 structure** is referenced by manifests, documentation, or the lockfile, but is not necessarily present in the current Git tree.
3. **Future architecture** is normative product direction from `AGENTS.md`; it is not implemented merely because the specification describes it.

Do not infer missing code from the specification. Do not claim an end-to-end path works until the tracked application binaries, migrations, frontend, and runtime validations exist and pass.

## Working-tree implementation update — September 29, 2026

The repository snapshot described below remains accurate for commit `baaff7f`. The current working tree now contains the smallest Phase 0 repair needed to make that declared foundation executable:

- the missing server binary, Tauri desktop shell, Svelte UI, PostgreSQL and SQLite migrations, and external discovery test vector;
- corrected ignore rules so those source files are visible to Git;
- a server command surface for `serve`, `migrate`, and `healthcheck`, with explicit configuration precedence and persistent server identity;
- a native client discovery command that uses normal Web PKI validation and verifies signed metadata before presenting it to the UI; and
- a fixture-labelled three-pane UI whose messaging controls remain disabled because the message data plane is not implemented.

Local validation passes workspace metadata, formatting, Clippy, Rust tests, frontend formatting/type/lint/unit/build checks, npm audit, cargo-deny, Tauri development launch, and a Windows release build without bundling. The security scan initially found RUSTSEC-2026-0285 in `rustls 0.23.43`; the lockfile now selects the fixed `rustls 0.23.45` and the scan passes.

An isolated `cords-phase0` Compose deployment on the development Docker host passed startup, readiness, request-size rejection, migration idempotency, non-root image, server restart, full Compose recreation without volume deletion, and server-identity/signature persistence. That deployment intentionally exposes only loopback HTTP for diagnostics. A real desktop-client connection to a remotely trusted HTTPS origin is still unproven; no certificate validation was bypassed and no host trust store was altered.

`CHECKPOINT.md` remains unchanged. In particular, the clean-checkout and hosted cross-platform CI claims are not complete merely because the working tree now contains the required files and local/remote validation passed.

## Executive summary

Cords is intended to be a self-hosted, multi-server communications system with a Tauri desktop client, an Axum server, PostgreSQL server storage, SQLite client storage, and Messaging Layer Security (MLS) end-to-end encryption. Servers are policy and ciphertext-relay authorities; authorized client devices are the only intended holders of message and attachment plaintext.

The tracked implementation is much smaller than that target. It currently contains:

- shared protocol types for strict HTTPS server origins, version negotiation, public errors, and signed server discovery metadata;
- a client-core service that fetches and verifies discovery metadata over normal Web PKI HTTPS;
- server-core code that creates or loads a persistent Ed25519 server key, signs discovery metadata, and builds a small Axum router;
- PostgreSQL and SQLite connection/migration lifecycle adapters;
- placeholder crates for native identity, MLS/attachment cryptography, and shared test vectors;
- Docker, Compose, Caddy, CI, release, ADR, and packaging configuration intended to support Phase 0.

The repository is currently structurally incomplete. `Cargo.toml` declares the following workspace members, but none is present in `HEAD`:

- `bins/cords-client/src-tauri`
- `bins/cords-server`
- `bins/cords-client/ui` (referenced by documentation and workflows)

The PostgreSQL and SQLite migrations and the external test-vector fixture are also absent. Root `.gitignore` rules ignore the contents of `/bins`, `/migrations`, and `/test-vectors`, which explains why newly created files under those paths can be omitted unless the ignore policy is corrected or files are force-added. This is not a sparse-checkout condition.

As a result, `cargo metadata`, workspace formatting, Clippy, tests, the container build, the frontend checks, and AppImage packaging cannot currently run from a clean clone. Fixing this repository-integrity problem is the necessary first step before treating Phase 0 as operational.

## Authority and scope

### `AGENTS.md` is the product specification

Despite its filename, root `AGENTS.md` is not merely agent etiquette. It is the 1,700-line normative product and technical specification. It defines:

- the product and threat model;
- identity and key separation;
- server discovery and trust transitions;
- authentication and membership;
- MLS conversation design;
- message envelopes, attachments, transport, and storage;
- crate boundaries and deployment requirements;
- test expectations and implementation phases;
- the execution contract for agents.

Read the relevant portions before substantial work. A change to a trust boundary, major dependency, key hierarchy, wire encoding, storage model, or deployment model requires an Architecture Decision Record (ADR). The specification explicitly forbids silently redefining the architecture in code.

There is no root `VISION.md` in the current tree. If one is introduced later, project instructions require it to be read before planning substantial changes.

### Current phase

The repository identifies itself as **Phase 0: Repository foundation**. The specification's Phase 0 deliverables are a workspace and crate boundaries, a Tauri shell with a basic three-pane UI, an Axum health endpoint, PostgreSQL and SQLite adapters, Docker startup, protocol negotiation, CI gates, ADRs, and test vectors.

Only the shared crates and supporting configuration are tracked at present. The binary crates, frontend, migrations, and vector fixture needed to complete and validate Phase 0 are absent. Phase 1 and later features must not be implemented opportunistically while repairing Phase 0 unless the task explicitly expands scope.

## Architecture: intended boundaries and current reality

### Intended trust split

The target architecture has a deliberately narrow division of authority:

```text
Tauri/Svelte presentation layer
        |
        | narrow commands and display-safe view models
        v
Rust client core -- identity, sessions, MLS, local storage, synchronization
        |
        | TLS 1.3 HTTPS and secure WebSocket
        v
Axum server -- admission, permissions, ordering, retention, ciphertext relay
        |
        +-- PostgreSQL metadata/ciphertext
        +-- encrypted attachment blobs
```

The WebView is not trusted with long-lived private keys or raw MLS state. The server is not a message-encryption endpoint and must never receive message or attachment plaintext merely as a development shortcut.

### Currently implemented request path

The only meaningful client/server protocol path implemented in tracked Rust code is signed server inspection:

```text
User-provided origin
  -> ServerOrigin::parse
       requires a bare HTTPS origin and normalizes host casing
  -> reqwest GET /.well-known/cords/server
       uses normal certificate validation; no insecure fallback exists
  -> deserialize SignedServerMetadataV1 from JSON
  -> recompute server_id from the included Ed25519 public key
  -> deterministic CBOR encoding with a domain-separation prefix
  -> verify the Ed25519 signature
  -> require protocol v1 overlap
  -> return an InspectedServerViewModel marked signature=valid, trust=not_pinned
```

This proves that the response is internally consistent with the included signing key. It does **not** pin the key to the origin. Persistent pinning and key-change handling are Phase 1 work.

On the server side, `ServerIdentity` loads or atomically creates a raw 32-byte Ed25519 signing key, derives its server ID, signs Phase 0 metadata, and places that metadata in an Axum `AppState`. The router exposes health and metadata endpoints with request IDs, tracing, a 15-second timeout, a 1 MiB body limit, and panic catching.

The tracked router always installs `/health/ready`. Its comment says readiness is added after storage startup succeeds, but the absent server binary is the component that would have to enforce initialization order before constructing or serving the router. That readiness contract therefore cannot be verified from the tracked tree.

## Core concepts every new agent needs

### Portable account, device, server, and conversation identities are different

Cords deliberately separates cryptographic roles:

- The **account root** is a client-held Ed25519 key that certifies devices and recovery actions. A server must never possess it.
- A **device identity** belongs to one installation and has its own signing/encryption material. Device authorization and revocation are account-signed and generation-numbered.
- The **server signing key** authenticates server metadata, memberships, moderation state, and key rotations. It does not replace TLS.
- **MLS keys and leaves** belong to devices in individual encrypted conversations. They must not reuse account, device, server, or OpenPGP key material.
- **OpenPGP** is an optional attestation mechanism for the native account root. It is not chat encryption and must not be required for normal use.

Only the server signing identity is partially implemented today. `cords-identity` is still a placeholder.

### TLS identity and Cords server identity are complementary

Normal Web PKI validates that the client reached the requested HTTPS origin. The long-lived Cords signing key gives that server a persistent application identity. The first accepted signing key is eventually meant to be pinned locally; later changes require an old-key rotation statement, a fingerprint in a trusted invite, or a prominent user-approved reset.

Never disable certificate validation, auto-accept a changed server key, or treat the self-signature alone as origin authentication. The local Caddy configuration uses an internal CA only as an explicitly trusted development example.

### Deterministic signed encoding

JSON text is not signed. `ServerMetadataV1::signing_bytes` produces:

```text
"CORDS-SERVER-METADATA-V1" || CBOR definite-length array(9)
```

The nine values are encoded in this exact order:

1. `protocol_min`
2. `protocol_max`
3. `server_id`
4. `server_name`
5. `api_base`
6. `websocket_path`
7. `server_signing_key`
8. `join_policy` as a definite-length string array
9. `features` as a definite-length string array

This is the decision recorded in ADR 0001. Adding or reordering fields is not a casual struct edit: it needs a new protocol version and stable vectors. General application code must call the protocol-owned encoder rather than constructing signing bytes independently.

### Server IDs

The current server identifier is unpadded URL-safe base64 of:

```text
SHA-256(server Ed25519 public key bytes)
```

Metadata verification first checks that this derived value matches `server_id`, then verifies the signature. This binds the advertised fingerprint to the advertised key.

### Protocol and application versions are independent

`PROTOCOL_V1` is currently `1`. `CapabilitiesV1::negotiate` selects the highest version in the intersection of client and server ranges. Package version `0.1.0`, release tags such as `v0.0.2`, and protocol version `1` are separate values and must not be conflated.

Feature support is meant to be explicit. The current server metadata and capabilities advertise empty feature lists and empty join policies because Phase 0 implements discovery only.

### Membership and authorization

In the target system, the server controls membership, roles, channel access, moderation, and delivery. The account controls which devices belong to the user. A request needs both a valid account/device chain and a valid server membership credential.

Permissions use stable capability strings such as `channel.write`; display role names such as `admin` must not be protocol authorization checks. Authorization belongs in `cords-server-core`, not only in Axum handlers. None of this is implemented in the tracked Phase 0 code.

### MLS conversations

Each encrypted channel is one MLS group. Each direct message is one MLS group containing devices from exactly two accounts. Devices—not abstract users—are MLS members, so one two-person DM may contain several leaves.

The target mandatory initial suite is `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`. Application code must use an internal `ConversationCrypto` interface instead of spreading OpenMLS types across the repository. The actual MLS library selection remains an open ADR-level decision.

The server may queue desired roster changes and lease commit work, but a current authorized client creates and validates the cryptographic commit. The server must never add itself as a hidden MLS member. A newly added device receives current and future keys, not automatic access to old history.

`cords-crypto` currently contains no MLS implementation.

### Relay envelopes versus encrypted events

The target protocol separates server-visible routing data from MLS-protected content:

- The **outer envelope** includes protocol version, event and route identifiers, sender member/device IDs, timestamps, encoding, ciphertext, and an idempotency key. The server adds route sequence and receipt time.
- The **inner event** includes message ID, account/device sender identity, event kind, body or payload, replies, edits, reactions, attachment descriptors, and client capabilities.

The server must not require plaintext message type, body, filename, reaction value, or reply target. Edits and deletes are immutable new events referencing prior IDs rather than database mutation of plaintext messages.

These schemas are future work and are not present in `cords-protocol` yet.

### Storage ownership

The target client uses SQLite for bookmarks, pinned identities, credentials, conversation metadata, ciphertext, optional encrypted plaintext cache, retries, identity state, and MLS state. Private keys, MLS state, attachment keys, and recovery material must be encrypted with a local master key held in an OS credential store, hardware provider, or passphrase wrapping. Storing the master key beside the database is forbidden.

The target server uses PostgreSQL for policy, membership, routing, ciphertext envelopes, inventory, moderation, and pending roster work. Encrypted attachment blobs live in persistent storage behind an abstraction. The server may delete retained ciphertext but cannot force recipients to erase downloaded copies.

The tracked `cords-storage` crate provides only connection, migration, schema-sentinel, and health operations. It has no domain repositories or schemas in the current tree.

### Public errors and secret-safe observability

`PublicError` establishes the Phase 0 shape of display-safe failures: stable code, human-readable message, request ID, retry classification, and optional structured details. Retry classes are `never`, `retry`, and `reconnect`. The type is not yet integrated into the Axum routes.

Logs must exclude private keys, tokens, invite/password material, plaintext messages, attachment keys, raw MLS state, recovery bundles, and full ciphertext bodies by default. Metrics must avoid high-cardinality user identifiers. Internal database errors and stack traces must not be returned to clients.

### Idempotency and restart safety

The future protocol requires every mutating HTTP request and uploaded event to support an idempotency key. Repeating the same authenticated device/route/key tuple must return the original result without duplicating state. Background jobs and migrations must be restart-safe. These are design requirements, not current implementation.

## Repository file-by-file map

### Root files

#### `AGENTS.md`

Normative product and technical specification. It is the authority for architecture, security boundaries, phases, and contributor behavior. The file currently lists license selection as an open decision and describes `GPL-2.0-only` as proposed, while the actual workspace and license file use `AGPL-3.0-only`; this discrepancy requires an owner decision and coordinated documentation/configuration change rather than a silent edit.

#### `Cargo.toml`

Root Cargo workspace manifest using resolver version 3 and Rust edition 2024. It declares seven tracked library crates plus two absent binary members. It centralizes package metadata, dependency versions, and lints.

Important dependency choices include Axum/Tokio for the server, reqwest with rustls and no default features for HTTPS, sqlx with rustls/PostgreSQL/SQLite/migrations, `ed25519-dalek` for signatures, `minicbor` for controlled encoding, and `zeroize` for secret buffers. Workspace Rust lints forbid unsafe code and warn on missing `Debug` and unnecessarily public symbols; Clippy warns on all/pedantic rules and on `unwrap`/`expect`.

#### `Cargo.lock`

Generated exact dependency resolution for applications and libraries. It still contains package records for the absent `cords-client` and `cords-server`, including Tauri dependencies. This indicates that a fuller workspace existed in a build context when the lockfile was generated, but a lockfile is not source code and cannot substitute for the missing manifests/files. Update it only through Cargo after restoring a valid workspace.

#### `rust-toolchain.toml`

Pins Rust `1.97.1` with the minimal rustup profile. CI and Docker repeat this version. Components such as rustfmt and Clippy are installed explicitly by workflows rather than by this file.

#### `deny.toml`

Policy for `cargo deny`: deny yanked dependencies, wildcard registry dependencies, unknown registries, and unknown Git sources; allow a defined license set; warn on duplicate versions. It contains explicit RustSec exceptions for Tauri's `urlpattern`/Unicode dependency and the archived GTK3/WebKit stack. These exceptions are documented technical debt and should be removed when upstream dependency changes make that possible, not copied to unrelated advisories.

#### `.gitignore`

Ignores build products, frontend dependencies/output, generated Tauri files, environment files, and SQLite files. It also currently ignores all contents beneath `/bins`, `/test-vectors`, and `/migrations`. Those last three rules conflict with the declared repository structure and have contributed to an incomplete tracked tree. Correcting this needs care so only source, fixtures, and migrations—not build outputs or secrets—become tracked.

#### `README.md`

Operator/contributor overview with server/client commands, validation commands, security notes for local Caddy and development PostgreSQL credentials, and a statement of the validation boundary. Its description of a tracked Tauri shell, fixture-backed timeline, server executable, migrations, and successful Compose path is not supported by the current Git tree. Treat those passages as intended Phase 0 behavior until the missing files are restored and checks rerun.

#### `compose.yaml`

Three-line root Compose entry point that includes `deploy/compose.yaml`, allowing `docker compose` to be run from the repository root.

#### `LICENSE`

Full GNU Affero General Public License version 3 text. This agrees with the workspace's `AGPL-3.0-only` metadata and Docker image label, but not with the older proposed-license wording in `AGENTS.md`.

### Protocol crate: `crates/cords-protocol`

#### `crates/cords-protocol/Cargo.toml`

Declares dependencies for base64url encoding, Ed25519 verification, deterministic CBOR encoding, JSON/Serde, SHA-256, errors, and URL parsing. Test-only dependencies cover known-vector hex and JSON.

#### `crates/cords-protocol/src/lib.rs`

Owns the implemented public Phase 0 protocol surface:

- `PROTOCOL_V1` and `SERVER_METADATA_DOMAIN_V1` constants;
- `ServerOrigin`, which accepts only bare HTTPS origins without credentials, path, query, or fragment and normalizes host casing/IPv6 formatting;
- `ServerMetadataV1` and its deterministic signing encoder and verifier;
- `SignedServerMetadataV1`, flattened for the JSON discovery response;
- `CapabilitiesV1` and highest-overlap negotiation;
- `RetryClass` and `PublicError`;
- `server_id_from_public_key`;
- `ProtocolError` with deliberately non-secret error messages.

Its unit tests cover strict origin handling, signature/fingerprint binding, tamper rejection, exact deterministic encoding bytes, and version negotiation. The exact vector is embedded as a test assertion even though the separately referenced JSON vector file is absent.

### Client core: `crates/cords-client-core`

#### `crates/cords-client-core/Cargo.toml`

Depends on the protocol crate, reqwest, Serde, and `thiserror`. It intentionally has no UI toolkit dependency.

#### `crates/cords-client-core/src/lib.rs`

Defines `ServerInspector`, which fetches, validates, and converts discovery metadata to an `InspectedServerViewModel`. Its default reqwest client retains trusted TLS validation. The returned model uses camelCase field names for the UI and explicitly reports a valid signature but an unpinned trust state.

`InspectError` maps protocol, network/HTTP, and no-common-version failures to display-oriented messages. There are currently no tests in this crate, and no Tauri command in the tracked tree wires the service to a UI.

### Server core: `crates/cords-server-core`

#### `crates/cords-server-core/Cargo.toml`

Depends on Axum, protocol/signature libraries, randomness, temporary-file support, tower-http middleware, tracing, UUID request IDs, and zeroization. Tokio, Tower service helpers, and JSON are test dependencies.

#### `crates/cords-server-core/src/lib.rs`

Contains two main responsibilities:

1. `ServerIdentity` creates or loads the persistent Ed25519 key. Creation uses OS randomness, a temporary file in the destination directory, file sync, restrictive Unix mode `0600`, no-clobber persistence, and directory sync. Loading rejects a non-32-byte file and, on Unix, group/world-accessible permissions. The raw bytes are read into a `Zeroizing` buffer. Non-Unix permission validation is currently a no-op, so Windows protection depends on surrounding filesystem/ACL handling not implemented here.
2. `router` constructs Phase 0 HTTP routes and middleware. It serves liveness/readiness (`204`), signed discovery, protocol capabilities, and self metadata.

Tests verify that identity persists across reloads and produces verifiable metadata, and that readiness/discovery routes respond. They do not connect PostgreSQL or test a binary startup sequence.

### Storage crate: `crates/cords-storage`

#### `crates/cords-storage/Cargo.toml`

Depends on sqlx and `thiserror`; test support uses Tokio and temporary directories.

#### `crates/cords-storage/src/lib.rs`

Wraps `PgPool` and `SqlitePool` in `PostgresStore` and `SqliteStore`. Both can connect, run runtime-loaded migrations, and issue a health query. PostgreSQL can additionally require a singleton row in `cords_schema_metadata` as the Phase 0 schema sentinel. `StorageError` distinguishes database, migration, and migration-source failures without exposing internals as a public API response.

The SQLite test creates a temporary database and tries to run `../../migrations/sqlite`; that directory is absent, so the test cannot pass in the current tree even after the workspace-member problem is fixed.

### Reserved crates

#### `crates/cords-identity/Cargo.toml` and `crates/cords-identity/src/lib.rs`

Dependency-free placeholder for Phase 1 account roots, device authorization, fingerprints, membership verification, OpenPGP attestations, and revocation. It contains only crate documentation and an explicit unsafe-code prohibition.

#### `crates/cords-crypto/Cargo.toml` and `crates/cords-crypto/src/lib.rs`

Dependency-free placeholder for Phase 2 MLS integration, attachment encryption, secret-wrapping interfaces, and cryptographic vectors. It also explicitly forbids unsafe code. No cryptographic primitive or adapter exists yet.

#### `crates/cords-test-vectors/Cargo.toml` and `crates/cords-test-vectors/src/lib.rs`

Placeholder library for stable public vectors. It exports `SERVER_METADATA_V1_VECTOR_PATH`, pointing to `test-vectors/server-metadata-v1.json`. That file/directory is absent and ignored by the current `.gitignore`; the constant is not proof that a usable vector exists.

### Deployment files

#### `deploy/Dockerfile`

Multi-stage server image definition. The Rust Bookworm builder copies manifests, crates, both declared binary areas, and migrations, then builds the locked release `cords-server`. The Debian Bookworm slim runtime installs CA certificates, creates numeric user/group `4848`, prepares data/migration directories, removes package-manager binaries/state, copies the binary/migrations/config, drops privileges, declares the data volume and port 4848, and uses `cords-server healthcheck` for container health.

It follows many specification requirements, but its build currently fails at `COPY bins/...` because those paths are absent. Multi-architecture publication and immutable digest publication are workflow/infrastructure concerns not demonstrated here.

#### `deploy/compose.yaml`

Defines the supported topology:

- PostgreSQL 17 with a named data volume and `pg_isready` health check;
- a built `cords-server` with environment overrides, named server-data volume, PostgreSQL health dependency, and internal HTTP exposed only on loopback host port `4849` for diagnostics;
- optional Caddy profile exposing HTTPS on port `4848`.

The hardcoded database password is explicitly development-only. The server URL in this file matches that password. The `CORDS_SERVER__PUBLIC_ORIGIN` default points to the Caddy HTTPS origin, not the diagnostic HTTP port.

#### `deploy/compose.production.example.yaml`

An override that requires `CORDS_POSTGRES_PASSWORD` and a matching URL-encoded `CORDS_DATABASE__URL`. It prevents silently retaining the development password when deliberately applied. It is an example, not a complete production hardening guide.

#### `deploy/Caddyfile`

Reverse-proxies the configured host on port 4848 to the server and uses Caddy's internal CA. This is suitable only when the CA is explicitly trusted for local development. Production should use a publicly trusted certificate.

#### `deploy/server.toml`

Default in-container server and database configuration: listen address, public origin, display name, data directory, PostgreSQL URL, and migration path. The specification's precedence is CLI over environment over file over built-in defaults, but the absent server binary is where that merge would be implemented and tested.

#### `deploy/example.env`

Minimal operator example for public origin, server name, and a database URL with a reminder that passwords must be URL-encoded. It contains placeholders, not usable secrets.

### Documentation and ADRs

#### `docs/protocol/phase-0.md`

Concise contract for the implemented discovery/capabilities/health endpoints. It correctly says pinning, messages, join, identity, MLS, attachments, WebSockets, and RTC are not Phase 0 features. Its readiness claim depends on the missing server startup code and therefore remains unverified in this tree.

#### `docs/adr/README.md`

Defines when and how ADRs are used. Trust-boundary, wire-format, storage, cryptographic-dependency, and deployment decisions belong here.

#### `docs/adr/0000-template.md`

Template requiring status/date, context, decision, and security/compatibility/operational/migration consequences.

#### `docs/adr/0001-deterministic-cbor.md`

Accepted decision for the nine-element definite-length CBOR array plus `CORDS-SERVER-METADATA-V1` domain prefix. It intentionally selects only Phase 0 metadata encoding and does not pre-decide later identity or MLS encodings.

### Forgejo automation

#### `.forgejo/workflows/ci.yml`

Defines three Phase 0 jobs:

- `rust-and-frontend` runs in a Node 24 Debian container with PostgreSQL 17, installs Rust 1.97.1 and system dependencies, performs a manual Forgejo checkout, then runs Rust format/Clippy/tests/deny and frontend format/type/lint/test/build checks;
- `containers` builds and starts Compose, checks readiness and signed-discovery shape, and always tears volumes down;
- `client-platform-check` checks the Tauri client on Ubuntu, Windows, and macOS.

Every job path relying on the missing binaries/frontend/migrations currently fails from a clean checkout. The workflow also mentions the specification's required checks only partially: no explicit secret scan is visible.

#### `.forgejo/workflows/release.yml`

On `v*` tags, checks out the exact tag, validates/formats/tests the project, packages an AppImage, imports a release OpenPGP secret, creates an armored detached signature, verifies it, and creates or reuses a Forgejo release before uploading both assets.

The secret import accepts either an ASCII-armored `CI_KEY` or base64-decoded key material, writes it under an ephemeral `GNUPGHOME` with restrictive umask, removes the intermediate key file, and never prints the key. The release token is used for authenticated checkout and publication. Publication does not delete or replace same-named existing assets, so repeated execution against a partially populated release may need explicit conflict handling.

The workflow cannot currently build because the Tauri sources are absent.

#### `.forgejo/docs/release-workflow.md`

Human guide to release triggers, required secrets, assets, signature verification, tag creation, and the distinction between a signed Git tag and detached AppImage signature. It clearly states that local packaging does not prove hosted Forgejo runner, network, or token behavior.

### Desktop packaging assets

#### `packaging/cords.desktop`

Linux desktop entry for an executable named `cords`, with instant-messaging/network categories, no terminal, and `cords` icon/window class.

#### `packaging/cords.svg`

128-by-128 scalable application icon: a dark rounded square with a blue-purple chat bubble and three light dots. It is a hand-authored vector and has no external asset dependency.

#### `scripts/package-appimage.sh`

Fail-fast Bash wrapper that resolves the repository root, runs `npm ci` and the Tauri AppImage build in `bins/cords-client/ui`, finds the first generated AppImage, and installs it as executable `target/Cords.AppImage`. It is idempotent with respect to the destination artifact but depends on the absent frontend/Tauri project and Linux AppImage build dependencies. The Git blob has valid Bash syntax; Windows checkouts with CRLF should use an environment such as Git Bash or normalize line endings before WSL execution.

## Missing declared files and consequences

The following are referenced but absent from `HEAD`:

| Missing path | Referenced by | Consequence |
|---|---|---|
| `bins/cords-client/src-tauri/Cargo.toml` and source | workspace, Dockerfile, CI, lockfile | Cargo cannot load the workspace; no Tauri command boundary exists |
| `bins/cords-client/ui/*` | README, CI, release script | no frontend, package lock, tests, or build exists |
| `bins/cords-server/Cargo.toml` and source | workspace, Dockerfile, lockfile | no executable loads config/storage/identity or serves the router |
| `migrations/postgres/*` | Dockerfile, server config, storage design | server schema cannot be created or validated |
| `migrations/sqlite/*` | storage unit test | SQLite migration test fails |
| `test-vectors/server-metadata-v1.json` | test-vectors crate constant and ADR process | external interoperability fixture is unavailable |

Additional specification-layout items not yet present include `bins/cords-admin`, `bins/xtask`, `CHANGELOG.md`, a separate `docs/CORDS_SPEC.md`, health-check script, fuzz targets, integration-test harness, and later-phase storage/blob implementations. Unlike the first table, many of these are planned future deliverables rather than current manifest-breaking omissions.

## Validation status at this snapshot

The following read-only checks were performed while producing this document:

- `git status --short`: clean before adding this document.
- `git sparse-checkout list`: confirmed the checkout is not sparse.
- `git ls-files` and `git ls-tree -r HEAD`: confirmed the missing paths are absent from Git, not merely the working directory.
- `git check-ignore -v --no-index`: confirmed `/bins/*`, `/migrations/*`, and `/test-vectors/*` match root ignore rules.
- `cargo metadata --no-deps --format-version 1`: failed with exit 101 because `bins/cords-client/src-tauri/Cargo.toml` is missing.
- `docker compose config`: not run because Docker is not installed/available on this Windows host. Independently, the Dockerfile's missing `COPY` sources make a clean build impossible.
- Bash syntax: the repository blob is valid under Git Bash. WSL Bash against this CRLF working tree reports an end-of-file syntax error caused by line endings, not the committed LF blob.

No claim is made that Rust tests, frontend tests, a Tauri build, a container build, Compose readiness, Forgejo CI, signing, release publication, or end-user UI behavior currently works.

## Recommended workflow for the next agent

1. Read the relevant `AGENTS.md` sections and ADRs before planning.
2. Run `git status --short`; preserve unrelated user changes.
3. Determine whether the task concerns tracked Phase 0 code, restoration of missing Phase 0 files, or an explicitly authorized later phase.
4. For repository restoration, first resolve the `/bins/*`, `/migrations/*`, and `/test-vectors/*` ignore rules and recover authoritative source from history, another branch/tag, or the project owner. Do not invent replacement application architecture from the lockfile or README.
5. Restore the smallest coherent unit, then validate it before proceeding. Start with `cargo metadata`; do not attempt downstream Cargo checks while workspace loading is broken.
6. Keep protocol wire types in `cords-protocol`, identity semantics in `cords-identity`, cryptographic adapters in `cords-crypto`, persistence behind `cords-storage`, client orchestration in `cords-client-core`, and authorization/policy in `cords-server-core`.
7. Add tests with behavior. For signed structures, add exact stable vectors and tamper/error cases.
8. If a change affects trust, keys, deterministic encoding, storage, deployment, or a major dependency, add or update an ADR in the same change.
9. Run the full appropriate local check set once the workspace is loadable. Report unavailable checks and runtime boundaries precisely.
10. Never equate static/source success with Tauri UI behavior, container startup, hosted CI, signed publication, or cross-platform runtime proof.

## High-risk mistakes to avoid

- Do not treat the specification's future behavior as current behavior.
- Do not weaken TLS validation for the local internal-CA example.
- Do not accept a self-signed discovery object as a pinned server identity.
- Do not put private keys or raw MLS state in TypeScript/WebView state.
- Do not put plaintext messages or attachment metadata in PostgreSQL.
- Do not use OpenPGP for chat encryption or reuse Ed25519 keys across roles.
- Do not expose OpenMLS concrete types outside the internal crypto boundary.
- Do not authorize only in HTTP handlers; core services must enforce policy.
- Do not sign JSON or generic/ambiguous CBOR.
- Do not add password admission until an audited RFC 9807 OPAQUE implementation and interoperability tests are selected.
- Do not hide missing files or failing checks with dummy fixtures, empty migrations, weakened tests, or workspace-member removal unless the requested architectural change explicitly justifies it.
- Do not describe native server deployment as supported; Docker remains the only intended supported hosting path until native deployment documentation and tests exist.

## Phase roadmap

- **Phase 0 — foundation:** discovery, workspace, UI shell, storage lifecycle, server startup, Docker, CI, ADRs, and vectors. This is the active but incomplete phase.
- **Phase 1 — identity and membership:** account/device keys, encrypted local secrets, fingerprints, server pinning, challenge authentication, admission, roles, revocation, and optional OpenPGP attestation.
- **Phase 2 — encrypted channels:** MLS adapter, KeyPackages, channel groups, roster commits/leases, synchronization, and encrypted message events.
- **Phase 3 — direct messages:** two-account MLS groups through a shared relay, global DM UI, blocking, and lifecycle handling.
- **Phase 4 — encrypted attachments:** chunked XChaCha20-Poly1305, resumable transfer, quotas, and integrity/safe-file validation.
- **Phase 5 — moderation and operations:** admin CLI/API, approvals, bans, audit, retention, garbage collection, backups, rate limits, and upgrade tests.
- **Phase 6 — RTC:** MLS-protected signaling, one-to-one audio, TURN, and security indicators; video follows audio stability.
- **Phase 7 — deferred privacy/scale:** personas, relay migration, transparency, history backup/transfer, mobile, SFU group calls, S3-compatible storage, and hardware-backed roots.

Every phase must preserve the central product promise: independently hosted servers may control community policy and relay ciphertext, but message and attachment plaintext remains on authorized client devices.
