# ADR 0003: Authenticated one-time server ownership bootstrap

- Status: Accepted
- Date: 2026-10-04

## Context

A public join proves control of an authorized Cords device but does not prove authority over the
server deployment. Channel creation likewise must not silently imply server ownership. Cords needs
a first-owner bootstrap that preserves account/device/server separation, does not expose account
private keys, and cannot be replayed to replace an established owner.

The client also needs a URL-only join experience. Requiring a user to copy a server fingerprint
into the same form as the URL adds friction without providing an independent verification channel.
Normal Web PKI and the persistent Cords signing identity remain separate requirements.

## Decision

The desktop identity client generates a random 256-bit, unpadded base64url claim code. The operator
places that code temporarily in the server configuration as
`CORDS_AUTHENTICATION__OWNER_CLAIM_CODE` and restarts the service. The server validates a minimum
32-character non-whitespace value, retains only its SHA-256 digest in service memory, and never
logs it.

An already authenticated device redeems the code at `/api/v1/ownership/claim`. The server compares
the digest in constant time and, under one database transaction and ownership lock, records the
authenticated account as the singleton owner and reissues server-signed membership credentials for
that account's active devices. Owner credentials include `server.manage` and `channel.manage`.
Future root-authorized devices for the owner account inherit those capabilities. A retry by the
same owner is idempotent; another account receives a conflict. Ownership transfer and deletion are
not implied and remain unavailable until separately specified.

For an unpinned origin, the client fetches discovery over certificate-validated HTTPS, verifies the
server's signed metadata, and pins the discovered signing identity. Every later authentication
repeats discovery and refuses an origin or server-identity mismatch. The fingerprint remains an
inspectable identifier but is no longer user input. Invite-carried independent fingerprints remain
a compatible future strengthening mechanism.

## Consequences

Possession of the configured code alone is insufficient: redemption also requires a valid Cords
device session, and the resulting authority is bound to that account. Conversely, a compromised
server configuration channel can expose an unredeemed code; operators must use a secret-capable
configuration path, remove the value after redemption, and avoid shell history where applicable.
The durable singleton owner record makes the configured value inert after the first successful
claim.

URL-only first use trusts the server identity authenticated by the requested HTTPS origin. It does
not claim protection from a compromised CA or origin on first contact. Pinning detects later
application-key substitution, and deployments needing independent first-contact verification can
compare the displayed pinned fingerprint or eventually use a fingerprint-bearing invite.
