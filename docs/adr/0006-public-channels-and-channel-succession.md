# ADR 0006: True public channels and immutable channel succession

- Status: Accepted by explicit product requirements
- Date: 2026-10-10
- Supersedes AGENTS.md sections 5, 6, 11.1 and 13.1 only for explicitly public
  channels: these routes intentionally disclose message bodies to their server.
  Encrypted channels and DMs retain their existing confidentiality boundary.
- Supersedes KEY-SYSTEM.md's blanket server-plaintext exclusion only for public
  messages. All account, device, MLS, server and local-vault key roles remain separate.
- Extends ADR 0005's metadata-only server archive with independently retained,
  read-only channel archives. Server archive/remove still purges active caches,
  but does not erase a channel archive already retained in the sealed vault.

## Decision

Channel creation has a permanent `encrypted` or `public` confidentiality mode,
defaulting to encrypted. Public messages are device-signed plaintext over verified
HTTPS/TLS; they never use MLS state or a shared encryption key. Server-side read,
write and management capabilities remain distinct from confidentiality. Currently
all active admitted members with `channel.read`/`channel.write` can read/write public
channels. Anonymous reads and additional per-channel role overrides are deferred
until the existing authorization architecture implements them. A visible encrypted
channel continues to require MLS membership for delivery.

The pinned server certifies an immutable channel identity. Clients retain that
signed record and monotonic signed lifecycle state in the encrypted account vault,
including after server removal, retirement and restart. Existing local MLS state
also establishes encrypted confidentiality during migration. Contradictory modes,
identity substitution, reactivation or lifecycle rollback raise **Channel Identity
Conflict** and cannot overwrite trusted state. Missing signed identity metadata
requires upgrading the server; it never implies a public route.

PostgreSQL schemas 11–12 retain all channel rows as durable tombstones, rejects
changes to IDs/modes and established identity fields, and prevents MLS state on
public channels. Existing rows migrate as encrypted without changing IDs, epochs,
keys or ciphertext. The legacy creation timestamp is explicitly zero (unknown).
No API edits confidentiality or physically deletes identity records.

An atomic, device-signed replacement request binds the original server-certified
identity, distinct proposed UUIDv7 successor, display name, new mode, root-certified
initiator, server-issued membership, issuance time and nonce. The server adopts
the proposed fresh UUID as its authoritative channel ID only after checking
current owner/create authority, scoped channel management, session/device state
and the durable owner record. This scoped exception to server-generated channel
IDs lets the initiator sign both IDs before the transaction, without a second
allocation/creation system. ID collision, stale (>300 seconds) new requests,
pending MLS roster work and already retired originals fail. Exact accepted retries
return the original result using existing idempotency storage. The server's signed
lifecycle record authenticates acceptance, while the independent initiator
signature prevents a server from inventing a succession claim.

Replacement preserves the predecessor's confidentiality and retires participation;
it creates an empty route and fresh MLS group if encrypted. No messages, keys,
rosters or permissions are copied. Same display names are allowed. Clients verify
the original identity, initiator authorization chain, initiating management
capability, signature, server membership and signed acceptance independently.
Historical succession certificates are checked at the signed issuance time so
later certificate expiration does not invalidate an accepted transition.

Every encrypted-to-public successor requires an explicit, durable acknowledgement
in client-core before sending. Selecting a successor or receiving its history
does not acknowledge it. Retired channels are read-only in core and server;
the UI exposes separate replacement, retirement, warning and Archives controls.

## Archives and limitations

On observing authenticated retirement, each device retains only previously
received messages in the existing sealed cache and records archive status in
its sealed durable vault state. Bodies are not copied into the main state blob. Identity,
server, name, mode, original public roster/group binding where retained, signed
succession and retirement state accompany the partial archive. Retention does
not fetch missing history or decrypt/re-publish old messages. Archive copies and
new cached relay envelopes are encrypted at rest with the existing independent
local master key; public messages are plaintext only in the transport application
payload and on the server. Old encrypted relay cache entries remain ciphertext.

Archives survive removing/archiving that server. Removing the entire local account
still deletes its vault. Existing cache policy retains received history; configurable
retention, full history export/backup, independent server/DM message archives and
retained per-message MLS cryptographic transcripts are deferred. Archive UI says
partial local history rather than claiming complete or independently re-verifiable
history. Current decrypted cache bodies retain the original validated device and
account IDs; signed channel/transition metadata preserves the confidentiality
record. Available public relay signatures remain in the sealed relay cache;
server removal preserves cache rows belonging to retained channel archives. Archives do not acquire old group secrets or attachment keys.

Signatures prove authorship, not confidentiality or honest server availability.
Public servers may read, index, copy, search or retain bodies; search/index APIs
are not introduced here. TLS certificate validation and server pinning remain
mandatory. Server capability claims retain the existing server-authority trust
boundary; no independent global role transparency system is invented.

## Deployment and validation

Upgrade desktop and server together. Back up PostgreSQL, its matching server key,
and each client vault before the forward migration. Rollback requires restoring
matching pre-migration artifacts; do not run old clients against new metadata or
roll back live MLS state. See [protocol](../protocol/public-channels.md).

Tests cover real PostgreSQL migration/rejection/idempotency and independent
clients over real TLS 1.3 with isolated trust, plus encrypted SQLite restart and
conflict rejection and desktop component checks. These are distinct from a
packaged desktop smoke, production deployment or full archive backup proof.
