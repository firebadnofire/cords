# ADR 0003: Pre-membership one-time server ownership bootstrap

- Status: Accepted (supersedes the October 4 client-generated-code decision)
- Date: 2026-10-10

## Context

A public join proves control of an authorized Cords device but does not prove authority over the
server deployment. A fresh server must therefore have an explicit state before it admits ordinary
members, and the first owner must prove both deployment access and control of a portable Cords
account root. Client-generated configuration values allowed membership to exist before ownership
and made server restart/configuration part of the claim protocol; that design is superseded here.

## Decision

After migrations and persistent server-identity binding, a fresh empty server atomically creates a
singleton `UNCLAIMED` record and a cryptographically random 256-bit, unpadded base64url code. The
plaintext is emitted once to the operator log. PostgreSQL stores only a domain-separated SHA-256
verifier bound to the server ID. Restart reuses that verifier without re-emitting or rotating the
code. No HTTP endpoint returns the code.

An operator may rotate an unused code only through the local
`cords-server ownership-bootstrap rotate` command. A populated database with neither an ownership
record nor bootstrap state refuses to serve until the operator explicitly runs
`cords-server ownership-bootstrap initialize`; this creates an unclaimed verifier and never selects
an existing account. An existing durable owner is migrated to `CLAIMED` without creating a code.

The signed `GET /api/v1/ownership` response exposes only `UNCLAIMED` or `CLAIMED` plus a verifier
generation. While unclaimed, ordinary join challenges and session creation are refused. A claimant
requests a short-lived ownership challenge and submits:

- the one-time code;
- the root-authorized device contact;
- a device signature over the server challenge; and
- an account-root signature binding that exact challenge, server, account and device.

The server validates all proofs and compares the verifier in constant time. One PostgreSQL
transaction and advisory lock then persist the contact, create the first owner membership, bind the
singleton owner, clear the verifier, mark the bootstrap `CLAIMED`, consume the challenge and issue a
session. A failed transaction leaves the server unclaimed and the code usable. Failed claims create
no contacts, account heads, memberships, owner records or sessions.

Ordinary members never receive `server.manage` or `channel.manage`. Later authorized devices for
the owner account receive owner capabilities from the durable owner binding. Device revocation,
account burning or membership inactivity does not delete ownership or recreate bootstrap state;
ownership recovery or transfer requires a separately specified protocol.

## Security and operational consequences

The code is bearer bootstrap material and appears in operator-controlled logs by design. Operators
must restrict log access and retention and rotate the code if it may have leaked before use. The
client keeps the submitted code only for the request, redacts it from Rust debug output, and clears
the UI field. Rate limits apply to challenge and claim attempts.

Claims still use normal certificate-validated HTTPS. The client verifies signed discovery, pins the
persistent server identity, and verifies the signed ownership state before sending the code. First
contact therefore inherits normal Web PKI trust; an independent invite fingerprint remains a future
strengthening mechanism.
