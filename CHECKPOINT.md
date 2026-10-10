# IMS Implementation Checkpoints

This document defines an ordered milestone checklist for a modern Internet Messaging Service (IMS) consisting of one or more servers and client applications.

It is intentionally implementation-agnostic. A specific IMS may use different protocols, storage engines, user interfaces, cryptographic libraries, deployment systems, or federation models, but a production-quality service should be able to satisfy the intent of every checkpoint that applies to its product model.

The checkpoints are ordered primarily by dependency, security importance, and operational priority. Later milestones generally assume the earlier milestones are complete and validated. Implementations should resist marking a checkpoint complete merely because an API, type, screen, or placeholder exists: a checkpoint means the behavior works end-to-end, survives realistic failure cases, and has appropriate tests.

Where a product intentionally omits a feature, document the decision and its consequences rather than silently treating the checkpoint as complete.

Checked items were re-audited against the clean `v0.2.0` source tree on October 9, 2026. Runtime claims rely on the bounded October 3 [encrypted-channel evidence](docs/reports/encrypted-milestone.md), [lifecycle report](docs/reports/lifecycle-acceptance.json), and [packaged-desktop report](docs/reports/desktop-packaged-smoke.json); source inspection alone was not used as runtime proof. Unchecked items and whole phases remain incomplete. LAN, local-build, workflow-contract, and packaged-Windows evidence is not hosted CI, public-PKI deployment, publication, or production release proof. `cargo deny check` remains blocked by RUSTSEC-2026-0173 without a policy suppression.

## Completion standard

A milestone is complete only when:

- [ ] the relevant server behavior is implemented where applicable;
- [ ] the relevant client behavior is implemented where applicable;
- [ ] persistent state survives restart where persistence is required;
- [ ] expected failure and recovery paths are implemented;
- [ ] security boundaries are enforced rather than merely documented;
- [ ] automated tests cover the important success, rejection, and tamper cases;
- [ ] the behavior is documented sufficiently for another developer or operator to reproduce and maintain it.

The boxes above describe the standard and are not themselves project milestones.

---

## 0. Repository, build, and trust foundation

Nothing higher in the stack matters if a clean checkout cannot be built, tested, or understood.

### Project integrity

- [x] A clean checkout contains every source file, migration, schema, fixture, and build manifest required to build the supported server and client.
- [ ] Server and client builds are reproducible from documented commands.
- [x] Dependency versions are locked or otherwise reproducibly resolved.
- [x] Supported runtime and toolchain versions are explicitly documented.
- [ ] CI builds and tests the server and every supported client platform.
- [ ] Formatting, linting, unit tests, integration tests, dependency/security checks, and secret scanning are automated.
- [ ] Release artifacts are built from identifiable source revisions and can be independently verified against them.
- [x] Architecture and trust-boundary changes have a durable decision-record process.

### Baseline security

- [x] Secrets, credentials, private keys, recovery material, plaintext messages, and equivalent sensitive state are excluded from logs by default.
- [x] Sensitive configuration is not committed with production credentials.
- [x] Cryptographically secure randomness is used wherever security depends on randomness.
- [x] Unsafe or unaudited cryptographic constructions are prohibited unless explicitly reviewed and justified.
- [x] Protocol inputs are size-bounded and strictly parsed before expensive or state-changing processing.
- [x] Public errors do not expose internal stack traces, database errors, secrets, or cryptographic state.

---

## 1. Server bootstrap and client connectivity

The first useful milestone is a client being able to locate a server, establish a secure connection, and determine what it is speaking to.

### Server

- [x] The server starts from a documented configuration with safe defaults.
- [x] The server exposes liveness and readiness information.
- [x] Readiness is not reported until required persistent storage and critical dependencies are usable.
- [x] The server has a persistent application identity distinct from its TLS certificate where the protocol requires long-lived server identity.
- [x] The server publishes protocol/version/capability metadata in a deterministic, authenticated form.
- [ ] Server identity rotation has a defined authenticated transition mechanism.
- [x] Graceful shutdown stops accepting new work and safely finishes or aborts in-flight state changes.

### Client

- [x] The client accepts or discovers a server endpoint without disabling normal TLS certificate validation.
- [x] The client can retrieve and validate server metadata before authentication.
- [x] The client rejects malformed, unauthenticated, incompatible, or cryptographically invalid server metadata.
- [x] The client records the server identity it has trusted and detects unexpected identity changes.
- [ ] A server identity change produces an explicit trust transition rather than silent acceptance.
- [ ] The client can distinguish connection failure, TLS failure, server-identity failure, protocol incompatibility, and temporary server unavailability.

---

## 2. Durable storage and restart safety

Messaging software is a distributed state machine wearing a chat bubble as a disguise. Persistence therefore comes before interesting chat features.

### Server

- [x] Persistent server state is stored behind a defined storage abstraction.
- [x] Database/schema migrations are versioned, ordered, restart-safe, and tested from supported upgrade points.
- [x] Server identity survives restart and cannot be silently regenerated over existing state.
- [x] Critical mutations support idempotency or equivalent duplicate suppression.
- [x] Replaying a successfully completed request does not create duplicate durable state.
- [x] Partial failures cannot leave security-critical state in an ambiguous half-committed condition.
- [ ] Backup and restore procedures exist for server-owned durable state.

### Client

- [x] Client state uses durable local storage rather than UI state as its source of truth.
- [x] Local schema migrations are versioned and tested.
- [x] Account, device, trust, conversation, synchronization, and retry state survive restart as appropriate.
- [x] Security-sensitive local state is encrypted at rest using a key not simply stored beside the encrypted database.
- [x] Interrupted writes and interrupted synchronization can recover without corrupting identity or conversation state.
- [ ] The client can detect an incompatible or corrupted local database and fail safely.
- [x] Independently password-wrapped local account vaults reject cross-account passwords in core tests.
- [x] Legacy passphrase migration copies and verifies account/device identity before registration and preserves the source vault.
- [ ] Multi-account picker, switching, removal, inactivity lock, suspend lock, and OS-session-lock behavior have packaged-runtime acceptance on every supported desktop platform.

---

## 3. Account and device identity

An IMS must know the difference between an account, a device, a server, and a conversation participant. Collapsing them into one magic key works wonderfully until the first lost phone.

### Account authority

- [x] Accounts have a durable cryptographic or equivalently strong identity independent of a single login session.
- [x] Account authority can authorize multiple devices without sharing one routine-use private key among them.
- [x] Account identity fingerprints or equivalent human-verifiable identifiers are defined.
- [ ] Account identity material has a documented backup/recovery model.
- [ ] Recovery does not give the server unilateral access to end-to-end encryption keys unless that is an explicit, prominently documented product property.

### Device authority

- [x] Every client installation has a unique device identity.
- [x] Device authorization is cryptographically bound to the owning account or equivalently authenticated.
- [x] Device authorization statements are replay-resistant and versioned/generation-aware where necessary.
- [ ] A newly authorized device can authenticate independently without copying another device's routine private keys.
- [x] Device revocation is supported.
- [x] Revoked devices cannot create new authenticated sessions or obtain new conversation key material.
- [ ] Device replacement and device loss have documented workflows.

### Key separation

- [x] Account, device, server, transport, conversation, attachment, recovery, and external-attestation key roles are explicitly separated.
- [x] Private key material is not reused across cryptographic roles merely because algorithms or key formats are compatible.
- [x] Signed structures use domain separation and deterministic signing encodings.
- [ ] Key rotation semantics are defined for every long-lived key class.
- [x] Compromise of one key class has a documented and bounded effect on the others.

---

## 4. Authentication, sessions, and membership

Possessing an account is not the same thing as being allowed into a particular server or community.

### Authentication

- [x] The server authenticates possession of an authorized device identity using a replay-resistant challenge or comparably strong mechanism.
- [x] Authentication does not transmit reusable private credentials in plaintext.
- [x] Authentication challenges expire and cannot be reused.
- [x] Session credentials are scoped, expiring, revocable, and securely stored by the client.
- [ ] Logout/revocation invalidates server-side authorization as intended rather than merely deleting a local token.
- [x] Rate limits and abuse controls protect authentication endpoints.

### Admission and membership

- [x] The server has an explicit account admission policy such as open, invite, approval, or another documented mechanism.
- [x] Membership is distinct from account/device identity.
- [x] Membership credentials or records bind the account/device to the correct server identity.
- [ ] Join, leave, suspension, ban, and rejoin semantics are defined.
- [ ] Server membership changes survive restart and are auditable at an appropriate level.
- [ ] A client can clearly represent whether it is authenticated, admitted, suspended, revoked, or disconnected.

---

## 5. Authorization and server policy

Authentication answers who. Authorization answers whether they are allowed to do the thing they are enthusiastically attempting.

### Server

- [x] Authorization is enforced in core service logic, not solely in UI or HTTP route handlers.
- [x] Permissions use stable machine-readable capabilities rather than display-role names as protocol logic.
- [ ] Roles/capabilities cover creation, reading, posting, moderation, membership administration, and other privileged operations as applicable.
- [ ] Permission changes take effect predictably for existing sessions.
- [x] Unauthorized requests are rejected without leaking protected state.
- [ ] Privileged actions have an appropriate audit trail.

### Client

- [ ] The client receives enough policy information to present valid actions without treating UI hiding as security enforcement.
- [ ] Permission changes are reflected without requiring destructive local resets.
- [ ] The client handles authorization rejection as a normal state transition rather than corrupting or discarding unrelated local state.

---

## 6. Conversation and channel model

Before sending messages, the service needs durable, synchronizable objects to send them to.

- [ ] The server can create, identify, enumerate, and retire conversation routes/channels according to policy.
- [x] Conversation membership is explicit and access-controlled.
- [x] Server-visible routing metadata is clearly separated from end-to-end protected content.
- [x] Stable conversation identifiers survive reconnects and client restarts.
- [x] Clients can synchronize the conversations they are entitled to know about.
- [x] Membership/roster changes have an ordered, conflict-safe model.
- [ ] Deleted or inaccessible conversations have defined client synchronization semantics.
- [x] Conversation metadata reveals no more plaintext information to the server than the product intentionally requires.

---

## 7. End-to-end conversation encryption

For an E2EE IMS, this is the point where the server becomes a ciphertext courier rather than an involuntary diary keeper.

### Cryptographic group/session layer

- [x] Conversation encryption uses a reviewed protocol appropriate to the product's one-to-one and/or group messaging model.
- [x] Each authorized device participates with conversation-specific cryptographic state rather than an account-wide shared conversation key.
- [x] Conversation cryptography is isolated behind an internal interface rather than leaking library-specific state throughout application code.
- [x] Initial key establishment authenticates participant/device identities.
- [x] Participant additions and removals cause the required cryptographic state transitions.
- [x] Removed/revoked devices cannot obtain future conversation keys.
- [x] Newly added devices do not silently receive historical plaintext unless explicit history transfer is a documented feature.
- [x] Cryptographic commits/updates are authenticated and ordered.
- [x] Malformed, stale, replayed, or unauthorized cryptographic updates are rejected.
- [x] Cryptographic state survives client restart securely.
- [ ] Protocol interoperability and deterministic vectors exist for protocol-owned encodings.

### Server relationship

- [x] The server can coordinate or relay cryptographic membership work without possessing conversation plaintext keys.
- [x] The server cannot add itself as an invisible encrypted-conversation participant.
- [ ] Server-side membership policy and cryptographic conversation membership cannot silently diverge indefinitely.
- [x] Recovery from interrupted roster/key updates is defined and tested.

---

## 8. Core message delivery

Only now do we earn the revolutionary capability of sending "hi" over the Internet.

### Message model

- [x] Messages have stable unique identifiers.
- [x] Client-generated events have idempotency identifiers or equivalent duplicate suppression.
- [x] Sender account and device identity are authenticated inside the protected message/event layer where E2EE is used.
- [x] Server-visible envelopes contain only the routing/order information the server genuinely requires.
- [x] Message body, rich payload, reply target, reaction value, and other private semantics are end-to-end protected where promised by the product.
- [x] Message/event schemas are versioned or extensible without ambiguous parsing.

### Delivery

- [x] The client can send an encrypted message and receive durable server acknowledgement.
- [x] The server assigns or maintains an authoritative ordering mechanism for each route where ordering is required.
- [x] Recipients can retrieve missed messages after reconnecting.
- [x] Duplicate uploads and downloads do not produce duplicate user-visible messages.
- [x] Temporary network failure queues outgoing work for safe retry.
- [ ] Permanent rejection is distinguishable from retryable failure.
- [ ] Multiple devices on the same account converge on consistent conversation state.
- [x] Message delivery continues correctly across server and client restarts.

---

## 9. Synchronization and offline operation

A messaging client that only works while continuously connected is a demo with excellent self-esteem.

- [x] The client maintains explicit synchronization cursors/checkpoints rather than assuming it has seen everything.
- [x] Reconnect resumes from durable synchronization state.
- [x] Gaps in ordered event streams are detected and repaired.
- [x] The server supports bounded catch-up without requiring complete history retransmission on every connection.
- [ ] Clients can operate usefully during temporary disconnection where product semantics permit it.
- [x] Outgoing offline actions are replayed safely after reconnect.
- [ ] Conflicting edits/state changes have deterministic resolution semantics.
- [ ] Synchronization is resilient to duplicated, delayed, and out-of-order network delivery.
- [ ] A full resynchronization path exists for damaged or irrecoverably stale local synchronization metadata.

---

## 10. Message lifecycle features

- [ ] Replies/references are represented as events or protected message relationships rather than requiring server plaintext inspection.
- [ ] Message edits preserve authorship and target integrity.
- [ ] Message deletion/redaction has clearly defined local, server, and recipient semantics.
- [ ] Reactions are authenticated and synchronized.
- [ ] Clients correctly process lifecycle events received before the referenced message during synchronization.
- [ ] Unauthorized users/devices cannot edit, delete, or impersonate another sender's content.
- [ ] Retention behavior distinguishes server deletion from guaranteed deletion of recipient-held copies.

---

## 11. Multi-device lifecycle and secure recovery

- [ ] A user can enroll an additional device through an authenticated workflow.
- [ ] Existing trusted devices can verify or approve enrollment where the architecture calls for it.
- [ ] Enrollment exposes a human-verifiable comparison such as a QR code, fingerprint, or short authentication string where useful.
- [ ] A server login credential alone does not silently grant historical end-to-end decryption capability.
- [ ] Recovery material is encrypted and integrity-protected.
- [ ] Recovery restores account authority without unnecessarily cloning device identity keys.
- [ ] Lost-device revocation is available from another authorized recovery path.
- [ ] Account-root/key rotation after suspected compromise is defined.
- [ ] Clients visibly distinguish verified, newly added, revoked, and otherwise noteworthy devices.
- [ ] Optional history transfer between devices is separately authenticated and end-to-end encrypted.

---

## 12. Direct messaging

- [ ] Users can establish a one-to-one conversation using stable account identities rather than fragile session identifiers.
- [ ] All currently authorized participating devices are represented according to the encryption protocol's device model.
- [ ] Adding or removing a device updates direct-message cryptographic membership correctly.
- [ ] Direct-message discovery/invitation does not unnecessarily expose private social-graph information.
- [ ] Blocking semantics are defined for new messages, existing conversations, device additions, and presence-like metadata.
- [ ] Abuse controls do not require the server to decrypt message bodies.
- [ ] A DM can survive device churn, reconnects, and server restarts without silently weakening encryption.

---

## 13. Encrypted attachments and media

- [ ] Attachment contents are encrypted client-side with fresh attachment-specific key material.
- [ ] Attachment encryption keys are delivered only through end-to-end protected conversation content.
- [ ] The server stores only ciphertext and the minimum metadata required for storage/delivery.
- [ ] Large files support chunked or streaming authenticated encryption.
- [ ] Interrupted upload and download can resume without weakening integrity guarantees.
- [ ] Recipients verify attachment integrity before exposing completed content to applications/users.
- [ ] Filename, media metadata, thumbnails, and descriptions follow the product's stated privacy boundary.
- [ ] Server-side quotas and garbage collection operate without needing attachment plaintext.
- [ ] Orphaned/expired attachment blobs can be safely reclaimed.
- [ ] Clients apply platform-appropriate safe-file handling to untrusted downloaded content.

---

## 14. Real-time transport and presence-like state

- [x] The service supports an efficient authenticated real-time transport for event notification/delivery.
- [x] Realtime transport loss falls back to the durable synchronization path without losing messages.
- [ ] Reconnection uses bounded backoff and avoids synchronized reconnect storms.
- [ ] Connection authentication can be renewed without silently extending revoked credentials.
- [ ] Presence, typing indicators, read state, and similar ephemeral metadata are explicitly classified by privacy level before implementation.
- [ ] Ephemeral features expire naturally and do not become accidental permanent surveillance records.
- [ ] Users can disable optional presence/read-state disclosures where the product promises such control.

---

## 15. Abuse prevention and moderation

- [ ] The server can suspend and ban accounts/memberships without decrypting ordinary message content.
- [ ] Rate limits exist for authentication, joining, messaging, uploads, invitations, and other abuse-sensitive operations.
- [ ] Moderation actions are authorization-checked and auditable.
- [ ] Blocking and reporting workflows clearly state what information is disclosed to server operators.
- [ ] User-submitted reports can intentionally disclose selected plaintext/evidence without creating general server decryption capability.
- [ ] Spam/abuse controls have bounded false-positive recovery mechanisms.
- [ ] Server operators can manage policy without possessing account roots or conversation keys.

---

## 16. Privacy, metadata minimization, and local data control

- [x] The project documents what metadata the server necessarily learns.
- [x] The server does not collect message semantics merely because doing so would simplify implementation.
- [ ] Logs and metrics minimize stable user/device identifiers and high-cardinality sensitive labels.
- [x] Telemetry is absent, minimal, or explicitly documented and controllable according to product policy.
- [x] Local plaintext caching behavior is documented and configurable where appropriate.
- [x] Local sensitive caches are encrypted at rest.
- [ ] Account export and local-data deletion semantics are defined.
- [ ] Server retention policy is documented and enforceable.
- [ ] Backups preserve the same confidentiality expectations as primary storage.

---

## 17. Operational maturity

### Server operations

- [x] Production deployment has a documented supported topology.
- [x] TLS configuration is production-safe and certificate renewal is automated or operationally documented.
- [x] Health checks distinguish process liveness from actual service readiness.
- [ ] Structured logs include request/correlation identifiers without leaking secrets.
- [ ] Metrics expose capacity, latency, error, queue, storage, and synchronization health without exposing sensitive content.
- [ ] Operators can perform backup, restore, migration, key rotation, and disaster recovery using documented procedures.
- [ ] Storage exhaustion, database outage, clock problems, and dependency failures fail predictably.
- [ ] Resource limits prevent individual clients from trivially exhausting memory, CPU, database connections, or storage.
- [ ] Upgrade and rollback procedures are tested across supported versions.

### Client operations

- [ ] Client updates preserve local identity and conversation state.
- [ ] Failed updates do not strand users with an unreadable local database without a recovery path.
- [ ] Crash reports and diagnostics redact cryptographic and message secrets.
- [x] Users can inspect connection/trust/device state sufficiently to diagnose security-relevant problems.
- [ ] Exportable diagnostic information is explicitly scrubbed of secrets.

---

## 18. Compatibility and protocol evolution

- [x] Protocol version negotiation is explicit.
- [x] Application version and protocol version are independent concepts.
- [x] Optional capabilities/features are advertised explicitly rather than inferred from version numbers alone.
- [ ] Unknown optional fields/events can be handled according to documented forward-compatibility rules.
- [x] Security-critical unknown fields or algorithms fail closed where required.
- [x] Signed encoding changes receive new domains/versions rather than ambiguously changing existing encodings.
- [x] Cryptographic algorithm agility is designed without opportunistic downgrade negotiation.
- [ ] Supported upgrade paths preserve trust pins, identities, memberships, and encrypted conversation state.
- [x] Interoperability vectors exist for stable wire and cryptographic structures.
- [ ] Compatibility tests exercise mixed supported client/server versions.

---

## 19. Release and supply-chain security

- [ ] Official server images/packages and client artifacts are cryptographically signed or otherwise verifiably authenticated.
- [ ] Release provenance identifies the source revision and build process.
- [ ] Clients verify update authenticity before installation where automatic updates exist.
- [ ] Dependency vulnerability scanning is part of normal maintenance.
- [ ] Compromised or revoked release-signing credentials have a documented recovery/rotation procedure.
- [ ] Build and release credentials are isolated from runtime server credentials and messaging identity keys.
- [ ] Release automation does not print or persist signing secrets beyond its required lifetime.
- [ ] A clean environment can independently verify published release signatures/provenance.

---

## 20. Adversarial and failure testing

- [x] Protocol parsers are tested with malformed, truncated, oversized, duplicated, and unexpected inputs.
- [x] Authentication is tested against replay and stale-challenge attempts.
- [x] Signed structures are tested against field tampering and key/fingerprint substitution.
- [ ] Authorization tests prove forbidden actions remain forbidden through every exposed interface.
- [ ] Conversation cryptography is tested across add/remove/update, simultaneous changes, stale state, and interrupted commit scenarios.
- [ ] Synchronization tests inject duplicates, gaps, delay, reordering, disconnects, and restarts.
- [ ] Storage tests inject failed migrations, interrupted writes, unavailable databases, and corrupted local state.
- [ ] Attachment tests cover corruption, truncation, replay, resume boundaries, and malicious metadata.
- [ ] Secret-scanning tests or equivalent checks prevent representative credentials/private material from entering releases or logs.
- [ ] Fuzzing targets security-sensitive parsers and protocol-owned encodings where practical.
- [x] End-to-end tests exercise a real server and multiple independent client/device instances.

---

## 21. Accessibility and usable security

A technically perfect secure messenger that humans cannot operate correctly has simply invented an unusually expensive denial-of-service attack against its own users.

- [ ] Critical messaging functions are keyboard accessible on applicable platforms.
- [ ] Security and trust states are conveyed with text/semantics rather than color alone.
- [ ] Screen readers can identify conversations, senders, message state, controls, and security warnings.
- [ ] Trust warnings explain the actual event and safe choices without training users to click through generic alarms.
- [ ] Device enrollment and verification are understandable without requiring cryptographic expertise.
- [ ] Recovery workflows clearly explain what can and cannot be recovered.
- [ ] Destructive actions distinguish local deletion, server deletion, and recipient-held copies.
- [ ] Offline, reconnecting, retrying, failed, sent, and delivered states are distinguishable.
- [ ] Accessibility does not require weakening authentication or encryption boundaries.

---

## 22. Advanced communications

These features are later priority because a reliable messenger should first master messages before attempting to become the entire telecommunications industry.

### Voice/video and RTC

- [ ] Signaling is authenticated and end-to-end protected where required by the threat model.
- [ ] Call participants are bound to established account/device identities.
- [ ] One-to-one audio works across supported network conditions.
- [ ] NAT traversal/TURN behavior is documented and does not silently weaken transport security.
- [ ] Call security state is visible to users.
- [ ] Video preserves the established signaling and participant-authentication guarantees.
- [ ] Group-call architecture has an explicit trust model for relays/SFUs.

### Optional advanced features

- [ ] Multi-server/federated operation, if supported, authenticates remote servers and does not collapse account/device trust into DNS alone.
- [ ] Federation failures cannot forge local membership or bypass local policy.
- [ ] Cross-server conversations preserve the advertised end-to-end encryption properties.
- [ ] Search, history backup, or history transfer preserves the documented plaintext boundary.
- [ ] Hardware-backed key storage can be used without making hardware possession the only viable recovery path unless explicitly intended.

---

## 23. Production readiness checkpoint

An IMS should not call itself production-ready until all applicable earlier checkpoints are satisfied and the following can be demonstrated as one coherent system.

- [ ] A new operator can deploy a server from the documentation without undocumented secrets or manual database surgery.
- [ ] A new user can install a client, verify/connect to a server, establish an account/device identity, authenticate, and join according to server policy.
- [x] Two independent accounts can establish an authorized encrypted conversation and exchange messages without the server obtaining plaintext.
- [x] Both users can disconnect, restart their clients, reconnect, and converge on correct state.
- [ ] A second device can be securely enrolled and a lost device can be revoked.
- [ ] Device revocation prevents future authenticated access and future conversation-key access.
- [x] Server restart, client restart, temporary network loss, duplicate requests, and delayed events do not corrupt or duplicate conversation state.
- [ ] Encrypted attachments can be uploaded, resumed, downloaded, authenticated, and garbage-collected without server plaintext access.
- [ ] Server backup and restore preserve server-owned state without granting operators end-to-end plaintext keys.
- [ ] Client upgrade preserves identity, trust, local storage, and encrypted conversation state.
- [ ] Security-sensitive failures are visible and actionable without encouraging users to bypass verification.
- [ ] Supported release artifacts and server deployments pass the complete automated test/security/release pipeline.

---

## 24. Long-term maintenance checkpoint

Completion is not permanent. Protocols age, dependencies rot, certificates expire, operating systems change, and somebody will eventually discover that a library everyone trusted has been abandoned since 2029.

- [ ] Supported server/client versions and end-of-life policy are documented.
- [ ] Security advisories and dependency updates are reviewed continuously.
- [ ] Cryptographic algorithms and protocol dependencies have migration plans before deprecation becomes an emergency.
- [ ] Old migrations and upgrade paths are periodically tested from real supported historical releases.
- [ ] Backup restoration is tested, not merely assumed from successful backup creation.
- [ ] Key-rotation and compromise-recovery procedures are periodically exercised.
- [ ] Privacy/threat-model documentation is updated when features change what servers, peers, or third parties can observe.
- [ ] Major architectural assumptions remain captured in durable specifications/ADRs rather than residing only in issue threads or contributor memory.
- [ ] Production incidents produce regression tests and, where appropriate, architecture/documentation updates.
- [ ] Every release continues to satisfy all applicable checkpoints above; previously completed milestones are not presumed permanently solved.

---

## Interpretation

This checklist describes capabilities and properties, not a mandated technology stack. For example, an implementation can satisfy secure group messaging without MLS if it uses another suitable reviewed design, and it can satisfy durable server storage without PostgreSQL. What matters is the security, reliability, interoperability, recoverability, and usability property represented by the checkpoint.

Likewise, a non-E2EE service cannot honestly check the end-to-end-encryption-specific boxes merely because TLS protects traffic between client and server. Such a service may still be a functioning IMS, but those milestones remain intentionally incomplete because the server occupies a different trust position.

The ordering is deliberately conservative: establish reproducible software, secure connectivity, durable state, identity, authentication, authorization, and conversation semantics before layering on encrypted messaging, rich features, RTC, or federation. Implementations may develop pieces in parallel, but dependencies should be validated in roughly this order before later milestones are considered complete.
