# ADR 0002: Native identity and durable MLS channel slice

- Status: Accepted for the approved implementation milestone
- Date: 2026-09-30

## Decision

Preserve the account/device/server/MLS authority separation in KEY-SYSTEM.md.
New signed objects use distinct `CORDS-*-V1` domains followed by RFC 8949
length-first deterministic CBOR. The restricted data model admits integers,
strings, booleans, null, arrays and string-keyed maps; floats are rejected.
All maps sort keys by UTF-8 length then byte order. Existing Phase 0 metadata
remains its unchanged nine-element array. Account fingerprints hash
`CORDS-ACCOUNT-ID-V1 || root_public_key` and use unpadded base64url on the wire.
Authorization generations are account-wide, with predecessor hashes and
rejection of conflicting statements at an already observed generation.

Select OpenMLS 0.9.0 with its matching 0.6.0 RustCrypto provider, traits and
basic-credential crates. Its declared Rust 1.91 minimum fits Cords 1.97.1.
The selection follows inspection of the downloaded APIs and upstream advisories:
[extension decoding](https://github.com/openmls/openmls/security/advisories/GHSA-w62v-gv48-63rh)
and [parser bounds](https://github.com/openmls/openmls/security/advisories/GHSA-rrmv-c79f-cf5r).
The mandatory suite remains X25519/AES128GCM/SHA256/Ed25519. No experimental
protocol or secret-debug features are enabled. The adapter owns all OpenMLS types.

Use a tentative in-memory OpenMLS provider per serialized client operation.
Persist its complete self-describing snapshot, client cursor, outbox and message
changes in one SQLite transaction. No network output or successful UI notification
may escape before persistence. Discard and reload tentative state after failure.
Pending commits remain pending until durable server acceptance is recovered.

SQLite secret records use XChaCha20-Poly1305 with random nonces, independent of
MLS secrets. Each decrypted message-cache entry is separately sealed with
installation/route/message/schema context. The local random master key is
protected by OS credential storage or Argon2id passphrase wrapping (64 MiB,
three iterations, one lane). It is never stored unwrapped beside SQLite.
Cached plaintext is an explicit history-retention choice: endpoint compromise
can expose that cache. Old database pages/backups may retain encrypted prior
records; this does not provide physical secure erasure or rollback-proof storage.

Application and MLS protocol events share a durable per-route sequence.
WebSocket CBOR notifications use normal device-bound sessions and only hint at
HTTP history progress. They never authorize separately or constitute history.
Authentication lifetimes are configurable defaults, not wire constants.

Creation and issuance timestamps permit up to five seconds of future clock skew
(`cords_identity::STATEMENT_CLOCK_SKEW_SECONDS`). This is local validation policy,
not a wire-format constant or a delayed activation mechanism. The LAN validation
exposed a 64 ms host clock difference that could appear as one second with integer
timestamps. Expiration checks have **no grace interval**. Challenge expiry and
session expiry remain server-owned deadlines; signatures, single-use consumption
and generation/predecessor checks are unaffected. Operators must synchronize
clocks; larger differences fail visibly instead of broadening the window.

## Revocation and epoch history

A root-signed device revocation advances the account generation and references
the preceding signed account statement. Revocation is terminal for that device
identifier. The server persists the statement and invalidates memberships and
sessions atomically; possession of a server membership cannot override it.
Root-authorized revocation requests do not depend on the revoked device's session,
so an exact request can recover a lost acknowledgement. Client storage records
the terminal state separately from the last authorization certificate.

Channel removal first disables delivery to the selected device, then a creator
submits the actual MLS removal commit. Server delivery denial is not a substitute
for cryptographic removal. The committed epoch, roster, ordered protocol event
and coordination result change in one PostgreSQL transaction.

Each accepted epoch stores an immutable public roster snapshot. A client catching
up across several epochs validates each MLS state against the matching historical
roster, rather than incorrectly comparing an intermediate epoch with the newest
roster. These snapshots contain public identity certificates and group bindings,
never MLS secrets or application plaintext. Pre-migration epochs without a saved
snapshot fail visibly if requested; migrations cannot reconstruct missing history
by guessing an earlier roster.

## Migration and operational consequences

Forward-only migrations preserve existing Phase 0 sources and metadata vectors.
Back up database and matching server signing key together before upgrade.
Never regenerate a missing signing key for an initialized database. Restore
matching state or stop. Never roll a live client's MLS state back to a prior
snapshot as an application recovery mechanism.

The isolated `.54` deployment uses a separately trusted development CA. This
establishes LAN test trust; it is not evidence of public-Web-PKI deployment.
Full production enrollment, recovery UX and later conversation features remain
outside this milestone. An adapter test is not end-to-end acceptance.
