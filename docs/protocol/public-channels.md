# Public channels, immutable identity and succession (v1)

Wire types are maintained in `cords-protocol::messaging`. Discovery advertises
`public-channels-v1` and `channel-succession-v1`; protocol version remains 1.
New clients require signed identity/lifecycle metadata before using channel state.

## Identity and authorization

`POST /api/v1/channels` accepts `name`, `confidentiality_mode` (`encrypted`, the
default, or `public`) and `idempotency_key`. Existing owner/create checks apply.
The response remains the server-adopted channel ID. Names are presentation labels;
duplicate names never merge routes.

Channel views include the permanent mode, signed `identity` and signed `transition`.
Identity binds version, server ID, channel ID, name, creator device, mode and
creation time. Transition binds version, server ID, channel ID, retired flag,
monotonic generation and optional signed succession. Generation is 0 for a new
ordinary active route, 1 for an active successor and 2 for retirement. Established
retirement/succession is terminal. Local acknowledgement/archive UI fields are
computed by client-core and are never trusted from HTTP responses.

The signing domains are `CORDS-CHANNEL-IDENTITY-V1`,
`CORDS-CHANNEL-TRANSITION-V1`, `CORDS-CHANNEL-SUCCESSION-V1` and
`CORDS-PUBLIC-MESSAGE-V1`, followed by the existing length-first deterministic CBOR.
Each uses distinct domain separation and strict schemas; JSON is only transport.

## Public relay events

The existing ordered route endpoint accepts `content_encoding: public.signed`,
an empty legacy `ciphertext` field, and `public_message: Signed<PublicMessage>`.
The signed object contains version, server ID, channel ID, event ID, idempotency
key, root-certified contact, server-signed membership and the plaintext `Message`.
Message content, sender account/device, client timestamp and schema/event kind
are covered by the device signature. Outer routing/author/timestamp/key fields
must agree with that object. The contact must match the currently authorized
session device; ordinary revocation, burn, membership and lockdown gates apply
under the existing transaction locks. Root/device authorization is independently
verified by recipients. Replaying across channels/servers or changing content
invalidates the signature/bindings; event/message IDs prevent accepted duplicates.

Encrypted routes exclusively accept `mls.application` and existing MLS commits.
Public routes reject bindings, Welcome requests, commits and roster operations;
membership churn needs no cryptographic rekey. Both modes require their existing
authenticated read/write capabilities. Public history is readable by admitted
members with `channel.read`; unauthenticated access is not implemented.

## Replacement and retirement

`POST /api/v1/channels/{original}/replace` accepts a `ChannelReplace` containing
a device-signed succession record and idempotency key. The signed statement binds
version, server ID, full predecessor identity, fresh successor UUID, display name,
mode, initiator contact, membership, issued-at time and nonce. Current owner/create
and route-management authority are checked. New requests expire after 300 seconds;
accepted exact retries recover the original response. Pending roster work blocks
replacement. The successor and both lifecycle records change atomically with
predecessor retirement. There is no history or key migration.

`POST /api/v1/channels/{original}/retire` accepts an idempotency-key JSON string
under existing route-management authority. It preserves the original immutable
identity and makes the route permanently read-only. Names may be reused with new
IDs. Physical ID deletion and mode edits are prohibited by PostgreSQL.

Clients verify every signed transition, reject identity conflicts and lifecycle
rollback, and preserve trusted records after restart/removal. Refreshing channel
metadata after reconnect discovers succession without relying on a transient
WebSocket notification. Users see both modes, both names and distinct IDs; choosing
to open a public successor does not bypass the durable send acknowledgement gate.

## Local archive semantics

Authenticated retirement snapshots eligible received history into the existing
sealed vault. Archives include the original identity/mode and succession record,
are partial and read-only, and remain available after server removal. No complete
history, physical secure erasure, independent cryptographic transcript backup,
server/DM history backup or automatic attachment recovery is claimed. See
[ADR 0006](../adr/0006-public-channels-and-channel-succession.md).
