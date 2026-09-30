# Cords Key System Architecture

## Status and purpose

This document defines the long-lived cryptographic identity and key architecture for Cords. It is intended to remain valid across implementation phases and to provide the normative model from which protocol encodings, storage formats, APIs, recovery flows, and implementation-specific ADRs are derived.

This document describes **roles, authority, trust relationships, lifecycle, and separation requirements**. It does not by itself define wire encodings, database schemas, user-interface details, or a concrete MLS library API. Those belong in versioned protocol documents, crate interfaces, and ADRs.

Where this document conflicts with transient implementation details, the conflict must be resolved explicitly. Code must not silently redefine a key role or trust boundary.

## Security goals

The Cords key system is designed around the following properties:

1. A Cords account is portable and is not cryptographically owned by any one server.
2. Servers authenticate and authorize accounts and devices without receiving the account's root private key.
3. Compromise of one device does not require sharing that device's private key with other devices.
4. Routine messaging does not require use of the account root private key.
5. Message and attachment encryption keys are distinct from account, device, server, and OpenPGP identity keys.
6. Server identity is distinct from Web PKI/TLS identity, while both are required at their respective trust layers.
7. OpenPGP may attest to a Cords identity but does not become the native Cords identity system.
8. Device addition, revocation, recovery, and server-key changes are explicit authenticated transitions rather than implicit consequences of account login.
9. Possession of server credentials alone is insufficient to recover end-to-end encrypted plaintext.
10. Cryptographic roles are domain-separated and private key material is not reused across roles.

## Identity hierarchy

Cords distinguishes four major identity scopes:

```text
Account
  |
  +-- Account Root Key
  |      |
  |      +-- authorizes Device A identity
  |      +-- authorizes Device B identity
  |      +-- authorizes recovery / root transitions
  |
  +-- Device A Identity
  |      |
  |      +-- authenticates device operations
  |      +-- binds MLS credentials / KeyPackages
  |      +-- participates in conversations as device-specific MLS leaves
  |
  +-- Device B Identity
         |
         +-- authenticates device operations
         +-- binds MLS credentials / KeyPackages
         +-- participates in conversations as device-specific MLS leaves

Server
  |
  +-- Server Signing Key

Conversation
  |
  +-- MLS group state and per-device leaves
```

These identities are related by signed authorization statements, not by reuse of the same key pair.

## Account root

### Role

Each Cords account has a native **account root signing key**. The initial required account-root algorithm is Ed25519 unless superseded by a versioned architectural decision.

The account root is the highest client-controlled authority for the account. It exists to certify changes to the account's authorized cryptographic state, particularly device enrollment, device revocation, and recovery or root-transition operations.

The account root is not a chat-encryption key, an MLS leaf key, a TLS key, a server credential, or an OpenPGP key.

### Authority

The account root may authorize statements including:

- creation and authorization of a device identity;
- revocation of a previously authorized device;
- monotonically ordered account/device authorization generations;
- recovery-related state transitions;
- replacement or rotation of account-root authority where the recovery design permits it;
- optional binding of external attestations to the native account identity.

The exact signed encodings are protocol-versioned and must be domain-separated.

### Custody

The account-root private key is client-held. A Cords server MUST NOT possess or escrow the unwrapped account-root private key.

The root should not be required for routine message sending, receiving, synchronization, or ordinary authenticated server requests. Implementations should therefore permit the root to be protected more strongly than routine device credentials, including encrypted local storage, passphrase wrapping, operating-system credential storage, or hardware-backed storage where available.

A server-side account login or session is not proof of possession of the account root.

## Device identity

### Role

Every Cords installation participating as an authorized account device has its own **device identity** and independent private key material.

A device identity represents one cryptographic installation, not the abstract user account. Multiple devices belonging to the same account remain distinct identities.

### Authorization

A device becomes part of an account through an account-root-signed device authorization statement. That statement binds, at minimum, the account identity, device identity/public key material, authorization generation or equivalent ordering state, and any protocol information required to prevent substitution or cross-protocol use.

Servers and peers must be able to verify the authorization chain without possessing the account-root private key.

Device authorization does not by itself grant membership on every Cords server. Server membership and account/device authorization are separate requirements.

### Revocation

Device revocation is an account-authorized state transition. Revocation must be represented in a way that allows servers and clients to reject the revoked device for future authenticated actions.

Revocation of a device does not imply that previously delivered plaintext, keys, or ciphertext can be erased from that device. Conversation protocols must instead prevent the revoked device from receiving future group secrets after the relevant MLS membership transition completes.

## Device authentication and encryption material

A device may require multiple pieces of key material for different protocols. Those keys MUST remain logically distinct even when generated and stored by the same installation.

A device identity signing key may authenticate device-level Cords statements and server requests. Any device encryption or key-agreement material required outside MLS must have a separately defined role and domain.

No device identity private key may be substituted directly for an MLS signature key merely because both algorithms happen to use compatible curve families or byte lengths.

## MLS identity and conversation keys

### Conversation scope

Each encrypted Cords channel is an MLS group. Each encrypted direct-message relationship is likewise an MLS group whose membership consists of authorized devices belonging to the participating accounts.

MLS membership is device-based. An account with three active devices can therefore occupy three MLS leaves in the same conversation.

### Key separation

MLS signature keys, HPKE material, KeyPackages, leaf keys, epoch secrets, exporter secrets, and other MLS state belong exclusively to the MLS protocol role.

They MUST NOT reuse:

- the account-root private key;
- device identity private keys;
- server signing keys;
- OpenPGP private keys;
- TLS private keys.

A device identity may **bind or certify** MLS credentials and KeyPackages according to the versioned Cords protocol, but certification is not key reuse.

### History semantics

Adding a new authorized device to an account does not automatically give that device historical conversation keys. A newly added MLS leaf receives access according to the MLS state transition that adds it and is intended to obtain current and future conversation state, not retroactive plaintext merely because it belongs to the same account.

Any future encrypted history-transfer or backup mechanism must be specified as a separate protocol and must not weaken this property implicitly.

## Server signing identity

### Role

Each Cords server has a persistent **server signing key** distinct from its TLS/Web PKI credentials.

The server signing identity authenticates Cords application-level state such as:

- server discovery metadata;
- server identity and key-rotation statements;
- memberships and membership credentials;
- authorization or moderation state where specified;
- other server-originated protocol statements explicitly assigned to this key role.

The server signing key does not encrypt user messages and is never an MLS group member.

### Relationship to TLS

TLS and the Cords server signing identity solve different problems.

Web PKI authenticates that a connection reached the HTTPS origin requested by the client. The Cords signing key provides a persistent application identity that can be pinned and recognized across connections.

A valid self-signature from a server key proves internal consistency only. It does not by itself prove that the key belongs to the requested origin.

Clients must not disable normal certificate validation as a substitute for Cords server-key verification, nor accept a changed Cords server key merely because the new key signs its own metadata.

### Rotation

After a server identity has been pinned, a replacement signing key requires an authenticated trust transition. Accepted mechanisms may include a rotation statement signed by the previously trusted key, a trusted invitation containing the new fingerprint, or an explicit user-approved trust reset.

Exact rotation encodings and failure behavior are versioned protocol decisions.

## Server membership versus account authority

Cords deliberately separates **account authority** from **server authority**.

The account root determines which devices are legitimately part of an account. A server determines whether that account/device chain is admitted to the server and what it may do there.

An authenticated request therefore generally requires both:

1. a valid account/device authorization chain; and
2. valid server membership/authorization state.

Neither side substitutes for the other. A server cannot manufacture an account-authorized device, and possession of an account root does not automatically grant membership or permissions on an independently operated server.

## Device enrollment

Adding a device is a cryptographic authorization event, not merely a successful server login.

A new device generates its own private key material locally. Enrollment must establish that an existing account authority intentionally authorizes the new device and must allow the relevant public identity material to be authenticated before the account-root authorization is accepted.

Permitted enrollment designs may include:

- direct QR-code transfer or verification between an existing trusted device and the new device;
- a short authentication string verified through an independent human comparison;
- an encrypted recovery bundle controlled by account-held recovery material;
- an optional OpenPGP attestation that independently binds the native Cords account identity.

A server-provided login code, password reset, email link, or equivalent server-only authentication MUST NOT by itself authorize a device to obtain end-to-end decryption capability.

Server authentication may assist transport and admission during enrollment, but the cryptographic device authorization must ultimately derive from account-controlled authority.

## Recovery

Recovery exists to restore account authority without turning the hosting server into a decryption escrow service.

Recovery material must be protected independently of ordinary server credentials. Any recovery bundle containing private key material must be encrypted before server storage or transport when the server is capable of observing the stored bytes.

A recovery process may restore the account root or establish a cryptographically authorized successor root, depending on the eventual versioned recovery design. That distinction must be explicit. Implementations must not silently replace the account root merely because a user regained access to a server account.

Recovery of account authority does not automatically imply recovery of historical MLS epoch secrets. Historical-message recovery, if supported, is a separate encrypted backup or transfer problem.

## OpenPGP attestation

OpenPGP is an optional external attestation mechanism. It is not the native Cords account identity and is not required for normal Cords operation.

An OpenPGP identity may sign a domain-separated statement binding an OpenPGP identity to a Cords account-root public key or fingerprint. Clients may present that attestation as additional evidence when users verify an account.

OpenPGP keys MUST NOT be used directly as:

- account-root keys;
- device identity keys;
- MLS signature or encryption keys;
- attachment-encryption keys;
- server signing keys.

Cords chat encryption must continue to function for users who have no OpenPGP identity at all.

## Attachment encryption

Attachment encryption is a separate cryptographic role from MLS group state and identity signing.

Attachment keys are per-attachment or otherwise scoped according to the attachment protocol. They must not be derived by directly repurposing account, device, server, OpenPGP, or TLS private keys.

The encrypted event may carry the protected descriptor or keying information necessary for authorized recipients to obtain the attachment plaintext. Servers may store and relay encrypted attachment blobs but must not require plaintext filenames, attachment keys, or attachment contents.

The concrete attachment construction is defined separately from this identity architecture.

## Key separation matrix

| Key/material | Primary authority or purpose | May sign/certify | Must not be used for |
|---|---|---|---|
| Account root | Portable account authority | Device authorization, revocation, recovery/root transitions | Chat encryption, MLS leaf operation, TLS, server signing |
| Device identity | One authorized installation | Device operations and MLS credential bindings as specified | Account-root authority, raw MLS key reuse, server signing |
| MLS keys/state | One device within one conversation/group state | MLS protocol operations | Account/device/server identity, OpenPGP, TLS |
| Server signing key | Persistent Cords server identity | Discovery, membership/policy state, rotations | User message encryption, MLS membership, TLS substitution |
| TLS key | HTTPS transport identity | TLS protocol only | Cords account/device/server application signatures |
| OpenPGP key | Optional external identity attestation | Cords account-binding attestation | Native Cords identity or chat/attachment encryption |
| Attachment key | Attachment confidentiality/integrity | Attachment cryptographic construction only | Identity, membership, MLS identity, server authentication |
| Local master key | Protect client secrets at rest | Local wrapping only | Network identity or protocol signatures |

Algorithm compatibility never overrides this table. Two roles using Ed25519, X25519, or related primitives still require independent key material unless a future protocol revision explicitly defines a safe derivation hierarchy and records that decision architecturally.

## Local secret protection

Long-lived private keys, recovery material, MLS state, attachment keys retained locally, and equivalent secret state must be encrypted at rest using a local master key or an equivalent platform protection mechanism.

The local master key must not be stored unprotected beside the database it protects. Appropriate protection mechanisms include operating-system credential stores, hardware-backed providers, or passphrase-derived wrapping where platform facilities are unavailable or deliberately not used.

The WebView/presentation layer must not receive long-lived private keys or raw MLS state. Cryptographic operations belong behind the native client-core boundary.

## Fingerprints and human verification

Cords must expose stable, human-verifiable fingerprints for identities where users need to establish or inspect trust. The exact fingerprint representation, encoding, and UX are protocol/UI decisions, but fingerprints must be derived deterministically from the canonical public identity they represent.

A fingerprint is an identifier for verification, not a secret and not an authorization credential by itself.

Short authentication strings may be used for interactive comparison but must be derived from a protocol that binds the complete identities and context being verified. They must not be treated as globally unique long-term fingerprints unless explicitly designed as such.

## Signed statements and domain separation

Every Cords-specific signed object must have an unambiguous versioned signing representation and a role-specific domain-separation prefix or equivalent construction.

Signatures must not be computed over ad hoc JSON text. A parser must not be able to reinterpret a valid signed object from one protocol role as another valid object with different semantics.

Account authorizations, revocations, recovery statements, device bindings, server metadata, server rotations, and OpenPGP attestations therefore require distinct signing domains even when they use the same underlying signature algorithm.

Canonical encodings and exact vectors belong in `cords-protocol` and the test-vector suite. Introducing or changing a signed representation requires the corresponding protocol/version and ADR work.

## Generations, ordering, and replay resistance

Account and device authorization state must contain sufficient monotonic or causally ordered information to prevent an older valid authorization statement from silently undoing a newer revocation or transition.

The existing architecture expects generation-numbered device authorization and revocation. The precise generation model must be specified before Phase 1 wire types are frozen.

Servers and clients must reject cryptographically valid but stale state where accepting it would roll identity state backward.

Protocol operations must additionally use nonces, challenges, expiration, idempotency keys, transcript binding, or equivalent mechanisms where appropriate to prevent replay in their own threat context. A signature alone does not establish freshness.

## Compromise boundaries

The system should preserve the following boundaries:

**Server compromise:** an attacker controlling a server may manipulate availability, policy, membership processing, routing, retention, and ciphertext delivery, but should not thereby obtain account-root private keys, device private keys, MLS plaintext, attachment plaintext, or attachment keys.

**Single-device compromise:** an attacker controlling one authorized device may obtain the secrets and plaintext available to that device. The compromise must not imply possession of other devices' private keys. Revocation plus conversation membership updates should exclude the compromised device from future secrets.

**Account-root compromise:** compromise of the account root permits fraudulent account-level authorization actions and is therefore severe. It does not retroactively reveal MLS epoch secrets that the attacker does not otherwise possess.

**TLS-key compromise:** compromise of a server's TLS private key is not automatically compromise of its pinned Cords signing identity, and vice versa. Each incident requires independent trust handling.

**OpenPGP-key compromise:** compromise of an optional OpenPGP attestation key affects the credibility of that external attestation. It does not directly yield native Cords private keys or conversation secrets.

## Algorithm agility

Key roles are architectural; algorithms are versioned choices within those roles.

The initial architecture uses Ed25519 for account-root and server signing identities and expects the mandatory initial MLS suite `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`. Future protocol versions may introduce new algorithms without collapsing the separation between account, device, MLS, server, TLS, OpenPGP, attachment, and local-storage roles.

Algorithm migration must define how old and new identities are bound, how downgrade is prevented, how fingerprints are represented, and how existing trust survives or deliberately resets. Such changes require an ADR and protocol-version consideration.

## Non-goals and forbidden shortcuts

The key system explicitly does not provide or permit the following shortcuts:

- no server escrow of unwrapped account-root keys;
- no server-only password/login recovery that silently grants message-decryption authority;
- no reuse of one Ed25519 private key for account, device, MLS, server, or OpenPGP roles;
- no use of OpenPGP as the Cords message-encryption protocol;
- no hidden server MLS member;
- no assumption that adding a device grants historical conversation plaintext;
- no disabling TLS certificate validation because Cords metadata is signed;
- no automatic acceptance of an unexpected server-signing-key change;
- no storage of long-lived client secrets in WebView/TypeScript state;
- no storage of the local database master key unprotected beside that database;
- no treating possession of ciphertext, a server session, or a membership record as proof of possession of an account/device private key.

## Ownership by component

The architectural ownership of key-system responsibilities is:

- `cords-identity`: account roots, device identities, authorization/revocation semantics, fingerprints, recovery identity transitions, and optional OpenPGP attestations;
- `cords-crypto`: MLS integration, attachment cryptography, secret-wrapping interfaces, and cryptographic adapters;
- `cords-protocol`: canonical signed wire structures, domain-separated encodings, protocol versions, and stable interoperability vectors;
- `cords-client-core`: orchestration of identity, enrollment, trust, storage, synchronization, and cryptographic operations without exposing secrets to the presentation layer;
- `cords-server-core`: membership, authorization, server identity, server-key rotation, admission, and policy enforcement without access to message plaintext;
- `cords-storage`: persistence interfaces that preserve the client/server secret boundary and enforce the required at-rest handling contracts.

Concrete library types should not leak across these boundaries merely for implementation convenience.

## Changes to this architecture

The following changes are architectural and require an ADR in addition to code changes:

- changing the account-root authority model;
- allowing a server to possess new client secret material;
- changing device authorization or revocation semantics;
- changing the relationship between device identity and MLS credentials;
- introducing private-key reuse or deterministic cross-role derivation;
- changing recovery authority;
- making OpenPGP mandatory or assigning it a native cryptographic role;
- changing server-key pinning or rotation trust semantics;
- changing the confidentiality boundary for messages or attachments;
- introducing a new major cryptographic dependency or primitive for these roles.

Implementation details that preserve these semantics may evolve without rewriting this document, subject to the normal protocol-version and ADR requirements elsewhere in the project.

## Summary invariant

The Cords key system has one central invariant:

> **Account authority, device authority, conversation cryptography, server authority, transport identity, external attestation, attachment encryption, and local secret protection are separate cryptographic roles. Trust may be explicitly bound across those roles, but private key material is not casually shared between them.**

That separation is what allows an account to remain portable, a server to remain independently operated, devices to be independently revocable, and message plaintext to remain under the control of authorized clients rather than infrastructure operators.
