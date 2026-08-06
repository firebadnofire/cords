# CORDS

*Right on the wire*

## Software Product and Technical Specification

**Document version:** 0.1 Draft  
**Date:** August 5, 2026  
**Primary audience:** Codex and human contributors  
**Repository model:** Rust monorepo  
**Status:** Architecture baseline for initial implementation

```{=openxml}
<w:p><w:r><w:br w:type="page"/></w:r></w:p>
```

# Contents

[[TOC]]

```{=openxml}
<w:p><w:r><w:br w:type="page"/></w:r></w:p>
```

# 1. Document purpose

This document is the implementation authority for the initial Cords codebase. It defines product behavior, security boundaries, data ownership, protocol responsibilities, repository structure, deployment requirements, and phased acceptance criteria.

Codex and human contributors MUST treat this specification as normative unless a later Architecture Decision Record, abbreviated ADR, explicitly supersedes part of it. Code MUST NOT silently redefine the architecture. When the implementation reveals a missing or incorrect requirement, the contributor MUST update this document or add an ADR in the same change.

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** express requirement strength.

## 1.1 Design summary

| Area | Initial decision |
|---|---|
| Product | Self-hosted, multi-server, real-time communication client with built-in end-to-end encryption |
| Product name | Cords |
| Tagline | Right on the wire |
| Primary language | Rust, edition 2024 |
| Repository | One Cargo workspace containing client, server, shared crates, administration tools, tests, and deployment files |
| Desktop client | Tauri v2 with a Svelte and TypeScript user interface |
| Server | Axum and Tokio |
| Realtime transport | HTTPS plus secure WebSocket |
| Calls | WebRTC, with TURN support |
| Conversation encryption | Messaging Layer Security, MLS 1.0, RFC 9420 |
| MLS implementation | Wrapped behind an internal interface, OpenMLS is the initial candidate |
| Account identity | Native Ed25519 account root and per-device keys |
| OpenPGP | Optional external attestation and hardware-backed identity integration, not message encryption |
| Server identity | Long-lived Ed25519 signing key pinned by clients in addition to normal TLS validation |
| Client database | SQLite, with cryptographic secrets encrypted at rest |
| Server database | PostgreSQL |
| Attachment storage | Encrypted blobs in local persistent storage initially, with an object-storage abstraction |
| Supported server hosting | Official Docker image and Docker Compose configuration |
| Default server port | 4848 |
| Proposed license | GPL-2.0-only, pending final project-owner confirmation |

# 2. Product vision

Cords is a Discord-shaped communication client without a single central Discord-owned account system. A user installs one client, maintains a portable cryptographic identity, and connects directly to independently operated Cords servers.

A server is represented by its HTTPS origin, for example:

```text
https://socool.com:4848
```

The client stores joined servers in a vertically scrollable sidebar. Each server controls its own membership, roles, channels, moderation, retention policy, and relay availability. The user controls their private identity keys, authorized devices, message plaintext, and cryptographic verification decisions.

Cords is not intended to hide the fact that a user connects to a server. It is intended to prevent the server from reading message contents, forging user messages, silently adding surveillance devices to encrypted conversations, or owning the user's identity.

## 2.1 Core product principles

1. **Portable identity:** A user identity is not created by or trapped inside one server.
2. **Independent communities:** Each server is independently hosted and administered.
3. **Client-held plaintext:** Message and attachment plaintext is produced and consumed by authorized clients only.
4. **Narrow authority:** Server, account, device, and conversation keys each have separate jobs.
5. **Inspectable protocol:** Public wire formats and trust transitions are documented and testable.
6. **No custom cryptography:** Cords composes established protocols and maintained libraries.
7. **Docker-first operations:** The supported server deployment is reproducible and intentionally narrow.
8. **Graceful self-hosting:** The server binary remains a normal executable even when Docker is the supported distribution.
9. **Explicit limitations:** Metadata exposure, history recovery, and device trust are shown to users rather than hidden behind misleading claims.

# 3. Scope

## 3.1 Initial release goals

The first production-capable release MUST provide:

- A desktop client for Linux, Windows, and macOS.
- A self-hosted Cords server distributed as a Docker image.
- Joining multiple servers by HTTPS origin or invite URL.
- Public, invite-only, and pre-registered server admission.
- Password admission once a reviewed RFC 9807 OPAQUE implementation is integrated.
- Portable client-controlled account and device identities.
- Server identity pinning.
- Text channels with MLS end-to-end encryption.
- Direct messages using MLS, initially relayed through a shared server.
- Message replies, edits, reactions, and deletion requests as encrypted events.
- Encrypted file attachments.
- Roles, channel permissions, kicks, bans, and invite management.
- Local message search on decrypted client data.
- One-to-one audio calls after encrypted messaging is stable.
- Protocol version negotiation and a documented compatibility window.
- Automated tests for cryptographic state transitions, protocol parsing, migrations, and deployment startup.

## 3.2 Explicit non-goals for the initial release

The initial release MUST NOT attempt to provide:

- Full Matrix-style server federation.
- Server-side plaintext search.
- Server-side content moderation of encrypted message bodies.
- Telephone network integration.
- A custom SFU or media server implementation.
- Anonymous networking comparable to Tor.
- Cryptocurrency, tokens, or blockchain identity.
- A server-resettable account password that bypasses cryptographic identity.
- Automatic decryption of history by newly added devices.
- A plugin system that executes untrusted server-provided code.
- Compatibility with Discord's proprietary protocol.
- OpenPGP encryption of chat messages.

# 4. User-facing concepts

## 4.1 Account

An account is the user's portable local cryptographic identity. It is identified by a fingerprint derived from the account root public key. The account root authorizes devices and recovery operations. The server MUST NOT possess the account root private key.

## 4.2 Device

A device is one installation of the Cords client. Every device has its own signing and encryption material. Devices are independently addable and revocable. Conversation membership is tracked at the device level because MLS members are clients, not abstract people.

## 4.3 Server

A server is an independently hosted community reachable through an HTTPS origin. It owns server membership, roles, channels, invite policy, relay services, and data retention policy. It has a persistent signing identity separate from its TLS certificate.

## 4.4 Membership

Membership is a server-signed authorization linking one Cords account and one or more of its devices to a server-local member identity and role set. A membership credential is not the user's global identity and does not grant access to unrelated servers.

## 4.5 Channel

A channel is a server-owned conversation namespace. An encrypted text channel maps to one MLS group. Channel access requires both server permission and valid MLS group membership.

## 4.6 Direct message

A direct message, or DM, is an MLS conversation involving exactly two user accounts and any number of their authorized devices. The server may relay ciphertext, but does not cryptographically own the DM.

For the initial release, every DM has one primary relay server that both parties currently share. The protocol MUST reserve space for additional relay endpoints and future relay migration.

## 4.7 Persona

The initial release uses the same account identity across servers. This permits straightforward identity portability but allows server operators to correlate a fingerprint observed on multiple servers.

Per-server pseudonymous personas are a planned privacy feature. They MUST NOT be implemented through a home-grown deterministic key derivation scheme. A later design must preserve unlinkability unless the user deliberately reveals a binding.

# 5. System architecture

```text
+---------------- Cords desktop client ----------------+
| Svelte/TypeScript UI                                 |
|          | narrow Tauri command and event API        |
| Rust core: identity | sessions | MLS | storage | sync|
+---------------------------+--------------------------+
                            | TLS 1.3, HTTPS, WebSocket
+---------------------------v--------------------------+
| Cords server: Axum and Tokio                         |
| discovery | auth | membership | ciphertext relay    |
| PostgreSQL | encrypted blobs | WebRTC signaling     |
+------------------------------------------------------+
```

The server is a delivery service and policy authority. It is not a trusted message encryption endpoint.

# 6. Trust model and threat model

## 6.1 Trusted components

A user trusts:

- Their own authorized Cords devices.
- The local operating system to protect running processes and local credential storage.
- Selected cryptographic libraries to correctly implement their protocols.
- The server only for availability, server-local permission enforcement, ordering, and retention behavior.

## 6.2 Adversaries considered

Cords MUST defend message content and authorship against:

- A curious or compromised Cords server.
- A network observer between client and server.
- A malicious server administrator attempting to substitute a user device key.
- A stolen but subsequently revoked device.
- Replay of old server membership or device state.
- Corrupted or maliciously crafted protocol messages.
- A client attempting to access a channel without server permission.
- A server attempting to add an undisclosed MLS group member.
- Attachment storage compromise.

## 6.3 Threats not fully solved

The initial architecture does not fully hide:

- Client IP addresses from the connected server.
- Connection timing and duration.
- Ciphertext size.
- Server membership and channel membership from that server.
- The identity of the authenticated device uploading an event.
- Contact relationships when a shared server relays a DM.
- Plaintext displayed by a compromised authorized endpoint.
- Screenshots, copy and paste, or manual forwarding by recipients.
- Traffic analysis by a sufficiently capable adversary.
- Denial of service by a server that refuses to deliver ciphertext.

The UI and documentation MUST describe these limitations accurately. Marketing MUST NOT claim complete anonymity or metadata protection.

# 7. Identity and key hierarchy

## 7.1 Key separation

Cords MUST use separate keys for separate protocols and roles.

```text
Native account root key
    | authorizes
    +-- device identity key
    |       | binds
    |       +-- MLS signature key
    |       +-- MLS KeyPackages and leaf keys
    |
    +-- recovery operations
    +-- optional OpenPGP attestation target

Server signing key
    +-- signs server metadata
    +-- signs membership credentials
    +-- signs role and moderation records
    +-- signs key-rotation statements
```

A private key MUST NOT be reused as both an OpenPGP key and an MLS key, even when both algorithms use Ed25519.

## 7.2 Account root

The initial account root is an Ed25519 signing key generated on the client.

```text
AccountId = SHA-256(version || account_root_public_key)
```

The exact binary prefix and encoding MUST be specified in the protocol crate and covered by test vectors. User-facing fingerprints SHOULD be grouped, checksummed, and represented in a case-insensitive alphabet suitable for manual comparison.

The account root SHOULD be used only for certification and recovery statements. Routine messages MUST be signed or authenticated using device and MLS keys.

## 7.3 Device authorization record

A device authorization record MUST include at least:

```text
version
account_id
device_id
device_signing_public_key
created_at
expires_at, optional
generation
capabilities
previous_record_hash, optional
```

The account root signs the deterministic binary encoding of the record. An existing authorized device MAY issue a provisional device authorization when the account root is unavailable, but the UI MUST distinguish provisional trust from root-certified trust.

## 7.4 Device enrollment

A new device may be authorized through one of these methods:

1. Scan a QR code from an existing authorized device.
2. Compare a short authentication string over an independent channel.
3. Import an encrypted recovery bundle.
4. Use an optional hardware-backed OpenPGP credential to attest the native account root.

A server-provided login code alone MUST NOT authorize a new device to decrypt conversations.

## 7.5 OpenPGP integration

OpenPGP, defined by RFC 9580, is an optional identity attestation layer.

The supported initial model is:

```text
OpenPGP certificate
    signs an attestation that
Native Cords account root = public key K
```

Cords MUST verify the OpenPGP signature using an embedded library such as Sequoia OpenPGP. The client MUST NOT shell out to `gpg` for ordinary protocol operation. Optional integration with `gpg-agent`, smart cards, or YubiKey devices may be added through a separate provider interface.

OpenPGP MUST NOT encrypt channel messages, DM messages, MLS state, or attachments. It MUST NOT be required for ordinary users.

## 7.6 Revocation and generation counters

Account and device state MUST be versioned by a monotonically increasing generation counter. Clients MUST reject known older state after observing a newer valid generation.

A device revocation record MUST contain:

```text
account_id
revoked_device_id
new_generation
reason_code, optional
issued_at
previous_state_hash
signature
```

The server may distribute these records but cannot create a valid one.

## 7.7 Key transparency

The initial release MAY use server-hosted append-only identity state with client-side consistency checks. A later global or cross-server transparency service is expected.

At minimum, clients MUST:

- Store the newest valid generation seen for every contacted account.
- Warn on rollback.
- Warn when a known account unexpectedly presents a different root key.
- Display device additions and removals.
- Refuse silent key substitution.

# 8. Server identity and discovery

## 8.1 Server origin

A Cords server is addressed by a normalized HTTPS origin:

```text
scheme = https
host = DNS name or IP literal
port = explicit port or 443
```

Path components are not part of the server origin.

## 8.2 Discovery endpoint

The server MUST expose:

```http
GET /.well-known/cords/server
```

Example response:

```json
{
  "protocol_min": 1,
  "protocol_max": 1,
  "server_id": "base64url-sha256-fingerprint",
  "server_name": "So Cool",
  "api_base": "/api/v1",
  "websocket_path": "/api/v1/events",
  "server_signing_key": "base64url-ed25519-public-key",
  "join_policy": ["invite", "pre_registered"],
  "features": ["mls-v1", "attachments-v1", "dm-relay-v1"]
}
```

The response MUST be signed by the current server signing key. The signature covers a deterministic CBOR representation, not the textual JSON representation.

## 8.3 TLS and persistent server identity

Clients MUST validate normal Web PKI TLS certificates. Cords server signing keys do not replace TLS.

The first accepted server signing key is pinned locally. A later key change requires one of:

- A rotation statement signed by the old server key.
- A user-approved trust reset with a prominent warning.
- A fingerprint already carried in a trusted invite link.

## 8.4 Invite URL

A standard HTTPS invite URL is preferred:

```text
https://socool.com:4848/join/<high-entropy-token>#cords-server=<fingerprint>
```

The URL fragment is not sent to the HTTP server and may carry the expected server fingerprint. The desktop client registers itself as a handler for supported invite URLs. A browser fallback page SHOULD explain how to install or open Cords.

Invite tokens MUST be random capabilities with server-side limits for expiration, remaining uses, issuer, and initial role.

# 9. Registration and authentication

## 9.1 Possession challenge

The server MUST prove that a registering client controls the submitted device private key.

```text
1. Client requests a registration challenge.
2. Server returns a random nonce, server ID, expiration, and context string.
3. Client signs the deterministic challenge object with its device key.
4. Server verifies the signature and admission requirement.
5. Server issues a membership credential.
```

Challenges MUST expire quickly and MUST be single-use.

## 9.2 Join policies

A server supports one or more policies:

- `public`
- `invite`
- `password`
- `pre_registered`
- `moderator_approval`

Policies may be combined.

### Public

Any valid device identity may request membership. Rate limits and optional moderator approval still apply.

### Invite

The client presents a high-entropy invite capability. The token is consumed or decremented only after successful proof of key possession.

### Pre-registered

An administrator records an expected account fingerprint or device key before the client connects. The pre-registration entry is converted into an active membership after signature verification.

### Password

Password admission MUST use an audited implementation of OPAQUE as specified by RFC 9807. The shared password is admission material, not the member's permanent authentication credential.

Until an acceptable Rust OPAQUE implementation and interoperability tests are selected, password admission MUST remain disabled rather than falling back to an improvised password protocol.

### Moderator approval

The server records a pending request containing only the public identity, requested display name, and admission evidence. A moderator approves or rejects it. Approval issues the membership credential.

## 9.3 Routine authentication

After registration, the client authenticates by signing a fresh server challenge with its authorized device key and presenting the current server membership credential.

Bearer tokens MAY be issued for connection efficiency, but they MUST be short-lived, scoped, revocable, and bound to a device identity. A bearer token MUST NOT become the root of account recovery.

# 10. Membership credentials and authorization

A server membership credential is a server-signed record:

```text
version
server_id
member_id
account_id
device_id or authorized device set
roles
issued_at
expires_at, optional
generation
status
```

The server is authoritative for membership status. The account is authoritative for whether a device belongs to the user. Both checks are required.

Permissions SHOULD use stable capability strings rather than hard-coded role names, for example:

```text
server.invite.create
server.member.ban
channel.create
channel.read
channel.write
channel.manage
channel.mls.commit
attachment.upload
call.start
```

Roles map to capabilities. Protocol logic MUST check capabilities, not display labels such as `admin`.

# 11. Conversation encryption with MLS

## 11.1 General rule

Every encrypted text conversation is one MLS group.

```text
Server channel -> MLS group containing authorized devices
DM             -> MLS group containing devices from exactly two accounts
```

MLS is used as a key establishment and message protection layer over Cords transport. The server stores and forwards MLS messages as opaque binary data.

## 11.2 Cipher suite

The initial mandatory cipher suite is:

```text
MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
```

A ChaCha20-Poly1305 suite MAY be negotiated later. The application MUST NOT invent a custom cipher suite.

## 11.3 MLS library abstraction

OpenMLS is the initial implementation candidate, but application code MUST depend on an internal `ConversationCrypto` interface rather than exposing OpenMLS types throughout the repository.

The abstraction MUST cover:

- KeyPackage creation and publication.
- Conversation creation.
- Welcome processing.
- Application message encryption and decryption.
- Add and remove proposals.
- Commit creation and processing.
- State persistence.
- Exported secrets where specifically required by a reviewed design.
- Error classification without leaking secret material.

## 11.4 Device-level membership

Each device is an MLS member. A two-person DM may contain several MLS leaves:

```text
Alice phone
Alice laptop
Bob phone
Bob desktop
```

Account-level user interfaces aggregate leaves by account but MUST retain device-level audit information.

## 11.5 KeyPackages

Each device publishes a limited pool of signed MLS KeyPackages to servers where it is reachable. KeyPackages MUST be single-use where required by the MLS implementation. The server MUST remove a consumed KeyPackage atomically.

Clients MUST replenish KeyPackages before the pool reaches a configurable low-water mark.

## 11.6 Channel group creation

The channel creator's device creates the initial MLS group and publishes signed group metadata tied to the server and channel identifiers.

The server MUST reject a group binding that does not match the channel, server ID, protocol version, or authorized creator.

## 11.7 Membership changes and commit coordination

The server maintains a queue of desired channel roster changes. A current authorized MLS member creates the actual cryptographic commit.

To reduce conflicting commits, the server MAY issue a short-lived commit lease:

```text
channel_id
base_epoch
lease_holder_device_id
expires_at
pending_change_hash
```

The lease serializes work but grants no new cryptographic authority. The committing client MUST independently validate the requested add or remove operation against signed server membership and permission state.

If no authorized member device is online, the roster change remains pending. The server MUST NOT add itself as a hidden MLS member to solve availability.

## 11.8 Removal behavior

When a member is kicked or banned:

1. The server immediately stops accepting new events from that membership.
2. The server stops delivering newly accepted channel ciphertext to the removed membership.
3. A removal proposal is queued.
4. An authorized current member creates a commit.
5. Clients show the removal as pending until the new epoch is confirmed.

Server-side delivery denial provides immediate practical exclusion. The MLS removal provides cryptographic exclusion for future messages.

## 11.9 Epoch updates

Clients SHOULD create periodic self-update commits so a quiet conversation does not remain in one epoch indefinitely.

Suggested triggers:

- Device addition or removal.
- Member addition or removal.
- Identity state change.
- Reconnection after a long absence.
- A configurable elapsed time.
- A configurable application-message count.
- Explicit user security action.

Exact defaults require performance testing.

## 11.10 New devices and old history

A newly authorized device receives current and future conversation keys after being added to each group. It does not automatically receive old message keys.

The initial release MUST state this clearly in the UI. Historical transfer and encrypted backups are separate features, not automatic consequences of joining MLS.

# 12. Direct messages

## 12.1 DM identity

A DM has:

```text
dm_id
participant account IDs, exactly two
participant device roster
MLS group ID
primary relay server
optional backup relay endpoints
creation timestamp
current lifecycle state
```

The DM remains cryptographically valid even if the primary relay server becomes unavailable. Delivery migration is a later operation.

## 12.2 Initial DM creation

For the initial release, users may start a DM when both are members of the same server.

```text
1. Initiator requests the other account's current contact bundle.
2. Server returns signed membership state and available device KeyPackages.
3. Initiator validates account and device authorization.
4. Initiator creates a two-account MLS group.
5. Welcome messages are uploaded through the shared server.
6. The server becomes the primary ciphertext relay.
```

The server MUST NOT claim that a server-local display name proves global identity.

## 12.3 DM relay migration

The wire model MUST allow a signed encrypted control event that adds a relay endpoint and later marks another endpoint primary. Actual cross-server relay migration is deferred until after the initial release.

## 12.4 Blocking

Blocking is a local and cryptographic action:

- The client stops displaying and sending messages.
- The relay server is asked to stop delivery when possible.
- The blocking client proposes removal or termination of the DM group.
- The UI warns that already delivered content cannot be remotely erased.

# 13. Message and event model

## 13.1 Outer relay envelope

The server-visible envelope contains routing and reliability data only:

```text
protocol_version
event_id
route_kind: channel or dm
route_id
sender_member_id
sender_device_id
client_created_at
content_encoding
ciphertext
idempotency_key
```

The server assigns:

```text
server_sequence
server_received_at
```

The server MUST NOT require plaintext message type, body, filename, reaction value, or reply target.

## 13.2 Inner encrypted event

The MLS-protected content contains:

```text
schema_version
message_id
sender_account_id
sender_device_id
client_timestamp
event_kind
body or event payload
reply_to, optional
edit_of, optional
attachment descriptors
client capabilities
```

Supported initial event kinds:

- `message.create`
- `message.edit`
- `message.delete`
- `reaction.add`
- `reaction.remove`
- `member.notice`
- `call.offer`
- `call.answer`
- `call.ice`
- `call.end`

Edits, reactions, and deletions are new immutable encrypted events referencing prior message IDs.

## 13.3 Identifiers

Identifiers SHOULD be random or UUIDv7 values generated by the authoritative endpoint. Database auto-increment values MUST NOT appear as public identifiers.

- `event_id`: generated by the sending client for idempotency.
- `message_id`: generated by the sending client.
- `channel_id`: generated by the server.
- `member_id`: generated by the server.
- `device_id`: generated by the client.
- `server_id`: hash of the server signing public key.
- `account_id`: hash of the account root public key.

## 13.4 Ordering

The server sequence is the delivery order for one route. Clients MUST tolerate delayed and duplicate application events.

MLS handshake messages and commits require stricter epoch processing. A client that detects a gap MUST request missing route events before applying later handshake state.

## 13.5 Idempotency

Every mutating HTTP request and uploaded event MUST support an idempotency key. Repeated submission with the same authenticated device, route, and idempotency key MUST return the original result rather than duplicating state.

## 13.6 Size limits

Defaults are configurable. Initial recommended values:

- Relay envelope: 1 MiB maximum.
- Text body after decryption: 64 KiB maximum.
- Attachment: 100 MiB maximum per object.
- Server display name: 100 Unicode scalar values.
- Channel name: 100 Unicode scalar values.
- User display name: 100 Unicode scalar values.

Limits MUST be enforced before expensive parsing or allocation.

# 14. Attachment encryption

Attachments are encrypted on the client before upload.

```text
1. Generate a random 256-bit attachment key.
2. Encrypt as a chunked authenticated stream using XChaCha20-Poly1305.
3. Compute a hash of the complete ciphertext and relevant metadata.
4. Upload only ciphertext.
5. Send the key, nonce or stream header, hash, size, and object ID inside the MLS event.
```

The storage server MUST NOT receive the attachment key, plaintext filename, MIME type, thumbnail, or plaintext hash unless a later feature explicitly changes the privacy model.

The inner encrypted descriptor SHOULD include:

```text
object_id
ciphertext_hash
ciphertext_size
plaintext_size
filename
media_type
stream_header
attachment_key
thumbnail descriptor, optional
```

Clients MUST verify authenticated decryption and ciphertext hash before exposing the file.

Resumable upload SHOULD use fixed encrypted chunks. Chunk indices and total length MUST be authenticated to prevent reordering or truncation.

# 15. Transport and API encoding

## 15.1 HTTPS

HTTPS is used for:

- Discovery.
- Registration and authentication.
- Server and channel listings.
- History synchronization.
- KeyPackage upload and retrieval.
- Attachment upload and download.
- Administrative operations.

TLS 1.3 SHOULD be preferred. TLS 1.2 MAY be supported only when the deployment platform requires it and secure cipher configuration is maintained.

## 15.2 WebSocket

Secure WebSocket is used for:

- New route events.
- Presence and typing indicators.
- Pending roster operations.
- Call signaling notifications.
- Connection liveness.

The initial subprotocol is:

```text
cords.v1.cbor
```

WebSocket application frames use CBOR. HTTP request and response bodies use JSON unless binary MLS payload size makes CBOR materially preferable.

## 15.3 Deterministic signed encoding

Objects that are signed outside an existing protocol MUST use deterministic CBOR as constrained by the protocol crate. Signatures MUST include an explicit domain-separation label and object version.

Example signing context:

```text
CORDS-SERVER-METADATA-V1 || deterministic_cbor(metadata)
```

JSON text MUST NOT be signed directly.

## 15.4 Version negotiation

Application versions and protocol versions are independent.

```json
{
  "client_version": "0.4.0",
  "protocol_min": 1,
  "protocol_max": 2,
  "features": ["mls-v1", "attachments-v1"]
}
```

A server MUST reject a client when no protocol version overlaps. Feature negotiation MUST be explicit. Unknown optional fields are ignored only where the schema declares forward compatibility.

# 16. Initial HTTP API surface

All paths below are relative to `/api/v1`.

| Method | Path | Purpose |
|---|---|---|
| GET | `/capabilities` | Protocol and feature negotiation |
| POST | `/auth/challenge` | Create a single-use device challenge |
| POST | `/auth/session` | Authenticate device and create short-lived session |
| POST | `/join/request` | Begin public, invite, password, or approval join |
| GET | `/join/status/{request_id}` | Read pending join status |
| GET | `/servers/self` | Read signed server metadata |
| GET | `/members` | Read authorized server roster visible to caller |
| GET | `/members/{member_id}` | Read member and device credential bundle |
| GET | `/channels` | List visible channels |
| POST | `/channels` | Create a channel |
| GET | `/channels/{channel_id}` | Read channel metadata and roster state |
| POST | `/channels/{channel_id}/roster-requests` | Queue MLS add or remove request |
| POST | `/channels/{channel_id}/commits` | Upload validated MLS commit |
| GET | `/routes/{route_id}/events` | Synchronize ciphertext event history |
| POST | `/routes/{route_id}/events` | Upload ciphertext event |
| GET | `/key-packages/{account_id}` | Retrieve available device KeyPackages |
| POST | `/key-packages` | Publish KeyPackages |
| POST | `/attachments` | Initiate encrypted upload |
| PUT | `/attachments/{object_id}/{chunk}` | Upload encrypted chunk |
| GET | `/attachments/{object_id}` | Download encrypted blob or manifest |
| POST | `/invites` | Create invite with limits |
| DELETE | `/invites/{invite_id}` | Revoke invite |
| POST | `/moderation/bans` | Ban membership or identity |
| DELETE | `/moderation/bans/{ban_id}` | Remove ban |
| GET | `/events` | Upgrade to secure WebSocket |

The exact schema MUST be maintained in the `cords-protocol` crate and exported as machine-readable API documentation.

# 17. Presence, typing, and ephemeral signals

Presence and typing indicators are metadata-heavy and are not essential to message correctness.

Defaults:

- Presence is opt-in per server.
- Typing indicators are opt-in per conversation.
- The server may relay ephemeral signals without durable storage.
- Ephemeral signals MUST expire quickly.
- Users may disable transmitting last-seen timestamps.

For encrypted channels, typing payloads SHOULD be MLS-encrypted when practical. The server still observes the sender connection and route.

# 18. Voice and video

## 18.1 Initial call scope

The first RTC milestone is one-to-one audio. Video and screen sharing follow after audio reliability.

The client WebView uses the platform WebRTC implementation for media capture, ICE, DTLS-SRTP, codecs, and peer connection state. Rust handles call authorization, signaling integration, permissions, and persistent state.

## 18.2 Signaling

Offers, answers, ICE candidates, and call-control messages MUST be carried inside MLS-encrypted DM events. The server only receives normal ciphertext envelopes and minimal wake-up metadata.

## 18.3 STUN and TURN

Servers MAY publish STUN and TURN configuration. TURN credentials MUST be short-lived and scoped. Coturn is the initial recommended TURN service.

## 18.4 Group calls

An SFU is deferred. When introduced, media requires application-level end-to-end encryption if the SFU must remain unable to access media. Ordinary transport encryption between each client and SFU is not sufficient for that claim.

Cords MUST NOT build a custom SFU in the initial project.

# 19. Server permissions and moderation

The server can:

- Approve or deny membership.
- Assign roles and capabilities.
- Create, archive, and delete channels.
- Restrict who may upload events to a route.
- Stop delivering events to a membership.
- Kick or ban members.
- Rate limit clients.
- Remove encrypted blobs according to policy.
- Record moderation actions.

The server cannot:

- Read encrypted message contents.
- Forge a valid MLS application message from a user device.
- Reconstruct attachment plaintext.
- Silently add a decrypting member without clients accepting a valid MLS state change.
- Recover a user's account root.

Message reports in encrypted spaces require user participation. A reporting client MAY submit the selected plaintext, original encrypted event, and verifiable context to moderators. The UI MUST state exactly what is disclosed.

# 20. Client storage

## 20.1 SQLite

The desktop client uses SQLite for:

- Server bookmarks and pinned identities.
- Membership credentials.
- Conversation metadata.
- Ciphertext event cache.
- Optional encrypted plaintext cache.
- Delivery cursors and retries.
- Device and account state.
- MLS persistent state.

## 20.2 Secret encryption at rest

Account private keys, device private keys, MLS state, attachment keys, and recovery material MUST be encrypted at rest.

The client generates a random local storage master key. Preferred storage:

1. Operating-system credential store.
2. Hardware-backed credential provider where available.
3. User passphrase wrapping with Argon2id when no credential store is available.

A fallback that stores the master key beside the encrypted database is not acceptable.

## 20.3 Plaintext cache

Users may choose:

- `encrypted`: plaintext is cached under the local storage master key.
- `session`: plaintext exists only while the client runs.
- `minimal`: only explicitly saved drafts and attachment metadata are retained.

Local search is available only over data the client has decrypted and retained.

# 21. Server storage

## 21.1 PostgreSQL

PostgreSQL stores:

- Server configuration references.
- Server memberships and roles.
- Channel definitions and permission mappings.
- Invite and pre-registration records.
- Ciphertext event envelopes.
- Route sequences and synchronization cursors.
- KeyPackage inventory.
- Attachment object metadata.
- Moderation and audit records.
- Pending MLS roster operations and commit leases.

## 21.2 Attachment blobs

The initial Docker deployment stores encrypted blobs under a persistent data volume. Storage access MUST be hidden behind a trait so S3-compatible object storage can be added without changing the protocol.

## 21.3 Retention

Retention is a server policy applied to ciphertext and metadata. The server MAY delete old ciphertext, but cannot force recipients to delete copies already downloaded.

Retention settings MUST be visible to members before joining or posting.

# 22. Monorepo and Cargo workspace

Recommended repository layout:

```text
cords/
+-- Cargo.toml
+-- Cargo.lock
+-- rust-toolchain.toml
+-- deny.toml
+-- README.md
+-- CHANGELOG.md
+-- AGENTS.md
+-- docs/
|   +-- CORDS_SPEC.md
|   +-- adr/
|   +-- protocol/
+-- crates/
|   +-- cords-protocol/
|   +-- cords-identity/
|   +-- cords-crypto/
|   +-- cords-storage/
|   +-- cords-client-core/
|   +-- cords-server-core/
|   +-- cords-test-vectors/
+-- bins/
|   +-- cords-client/
|   |   +-- src-tauri/
|   |   +-- ui/
|   +-- cords-server/
|   +-- cords-admin/
|   +-- xtask/
+-- migrations/
+-- deploy/
|   +-- Dockerfile
|   +-- compose.yaml
|   +-- example.env
|   +-- healthcheck.sh
+-- test-vectors/
+-- scripts/
+-- .forgejo/
    +-- workflows/
```

## 22.1 Crate responsibilities

### `cords-protocol`

Owns public identifiers, HTTP and WebSocket schemas, deterministic encodings, protocol versions, error codes, feature names, and TypeScript type generation.

It MUST NOT depend on Axum database models, UI state, or OpenMLS internal types.

### `cords-identity`

Owns account roots, device authorization, fingerprints, server identities, membership credential verification, OpenPGP attestations, and revocation state.

### `cords-crypto`

Owns the MLS adapter, attachment encryption, secret wrapping interfaces, and cryptographic test vectors. It MUST forbid custom unaudited cryptographic primitives.

### `cords-storage`

Owns storage traits and database-independent repositories. Concrete PostgreSQL and SQLite implementations MAY live here or in client and server adapters, but wire types MUST remain separate.

### `cords-client-core`

Owns server sessions, synchronization, retries, MLS conversation orchestration, local cache policy, and UI-facing application services.

### `cords-server-core`

Owns membership, permissions, channel policy, relay acceptance, sequence allocation, moderation, and roster-operation coordination.

### `cords-test-vectors`

Owns stable identity fingerprints, deterministic CBOR encodings, signature fixtures, encrypted attachment fixtures, and MLS integration scenarios.

## 22.2 Workspace lints

The workspace SHOULD enforce:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_debug_implementations = "warn"
unreachable_pub = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
unwrap_used = "warn"
expect_used = "warn"
```

Exceptions require a local annotation and explanation. Cryptographic or FFI code that needs `unsafe` MUST be isolated in a reviewed crate boundary.

# 23. Desktop client architecture

## 23.1 Tauri boundary

The TypeScript UI is not trusted with long-lived private keys or raw MLS state.

The Tauri API exposes narrow commands such as:

```text
list_servers
join_server
list_channels
open_conversation
send_message
edit_message
add_reaction
start_call
verify_device
export_recovery_bundle
```

The UI receives view models, not internal cryptographic objects.

## 23.2 UI layout

Initial desktop layout:

```text
+------+----------------+-----------------------------------+
|      |                |                                   |
| srv  | channels and   | message timeline                  |
| bar  | direct messages|                                   |
|      |                | composer                          |
+------+----------------+-----------------------------------+
```

The server bar is vertically scrollable. Selecting a server changes the channel list and server context. Direct messages MAY appear in a dedicated global area rather than being duplicated under every server.

## 23.3 Security UX

The client MUST display:

- Server fingerprint changes.
- Account key changes.
- New and revoked devices.
- Pending MLS roster changes.
- Whether a conversation is verified.
- Whether a new device lacks old history.
- Server retention policy.
- Whether a call is peer-to-peer or uses TURN.

Security warnings MUST be actionable and must not train users to click through routine false alarms.

# 24. Server architecture

## 24.1 Process model

`cords-server` is a normal Rust executable that:

1. Loads configuration.
2. Initializes structured logging.
3. Connects to PostgreSQL.
4. Validates or runs migrations according to configuration.
5. Loads or creates the persistent server signing key.
6. Initializes blob storage.
7. Starts HTTP and WebSocket listeners.
8. Handles graceful shutdown.

The Docker image is a distribution method, not an architectural dependency.

## 24.2 Axum layers

Recommended middleware order:

```text
request ID
trusted proxy handling, if configured
body and header size limits
timeout
rate limiting
TLS termination or forwarded TLS verification
session authentication
authorization
route handler
structured response logging
```

Sensitive headers, signatures, tokens, ciphertext bodies, and key material MUST be redacted from logs.

## 24.3 Background work

Background tasks MAY handle:

- Expired challenge cleanup.
- Invite expiration.
- KeyPackage low-inventory notifications.
- Blob garbage collection.
- Retention deletion.
- Pending roster operation reminders.

All background work MUST be restart-safe and idempotent. Important state belongs in PostgreSQL, not only in memory.

# 25. Docker-first deployment

## 25.1 Supported topology

The official Compose deployment contains:

- `cords-server`
- PostgreSQL
- Persistent server data volume
- Optional coturn profile
- Optional reverse proxy example

The server MAY terminate TLS directly. Production documentation SHOULD recommend a reverse proxy such as Caddy or nginx without making it mandatory.

## 25.2 Container requirements

The official image MUST:

- Use a multi-stage build.
- Run as a non-root numeric user.
- Contain the server binary and required CA certificates.
- Include no compiler or package manager in the final image.
- Expose port 4848 by default.
- Define a health check.
- Store mutable state only in declared volumes.
- Support `linux/amd64` and `linux/arm64`.
- Publish immutable digest-addressable images.
- Include OCI source, revision, version, and license labels.

A Debian slim runtime is preferred initially. A musl image MAY be investigated later, but static linking MUST NOT delay the first reliable deployment.

## 25.3 First startup

If no server identity exists:

1. Generate the key using a cryptographically secure random source.
2. Write it atomically with restrictive permissions.
3. Derive and log the public fingerprint.
4. Continue startup only after confirming persistent storage is writable.

If the operator starts without the expected persistent volume, the new server identity will differ. The log MUST make that event conspicuous.

## 25.4 Configuration precedence

```text
command-line arguments
then environment variables
then configuration file
then built-in defaults
```

Nested environment variables use double underscores, for example:

```text
CORDS_SERVER__LISTEN=0.0.0.0:4848
CORDS_SERVER__PUBLIC_ORIGIN=https://socool.com:4848
CORDS_DATABASE__URL=postgres://...
CORDS_REGISTRATION__MODE=invite
```

Private signing keys SHOULD be mounted as files or Docker secrets. They SHOULD NOT be placed directly in environment variables.

# 26. Administration CLI

`cords-admin` communicates with the authenticated server administration API. It MUST NOT mutate PostgreSQL directly.

Initial commands:

```text
cords-admin server status
cords-admin server fingerprint
cords-admin invite create
cords-admin invite revoke
cords-admin preregister add
cords-admin member list
cords-admin member ban
cords-admin member unban
cords-admin role create
cords-admin role grant
cords-admin channel create
cords-admin key rotate
```

Machine-readable JSON output SHOULD be available for every command.

# 27. Database migrations

Migrations are versioned in the repository and embedded or packaged with the server binary.

Supported modes:

```text
cords-server migrate
cords-server serve --migrate
cords-server serve --require-current-schema
```

The Docker Compose quick start MAY use automatic migrations. Production operators may choose an explicit migration step.

A schema change MUST include:

- Forward migration.
- Rollback guidance, even when automatic rollback is unsafe.
- Migration test from the previous supported release.
- Updated backup and restore notes.

# 28. Error model

Public errors contain:

```text
stable error code
human-readable message safe for display
request ID
retry classification
optional structured details
```

Examples:

```text
CORDS_AUTH_CHALLENGE_EXPIRED
CORDS_PROTOCOL_NO_COMMON_VERSION
CORDS_SERVER_KEY_CHANGED
CORDS_MLS_EPOCH_GAP
CORDS_MEMBERSHIP_REVOKED
CORDS_PERMISSION_DENIED
CORDS_ATTACHMENT_HASH_MISMATCH
CORDS_RATE_LIMITED
```

Internal stack traces and database errors MUST NOT be returned to clients.

# 29. Observability and privacy

The server uses structured tracing with request IDs and route-safe identifiers.

Logs MUST NOT include:

- Private keys.
- Recovery bundles.
- Bearer tokens.
- Invite tokens.
- Password material.
- Decrypted messages.
- Attachment keys.
- Raw MLS state.
- Full ciphertext bodies by default.

Metrics MAY include aggregate connection count, request latency, queue depth, database health, event volume, and storage usage. Metrics labels MUST avoid account IDs, channel IDs, DM IDs, and other unbounded user identifiers.

Telemetry from the desktop client is opt-in. Crash reports MUST strip secrets and message content.

# 30. Abuse resistance

The server MUST implement configurable:

- Per-IP and per-account connection limits.
- Challenge creation limits.
- Join attempt limits.
- Event rate and burst limits.
- Attachment size and quota limits.
- KeyPackage publication limits.
- Invite creation limits.
- WebSocket idle timeouts.
- Maximum concurrent calls or TURN allocations where applicable.

Parsers MUST reject oversized and deeply nested data before expensive processing. Compression bombs and decompression of attacker-controlled payloads require explicit limits.

# 31. Testing requirements

## 31.1 Unit tests

Every crate MUST test public invariants and error paths.

Security-sensitive tests include:

- Fingerprint stability.
- Deterministic CBOR byte equality.
- Signature domain separation.
- Device authorization and revocation.
- Server key rotation.
- Challenge replay rejection.
- KeyPackage single-use behavior.
- Attachment truncation and reordering rejection.
- Permission checks.

## 31.2 Integration tests

The workspace MUST include multi-client scenarios:

1. Create two accounts and multiple devices.
2. Join one server.
3. Create an encrypted channel.
4. Add and remove devices.
5. Exchange messages across epochs.
6. Restart the server and clients from persisted state.
7. Verify duplicate event handling.
8. Verify a revoked device cannot receive future events.
9. Create a DM through a shared server.
10. Upload and decrypt an attachment.

## 31.3 Interoperability and test vectors

Cords MUST store stable vectors for:

- Signed metadata encoding.
- Account and server fingerprints.
- Membership credentials.
- Attachment encryption.
- Protocol envelope serialization.

MLS behavior SHOULD be tested against upstream implementation vectors and, when practical, a second independent MLS implementation.

## 31.4 Fuzzing

Fuzz targets SHOULD cover:

- CBOR and JSON decoders.
- Public identifier parsing.
- Invite URL parsing.
- Membership credential parsing.
- Attachment manifests.
- WebSocket frame payload dispatch.
- State-machine transitions around missing and duplicate events.

## 31.5 Continuous integration

Required checks:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
frontend format, typecheck, lint, and tests
container build
Docker Compose startup smoke test
migration test
license and secret scan
```

Release workflows MUST build from a clean tag and record the exact source revision in binaries and container labels.

# 32. Security development rules

Contributors MUST follow these rules:

1. Do not implement cryptographic primitives from scratch.
2. Do not use unauthenticated encryption.
3. Do not sign ambiguous or non-deterministic encodings.
4. Do not reuse keys across protocols.
5. Do not log secrets, plaintext, or recovery material.
6. Do not accept silent server or account key replacement.
7. Do not add a server-side plaintext shortcut for development.
8. Do not use `unwrap` or `expect` on attacker-controlled input paths.
9. Zeroize secret buffers where the selected library supports it.
10. Keep dependencies minimal and review cryptographic dependency updates.
11. Isolate FFI and unsafe code.
12. Treat the WebView as an untrusted presentation layer relative to private keys.
13. Require authorization checks in server-core services, not only in HTTP handlers.
14. Add regression tests for every security bug.

# 33. Release and compatibility policy

The client, server, and admin CLI are released from the same repository tag, but may have different package versions later.

Release artifacts SHOULD include:

```text
cords-client-<version>-linux
cords-client-<version>-windows
cords-client-<version>-macos
cords-server-<version>-linux-amd64
cords-server-<version>-linux-arm64
cords-admin-<version>-<platform>
container:<version>
container:<major>.<minor>
container:latest
```

Standalone server binaries are published as convenience artifacts. The supported hosting procedure remains Docker until native deployment documentation and tests are added.

A protocol compatibility window MUST be documented. A client MUST not assume that server and client were upgraded together.

# 34. Implementation phases

## Phase 0: Repository foundation

Deliverables:

- Cargo workspace and crate boundaries.
- Tauri shell and basic three-pane UI.
- Axum server with health endpoint.
- PostgreSQL and SQLite adapters.
- Dockerfile and Compose startup.
- Shared protocol version negotiation.
- CI gates.
- ADR process and test-vector directory.

Acceptance criteria:

- `docker compose up` starts a healthy server.
- Desktop client connects and displays signed server metadata.
- No message functionality is claimed yet.

## Phase 1: Native identity and server membership

Deliverables:

- Account and device key generation.
- Encrypted local secret storage.
- Fingerprints and verification UI.
- Server signing identity and pinning.
- Challenge-response authentication.
- Public, invite, and pre-registration admission.
- Membership credentials and roles.
- Optional OpenPGP attestation import and verification.

Acceptance criteria:

- A device can join, reconnect, and be revoked.
- Server key changes produce a blocking warning.
- Challenge replay fails.
- Secrets remain encrypted at rest.

## Phase 2: Encrypted channels

Deliverables:

- MLS adapter.
- KeyPackage publication.
- Channel creation and group binding.
- Add and remove roster operations.
- Commit leases and event synchronization.
- Encrypted message creation, edit, reaction, and deletion events.

Acceptance criteria:

- The server database contains no plaintext message body.
- Two accounts with multiple devices can exchange messages.
- Removing a device excludes it from future epochs.
- Restarted clients recover MLS state correctly.

## Phase 3: Direct messages

Deliverables:

- Shared-server contact bundle lookup.
- Two-account MLS group creation.
- DM primary relay binding.
- Global DM list in the client.
- Blocking and termination behavior.

Acceptance criteria:

- A DM can be created from a shared server.
- The relay cannot decrypt content.
- Device changes are visible and cryptographically applied.

## Phase 4: Encrypted attachments

Deliverables:

- Chunked XChaCha20-Poly1305 encryption.
- Resumable upload and download.
- Local and server quotas.
- Integrity validation and safe file handling.

Acceptance criteria:

- Storage compromise reveals no filename, MIME type, or attachment key.
- Truncated, reordered, or modified ciphertext is rejected.

## Phase 5: Moderation and operational hardening

Deliverables:

- Admin CLI.
- Invites, approvals, roles, bans, and audit records.
- Retention jobs and blob garbage collection.
- Backup and restore documentation.
- Rate limiting and abuse controls.
- Upgrade and migration tests.

## Phase 6: One-to-one RTC

Deliverables:

- MLS-encrypted WebRTC signaling.
- Audio calls.
- TURN integration.
- Call security indicators.
- Video and screen sharing after audio stability.

## Phase 7: Deferred privacy and scale features

Candidates:

- Per-server pseudonymous personas.
- DM relay migration and cross-server delivery.
- Key transparency service.
- Encrypted history backup and device transfer.
- Mobile clients.
- SFU-based group calls with application-level media encryption.
- S3-compatible attachment storage.
- Hardware-backed native account roots.

# 35. Definition of done for any feature

A feature is not complete until:

- The architecture boundary remains intact.
- Protocol changes are versioned and documented.
- Database changes have migrations.
- Authorization is enforced in core services.
- Unit and integration tests cover success and failure paths.
- Errors are stable and safe to display.
- Logs contain no sensitive content.
- The Docker deployment still passes its smoke test.
- Client UI communicates relevant security state.
- Documentation and examples are updated.
- `cargo fmt`, Clippy, tests, dependency checks, and frontend checks pass.

# 36. Codex execution contract

Codex MUST follow this workflow when implementing Cords:

1. Read this specification and all relevant ADRs before modifying code.
2. Identify the current implementation phase and avoid silently building later-phase features.
3. Write a short implementation plan in the task or pull request description.
4. Modify the smallest appropriate crate boundary.
5. Reuse protocol and identity types rather than cloning similar structures.
6. Add tests before or with behavior changes.
7. Run the full required local check set when practical.
8. Report any unrun checks honestly.
9. Never weaken a security requirement merely to make a test pass.
10. Never replace a maintained protocol with a hand-built approximation.
11. Create an ADR when changing a major dependency, trust boundary, key hierarchy, wire encoding, storage model, or deployment model.
12. Do not commit generated secrets, local databases, test accounts, or real invite tokens.
13. Keep public APIs documented and keep internal types private by default.
14. Prefer explicit state machines and typed transitions over collections of booleans.
15. Preserve backward compatibility inside the declared protocol window.

## 36.1 Expected Codex response to ambiguity

When a requirement is incomplete, Codex SHOULD:

1. Select the safest interpretation consistent with this document.
2. Record the interpretation in code comments, tests, or an ADR as appropriate.
3. Avoid inventing cryptographic or identity semantics.
4. Leave a clearly scoped issue when a human decision is genuinely required.

## 36.2 Prohibited shortcuts

Codex MUST NOT:

- Put plaintext messages in PostgreSQL even temporarily.
- Put private keys in TypeScript state.
- Disable TLS verification.
- Auto-accept changed server keys.
- Use server passwords as long-term user authentication.
- Treat a username or email address as a cryptographic identity.
- Add hidden server membership to MLS groups.
- Directly query PostgreSQL from the admin CLI.
- Serialize internal database structs as the public protocol.
- Couple the entire application to OpenMLS concrete types.
- Mark native deployment as supported without tests and documentation.

# 37. Open decisions

The following decisions remain intentionally open and require an ADR before production release:

1. Confirm `GPL-2.0-only` versus another GPL identifier.
2. Select OpenMLS or another RFC 9420 implementation after a focused prototype and persistence review.
3. Select the deterministic CBOR Rust library and lock its encoding profile with vectors.
4. Select the local credential-store abstraction across Linux, Windows, and macOS.
5. Define default MLS self-update time and message-count thresholds.
6. Define the exact compatibility window between client and server protocol versions.
7. Define backup format and recovery UX.
8. Select an audited Rust OPAQUE implementation before enabling password admission.
9. Decide whether the first public release includes video or audio only.
10. Define the privacy design for per-server personas without cross-server linkability.

# 38. Authoritative references

The implementation SHOULD consult the current published standards and official project documentation rather than secondary tutorials.

1. RFC 9420, *The Messaging Layer Security Protocol*.
2. RFC 9750, *The Messaging Layer Security Architecture*.
3. RFC 9580, *OpenPGP*.
4. RFC 9807, *The OPAQUE Augmented Password-Authenticated Key Exchange Protocol*.
5. RFC 8949, *Concise Binary Object Representation*.
6. RFC 6455, *The WebSocket Protocol*.
7. RFC 8446, *The Transport Layer Security Protocol Version 1.3*.
8. W3C, *WebRTC: Real-Time Communication in Browsers*.
9. OpenMLS official repository and book.
10. Tauri v2 official documentation.
11. Axum official crate documentation.

# 39. Final architecture statement

Cords is one Rust monorepo containing a client, server, administration CLI, shared protocol crates, cryptographic adapters, test vectors, and Docker deployment files.

The client connects to independent HTTPS server origins shown in a Discord-like sidebar. Servers control communities and relay ciphertext. Accounts and devices control identity. MLS groups control who can decrypt channels and direct messages. OpenPGP may attest a Cords account but does not encrypt chat traffic. Docker is the only initially supported server-hosting method, while standalone binaries remain available for unsupported custom deployments.

The product promise is narrow and testable:

> Cords lets people communicate across independently hosted communities while keeping message and attachment plaintext on authorized client devices.

**Cords. Right on the wire.**
