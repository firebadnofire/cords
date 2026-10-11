# ADR 0007: Rework security contracts and checkpoint activation

- Status: Accepted by explicit product requirements; implementation is gated below
- Date: 2026-10-10
- Extends KEY-SYSTEM.md's ordering, recovery and compromise requirements.
- Supersedes ADR 0005's exceptional readmission of burned identities **when C2's
  terminal-burn compatibility gate activates**. C0 does not change that runtime.
- Clarifies ADR 0006's local archive retention and deletion boundaries.
- Selects authenticated DM courier migration for C5d of this milestone, superseding
  AGENTS.md section 12.3's initial-release deferral only after that gate passes.

## Scope and activation

Preserve all eight approved work areas and the existing Rust/Tauri/Svelte,
account-root/device, MLS, TLS, PostgreSQL and sealed SQLite architecture. Execute
the [checkpoint plan](../rework-plan.md) in small plan → implement → validate units.
Every unit leaves a runnable messenger. Working behavior stays available until its
replacement passes its UI/native/server/persistence acceptance gate.

This ADR establishes security semantics. It does **not** assign new signing bytes,
wire versions, capability strings or database encodings. Before the affected unit
implements them, a scoped protocol decision and byte/signature vectors must lock
those representations, bounds and compatibility negotiation. Existing domains and
authenticated records remain unchanged. Legacy signatures are evidence of their
original object only. No migration may fabricate signatures over new state.

C0 emits no new runtime formats. Missing integration, packaged, platform or hardware
evidence leaves a gate incomplete. A scoped architectural ambiguity stops its
affected unit for an explicit decision; independently validated work may continue.

## Root concurrency, succession and terminal burning

A root-state record must bind version, account/root, generation, operation,
predecessor hashes and resulting device/authority state through deterministic,
domain-separated signatures. Accept only the already verified account/root binding.
An ordinary device certificate confers neither root custody nor unrestricted
enrollment authority. Root move and duplication use this same verified state
machinery; ordinary enrollment transfers only a new device's authorization.

Identical records are idempotent. Ordinary transitions extend a verified predecessor.
Incomplete ancestry is pending, never a reason to overwrite trusted state. Different
valid descendants of a predecessor are a fork even if generations differ. Preserve
both branches and show Identity-state conflict. Arrival order, timestamps, lexical
ordering, higher generations and server preference cannot select a winner.
Conservatively deny authenticated revocations while withholding conflicting grants.
Invalid signatures or substituted roots never supply revocation authority.

Resolution requires an explicitly reviewed root-signed reconciliation referring to
**every known conflicting head**, with a generation exceeding all referenced heads.
Revocations are terminal for a device ID; reenrollment creates a new device identity.
A later discovered omitted branch reopens conflict. Retain branch and reconciliation
evidence across restart. This provides local evidence consistency, not global
consensus or discovery of evidence withheld by a disconnected or malicious server.

**Reconciliation is not recovery from root compromise.** A valid root signature
cannot establish which holder is legitimate. A compromised holder can sign grants
and reconciliations too. Terminal root burning is the response to root compromise,
subject to the distribution and endpoint-compromise limitations below; reconciliation
must never be presented as establishing exclusive legitimate control.

A valid portable burn statement is terminal independently of competing generations.
It dominates grants, reconciliation, recovery, attestations and root-custody copies.
Persist evidence before acknowledging burn or attempting delivery. Retain each
destination's failure and retry state. Known burned roots cannot authenticate, enroll,
authorize manifests, recover ownership, or regain authority from old backups.
Exceptional moderator readmission under ADR 0005 is rejected when this gate activates.
The current server-bound V1 burn remains evidence in its original domain, not a
signature over the future portable object; its cross-version handling requires the
C2a protocol decision and compatibility vectors before activation.

Local enforcement is immediate; compliant remote enforcement begins when evidence
arrives. Unknown memberships, partitions and withheld evidence cannot be claimed
notified. Burning does not erase delivered plaintext or retained archives. A stolen
authorized endpoint may retain plaintext and secrets already received.

Preserve ADR 0005's immutable designation/acceptance chain, **server acceptance +
30 days** maturity, owner-burn handoff or lockdown, permanent bootstrap closure,
operator recovery and retained historical evidence. Operator recovery cannot restore
a burned identity. Never rewind durable ownership/lifecycle heads to reverse a change.

## Report authorization finality and authorizer succession

Use a separate owner-authorized append-only report-authority chain. Each committed
entry binds epoch, sequence, predecessor hash, recipient manifest, applicable
owner/root state and operation. Dedicated recipient keys, selective disclosure and
established HPKE/streaming primitives preserve role separation. Servers receive no
report-decryption keys, attachment keys or original MLS secrets. Newly designated
moderators acquire no automatic historical report access.

Exactly one authorizer key issues fresh approvals within an accepted epoch. An
ordinary recipient-manifest update, including one signed using a root copy, cannot
activate a second authorizer. Recipient changes require owner authorization and
sequencing by the active authorizer. **Revocation is committed when that authorizer
durably appends it, before releasing its signed commit acknowledgement.** Pending
requests and ordinary server-role changes are not committed report revocations.

Serialize approvals and revocations in the same durable ordering. After a revocation
commit, no new approval may include removed recipients. An approval committed before
revocation records prior authorization; one committed afterwards excludes them.
Approvals bind nonce, report ID, exact disclosure digest, committed manifest/head,
epoch and validity interval. Identical retries return the original committed response;
reuse with a different digest or context fails. Pending client reports revalidate
before encryption/submission. Known revocation, burn, expiry or authority conflict
invalidates an unused approval. Previously delivered material cannot be recalled.

Handoff durably freezes the old epoch, binds **one** successor and final head, and
provides a closing proof. The successor acknowledges that exact state before
activation. The old authorizer issues neither further approvals nor another successor.
If safe closure is unavailable, keep reports encrypted locally and pending rather
than automatically promoting another device. Owner succession carries the established
designation, acceptance and maturity evidence. Operator recovery without an independently
verifiable owner chain suspends reporting until explicit authority verification.

Crash recovery replays the durable transaction log. Persist commits and idempotent
responses before external acknowledgement; prepared-only entries are unacknowledged.
Restored/incomplete authority cannot resume signing until continuity is established.
An older backup is not current authority. Conflicting heads freeze reporting.

**A single authorizer cannot prevent equivocation by a compromised or cloned signing
authority.** Durable serialization protects an honest installation's ordinary crash
recovery; it is not proof that the key has no clone. Preserve conflicting evidence
and fail closed when discovered. There is no guarantee of immediate global revocation
awareness during partitions. **Committed revocation and globally observed revocation
are distinct:** an acknowledgement proves a local authority-chain commit, not delivery
to every client. Unknown/withheld conflicts cannot be claimed detected. Partition safety
takes precedence over report availability; compromised authority devices remain a
documented compromise boundary.

## Hardware and optional OpenPGP

Native Ed25519 PIV support depends on actual hardware, firmware, platform, middleware,
algorithm support and PIN/touch policy. Test it separately from OpenPGP. Never substitute
a root algorithm, export a non-exportable key or silently fall back to software.
Declare native YubiKey support only after real compatible-token tests cover signing,
enrollment, revocation, succession, burning, absent/locked-token behavior and restart.
Unsupported configurations remain explicit; software identities remain functional.
An existing hardware-root account cannot fall back to a software root.

Verify optional OpenPGP attestation with an embedded library and independent context,
tamper and burned-root tests. It neither replaces native authority nor revives a burn.

## Local archive contract

Archives belong to the local account vault, independently of server membership,
courier availability or another installation. Preserve partial received history and
its authenticated identity, confidentiality, lifecycle and succession records.
Retention is indefinite by default until explicit local deletion; introduce no
automatic expiry in this milestone. Bookmarks, active caches and retained archives
remain distinct. Leaving/removing a server retains archives and preserves established
purging of unretained active caches. Offer explicit local retention before departure;
retain only received data, never implicitly fetch complete history.

Retained DM history is account-scoped and survives courier changes/unavailability.
Migration manufactures neither history nor relocated attachment objects. Archive
view/search requires vault unlock, but no network, authentication, membership or
available courier. Offline files require locally retained, verified objects and
separately sealed decryption material; a descriptor is not an offline file.

Deletion transactionally removes selected bodies, file references and unshared
retained objects/keys, while retaining minimal burn, revocation and rollback evidence
separately. It removes **application-accessible local content and applicable
cryptographic material**, not remote/recipient copies. It does **not guarantee forensic
erasure** across backups, snapshots, database journals or physical storage. Document
that limitation in the deletion confirmation. Shared retained objects remain accessible
to their other retained references; deletion cannot discard keys they still require.

Removing an entire local account removes its vault/archives subject to the same
forensic limitation. Burning disables authority without automatically destroying
history. Archives remain read-only. Newly added devices and recovery bundles receive
no historical messages or keys automatically.

## Validation and rollback

See [contracts and baseline audit](../protocol/rework-baseline.md) and
[checkpoint acceptance vectors](../../test-vectors/rework-contracts.json).
Semantic scenarios are future acceptance obligations, not passing protocol tests.
Byte/signature vectors and integrated execution are required at the affected gates.

Before external side effects, rehearse matching artifact/backup restores in isolation.
After accepting/emitting security records, MLS traffic or authority commits, never
restore older live heads, keys, counters, outboxes or cursors. Roll back only code
proven compatible with durable current state; otherwise disable the affected feature
and repair forward. Physical restores of pre-migration copies are isolated recovery
exercises, never permission to rewind live cryptographic authority. Every migration
records its fence, backup set, compatible binaries, restart and recovery procedure.
