# C0 wire, storage and compatibility audit

Source baseline: `2c4febab7594b252214c1179c81a5f6775b70d3d` (2026-10-10).
These are current implementation facts, distinct from
[ADR 0007](../adr/0007-rework-security-contracts.md)'s requested behavior.

## Existing boundaries

| Boundary | Current contract and source | Constraint for the rework |
| --- | --- | --- |
| Discovery | `ServerMetadataV1`, `cords-protocol/src/lib.rs`: protocol 1, deterministic nine-element CBOR array, `CORDS-SERVER-METADATA-V1` | Never add signed fields to this object implicitly; version/negotiation before new objects |
| Application statements | `cords-protocol/src/messaging.rs`: domain prefix + restricted deterministic CBOR map; strict `Signed<T>` and object fields | Preserve exact domains/bytes; old signatures cannot authorize a new representation |
| Identity | `DeviceAuthorization`, `DeviceRevocation`; `cords-identity::validate_transition` and independent software root/device keys | Current linear validation is not fork storage, root custody transfer or compromised-root recovery |
| Burn/departure | Separate V1 `IdentityBurn` and `ServerDeparture` domains; server-bound proofs, signed receipts, sealed retry queue | V1 burn is not portable new root-burn encoding; terminal rejection/legacy evidence mapping must be gated explicitly |
| Ownership | Root designation and successor acceptance, signed ownership generation, server-time maturity; schemas 9–10 | Preserve 30-day timing, immutable designation, operator recovery and closed bootstrap |
| Channels | Signed immutable `ChannelIdentity`, `ChannelSuccession`, `ChannelTransition`; schemas 11–12 | Mutable display metadata cannot rewrite identity, confidentiality, retirement or lifecycle heads |
| Encrypted messages | `Message` schema 1, `message.create`; `Client::sync_route` explicitly checks both after MLS decryption | Never inject edits/reactions/new inner schema into legacy routes |
| Public messages | `PublicMessage` V1 wraps contact, membership, route/event/idempotency binding and message; device-signed | Preserve route/author binding and public/encrypted distinction; new actions need negotiated encoding |
| Relay | `EventUpload`, route sequence/idempotency, WebSocket `cords.v1.cbor`; current routes are channels | DM routing/identity requires separately validated protocol and persistence, not a renamed channel |
| Native | Narrow Tauri commands → `cords-client-core::Client`; secrets stay Rust-side | UI may expose public state only; no root/session/MLS material enters TypeScript |
| Vault | Sealed client durable state, independent master key, SQLite schema 3, server-scoped sealed message cache | Preserve authenticated encryption contexts, genuine identity/signature evidence and per-server isolation |
| Archives | Sealed account-global channel identity/lifecycle and retained cache; `channel_archives` / `archived_channel_history` | Current partial channel archives are not full export, DM archives or offline undownloaded files |
| Server persistence | `PostgresStore::require_current` requires schema 12; migrations through channel succession | Older live binaries require demonstrated current-state compatibility; no downgrade of security heads |
| Future paths | Root fork/custody providers, DMs, attachment streaming and report authority are not implemented | Semantic scenarios are obligations, not simulated successful feature behavior |

Paths in the table are relative to `crates/` except the Tauri host in
`bins/cords-client/src-tauri`. Production is Svelte/Tauri; the sibling React reference
is not a network/persistence implementation. Functional predecessor paths remain.

## Capability and format gates

Current discovery features include the authenticated public-channel/lifecycle,
admission/card/page, departure/burn and ownership paths. `Client::trust` explicitly
requires supported card/page features rather than guessing older directory encodings.
New capability names, route formats, signed-byte domains, record bounds and version
selection require a scoped protocol decision before the corresponding implementation.
Server advertisement alone does not authorize new client identity or report authority.

Root-state activation must handle genuine old certificates/revocations and server-bound
burns without fabricating a portable signature. Devices/servers lacking required root
semantics receive a visible upgrade-required state; they do not silently accept new
formats. New message events need a per-route compatibility gate. Legacy history remains
readable in its original decoder, including its provenance limitations.

## C0 fixtures and evidence classes

- `test-vectors/server-metadata-v1.json` and `identity-v1.json` fix existing public
  signing bytes/signatures. Test-only keys never become deployment keys.
- `cords-test-vectors` executes typed identity vectors and tamper/domain rejection.
  The semantic manifest's JSON structure/links are checked separately; that check
  is **not** execution of future security transitions.
- `cords-storage` creates isolated schema-2 fixtures with known opaque cache/state
  bytes, injects a failed schema-3 migration, checks rollback, then retries/reopens.
  No live database/authority snapshot is restored. This is a database migration test,
  not proof that those opaque bytes are valid messages or MLS snapshots.
- `cords-client-core` existing sealed-cache, account-vault, archive/conflict, burn-retry
  and restart tests supplement structural migration evidence.
- Real PostgreSQL tests in `cords-server-core/src/messaging/tests.rs` have explicit
  ignore annotations and require `CORDS_TEST_DATABASE_URL`. Ordinary `cargo test`
  alone does not execute them. Independent TLS fixtures use separate vaults/device
  keys and explicitly trusted ephemeral certificates, with TLS 1.3 enforcement;
  production certificate validation is unchanged.
- Historical packaged/CLI reports under `docs/reports` do not establish a current
  two-packaged-client desktop baseline. Record fresh artifact hashes and observed
  native interactions separately from component, build and network-core results.

## C0 rollback boundary

No new runtime format, native command, schema, security head or behavior is introduced.
Documentation/test changes can be reverted against the same runtime. Baseline tests
operate only on temporary vaults/database fixtures and an isolated PostgreSQL service.
Do not run `encrypted-acceptance.py` with its default deployed-server restart actions
against a live community. For later migrations, use the per-unit backup/activation
record in [the plan](../rework-plan.md); restoring test copies never authorizes rewinding
live keys, authority, MLS state, counters, outboxes or cursors.
