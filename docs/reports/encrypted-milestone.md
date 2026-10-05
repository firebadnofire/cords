# Encrypted conversation milestone evidence

Date: 2026-10-03. These results describe the uncommitted working tree and isolated
LAN deployment, not a published release or hosted CI run.

The real two-client acceptance scenario **passes**. The exact Linux OCI build
script and Windows portable build script also pass, and the extracted Windows
application exchanges authenticated MLS messages through that OCI server.
The full validation checklist is not green: the Rust dependency advisory check
still rejects an unmaintained transitive build dependency (section 22).

## 1. Implemented scope

Implemented a shared-core encrypted text-channel slice: independent installations,
identity certification, server trust, device authentication, server membership,
MLS channel coordination, durable ciphertext relay, HTTP synchronization,
authenticated WebSocket notifications, encrypted local history and crash recovery.
CLI orchestration and minimal Tauri commands use `cords-client-core`.

Evidence: [CLI acceptance](script-image-acceptance.json),
[lifecycle acceptance](lifecycle-acceptance.json), and
[packaged desktop](desktop-packaged-smoke.json). CLI and desktop evidence are
separate; the UI demonstration does not substitute for restart acceptance.

## 2. Foundation prerequisites

Restored application sources to version control visibility by replacing the broad
`/bins/*` ignore. Preserved crate boundaries and discovery vectors. Added live
database readiness and schema checks, database-bound persistent server identity,
forward migrations, and ADR 0002. Toolchain: Rust 1.97.1, Node 24, PostgreSQL 17,
SQLite, Axum/Tokio and Tauri/Svelte. Windows requires MSVC prerequisites/WebView2.

## 3. Conversation components

`cords-protocol` owns wire structures; `cords-identity` owns certification;
`cords-crypto` owns local protection and OpenMLS; `cords-storage` owns database
access; server/client core crates own services. `cords-dev` is a development CLI,
not an alternate protocol. Python runners launch independent real CLI processes.

## 4. Operational key system

Each installation generates separate account-root, device-identity and MLS signing
keys. Root authorization and device-signed MLS binding have deterministic,
domain-separated encodings and fixed vectors. Local secrets use a random master
key protected by the OS credential store or Argon2id passphrase wrapping.
Each decrypted cache record has independent authenticated encryption under local
storage protection. It does not use retained MLS epoch or message secrets.

## 5. Account and device authorization

Account fingerprints derive from root public keys; random device IDs identify
installations. Validation enforces signatures, key roles, generations,
predecessors, expiry and rollback rejection. Root revocation is terminal for that
device ID. Generation transitions and revocations serialize with server mutations.
Creation/issuance timestamps allow five seconds of clock skew; expiry has no grace.
Full multi-device enrollment and recovery UX remain deferred.

## 6. Authentication flow

Join/session challenges bind server, operation, account/device, authorization,
nonce and expiry. Proof uses the device signing key. PostgreSQL persists and
atomically consumes challenges; identical idempotent retries recover results.
Session token hashes, device binding, expiry and revocation are persistent.
Defaults are configurable: challenge 60 seconds, session 900 seconds.
Authentication has a bounded global budget and outstanding per-device challenge
limit. Errors expose stable codes rather than database or secret contents.

## 7. Independent server membership

Public admission is enabled only for this isolated test deployment. Server-signed
membership is independent of root-signed account authorization; both are checked.
Stable capabilities govern access. Channel creators manage the slice's roster;
additions require their explicit approval. Revocation immediately denies server
delivery and queues an authenticated MLS removal for an authorized creator.

## 8. MLS adapter and suite

ADR 0002 records reviewed OpenMLS 0.9 with matching 0.6 provider/basic/traits
dependencies. Library-specific types stay inside `ConversationCrypto`.
Mandatory suite: `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`.
The adapter supports KeyPackages, group creation/binding, Welcome, add/remove,
commits, application encryption/decryption and persistent recovery. Past epochs
are not retained for history caching. Public historical roster snapshots let
clients validate successive epochs during offline catch-up.

## 9. HTTP endpoints

Discovery remains `/.well-known/cords/server`. Under `/api/v1`: join/request,
auth/challenge, auth/session, devices/revoke, members, key-packages (including
read-only account retrieval), channels, channel detail/binding/roster-requests/
commits/welcome/pending-removals, and route event upload/history. Mutations use
device-scoped idempotency and transactional permission checks. KeyPackages are
reserved through roster mutations, never consumed by GET.

## 10. WebSocket frames and authorization

`/api/v1/events` requires the normal device-bound authenticated session and
`cords.v1.cbor`. Versioned CBOR route progress is
`[1, "route.advanced", route_id, sequence]`; HTTP fetches authoritative events.
Ping/pong, session revalidation, bounded frames and lag closure protect liveness.
The client subscribes before catch-up and reconnects through the same HTTP path.
There are no separate socket credentials.

## 11. PostgreSQL migrations

Version 1 foundation is preserved. Subsequent migrations add server identity
binding (2), messaging/authentication/coordination (3), revocation/removal and
epoch rosters (4), and pending policy removals (5). Accepted commits atomically
update epoch, roster, KeyPackage consumption, ordered event and Welcome delivery.
Real PostgreSQL tests run migrations twice; deployed upgrades reached schema 5
without deleting volumes. Earlier missing roster snapshots cannot be invented.

## 12. SQLite migrations and protection

SQLite version 2 adds installation metadata, encrypted durable client state,
ciphertext storage and independently encrypted message cache. FULL synchronization,
transactions and an exclusive installation lock protect cryptographic mutation.
MLS state, contiguous cursor, cache and deduplication commit together. Rust keeps
secrets outside frontend view models. The portable ZIP now includes the migrations
at the resource path used by release-mode Tauri.

## 13. Synchronization and idempotency

Per-route sequences include commits and applications. PostgreSQL assigns sequences
inside the accepted transaction and acknowledges committed results only.
Identical retries recover the original event; conflicting idempotency reuse fails.
Clients persist exact ciphertext, identifiers and resulting MLS state before
upload, reconcile self echoes and recover pending commits without re-encrypting.
Unprocessable history fails visibly instead of skipping events or resetting groups.

## 14. Exact build and deployment commands

On `.54`, in `/home/william/cords-milestone`:

```sh
bash build-scripts/linux-server-oci-img.sh
# Existing private .env selects CORDS_MILESTONE_IMAGE=cords-server:0.1.0.
docker compose --env-file .env -f deploy/compose.milestone.yaml up --no-build -d --wait
```

The deployed image is
`sha256:4d46cf047f81ecd63a9f5a887aeea89c0fd37e07020c0dbb0b548b315ead1092`.
The build ran in a source archive, so its revision label is `unknown`.
Before the switch, matching database/key backups were preserved under
`backups/pre-script-image-20261003T181450Z.dump` and
`backups/server-state-20261003T181450Z`.

On Windows, `./build-scripts/windows-client.ps1` produced the portable ZIP.
Its SHA-256 is
`6f5321bec2310aa2ab02acb6068147248b3a57c2c9c7673dfce2ba97172cf011`.
Checksum and archive contents were verified; the extracted `Cords.exe` initialized
without `CORDS_CLIENT_MIGRATIONS` or source resources. See the
[operator guide](../encrypted-milestone.md) for exact initialization/backup commands.

## 15. Origin and proxy

Origin: `https://192.168.86.54:5848`. Compose project: `cords-milestone`.
Caddy terminates TLS 1.3/WSS with an explicitly trusted development CA and IP
certificate validation. It sets HSTS and uses `default_sni` for IP clients.
Only `.54:5848` is published; PostgreSQL and backend HTTP stay private.
The separate phase-0 deployment is untouched. Persistent volumes survive restarts.

## 16. Exact independent-client commands

```powershell
cargo build -p cords-dev --features fault-injection
python scripts/encrypted-acceptance.py --client target/debug/cords-dev.exe --ca target/cords-milestone-ca.crt --crash-boundaries --report target/script-image-acceptance.json
python scripts/lifecycle-acceptance.py --client target/debug/cords-dev.exe --ca target/cords-milestone-ca.crt --report target/lifecycle-acceptance.json
```

Each runner creates independent directories, processes, random passphrases and
identities. No database or key material is cloned. The manual A/B JSON sequence,
trust procedure and state locations are in the operator guide.

## 17. Validation performed

Rust formatting, strict workspace/all-target/all-feature Clippy, workspace tests
and discovery/identity protocol vectors passed. Ten explicitly run PostgreSQL
integration tests passed: challenge rejection/replay/expiry/idempotency, device-ID
collision, authorization/revocation ordering, parser/session limits, transactional
sequence rollback, authentication budgets, root revocation/rollback, initial
binding and route permission/concurrency/restart, plus a real loopback WebSocket
test for missing/invalid sessions, missing/wrong subprotocol, oversized frames,
and revocation of established and new connections. This test uses PostgreSQL and
the real router; the LAN acceptance separately exercises certificate-validated
WSS through Caddy. Frontend formatting, Svelte
checks (zero errors/warnings), ESLint, Vitest and production build passed.
The lint dependency was updated to compatible `typescript-eslint` 8.71.0;
`npm audit` now reports zero vulnerabilities. CI includes the audit gate.

Both requested build scripts passed. Compose is healthy. Gitleaks source scanning
passed with zero leaks; license/bans/source checks pass. Rust advisories still fail
as described below. Hosted CI, publication, code signing and other desktop
platform packages were not validated by this LAN exercise.

Packaged desktop device `01a10301-0ed4-7541-adde-410ba0bde8a4` joined route
`01a10301-e5cd-7f00-939a-8d277659373a`. Its UI sent event
`01a10302-fa5a-7bb1-979a-e8c6ede4d2d5`, independently decrypted exactly once by
the CLI, and visibly displayed reply `01a10302-fe19-7aa1-9e56-59d9c72f9336`.
This was the extracted release executable, not a debug or mocked frontend.

## 18. Database plaintext proof

The script-image run scans a consistent logical dump for every one of nine
acceptance markers. All nine match counts are zero. Forms include raw UTF-8,
base64/base64url with and without padding, hex case variants, JSON Unicode,
SQL quote/backslash escaping, octal and byte escapes. Lifecycle testing separately
reports four zero counts. Dump contents and unlock material are not in reports.
This demonstrates absence of these markers, not absence of all possible metadata.

## 19. Reconnect evidence

B's WebSocket was explicitly disconnected. One offline event was fetched through
HTTP after reconnect and displayed exactly once. Live messages were asserted to
arrive through WebSocket-triggered synchronization. Lifecycle B separately caught
up five events (two commits and three messages) across add/remove epochs.

## 20. Client restart and MLS continuity

Both independent processes were killed and reopened on their existing storage.
Account/device IDs, pins, displayed history and cursors were asserted unchanged;
the same MLS conversation continued. Pending commit acknowledgement recovery and
application crashes after durable outbox persistence and after server response
also passed. Recovered application sequences were 9 and 10. Both final cursors
were 10: one commit plus nine application events, each message displayed once.

## 21. Server and PostgreSQL restart continuity

The acceptance runner killed/restarted Cords and separately restarted PostgreSQL,
preserving volumes, memberships, channel, sequence and history. Messaging continued
after both. All three explicit fingerprint observations are identical:
`d4n8X2WgxvSlPAaDyIH-7-9gYeFXgD4QgZlj7cXikrU`.
The latest acceptance route is `01a102fe-c209-7df0-967b-0982576880bf`.

## 22. Remaining gates and limitations

`cargo deny check` still fails on
[RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173):
`proc-macro-error2` is unmaintained, with no patched version. It is reached through
`hax-lib-macros`/`hax-lib`/libcrux in the OpenMLS provider dependency tree.
No advisory suppression or replacement cryptographic implementation was added.
This remains an unresolved dependency-policy gate, not a passing check.

The desktop is deliberately minimal. Management remains creator-only; creator
revocation can leave no available manager. Re-adding the same removed installation
does not have a complete client workflow. Periodic self-update scheduling, full
enrollment/recovery, production admission policies and automatic management UX
remain incomplete. Limits exist, but not every limit is configurable. Exhaustive
arbitrary crash-point testing is not claimed by the named acceptance cases.
No public-PKI, published artifact,
signed installer or hosted CI success is claimed. DMs, attachments, calls,
reactions and federation remain deferred.

**Milestone question:** Can two genuinely independent Cords clients, using the real
Cords account/device key system and MLS conversation encryption, authenticate to
the Cords server hosted on `192.168.86.54`, exchange encrypted messages using HTTP
plus WebSocket, recover missed/history events after reconnect, survive client and
server restarts, and continue the same conversation afterward? **Yes**, as shown
by the real-server acceptance evidence above. That answer does not mark every
production checkpoint or the remaining dependency-policy gate complete.
