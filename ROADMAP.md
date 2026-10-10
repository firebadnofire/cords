# Cords Development Roadmap

**Status:** Proposed beta roadmap  
**Target:** v0.1 through v0.9 beta milestones, followed by v1.0 stable  
**Source of technical truth:** [`CHECKPOINT.md`](CHECKPOINT.md)

> **Versioning note:** The checkpoint audit references a clean `v0.2.0` source tree dated October 9, 2026. The v0.1–v0.9 labels below are **proposed product milestones**, not a claim about historical releases or current version numbering. Reconcile tags, release history, and the proposed sequence before adopting these as published version numbers. If v0.1/v0.2 have already shipped, retain the milestone names as planning labels or move outstanding work into the next actual release. Never retroactively declare a release complete.

## How to use this roadmap

- Check a box only after the behavior is demonstrated **end to end**, including relevant server/client behavior, persistence, recovery, authorization, tests, and documentation. UI mockups and source-only implementations do not count.
- `CHECKPOINT.md` is the **implementation-agnostic technical checklist**; this file is the **product release plan**. Do not automatically check a roadmap item because a related technical checkpoint is checked. Many items span several checkpoints.
- Every milestone must pass the [common release gates](#common-release-gates) in addition to its own acceptance criteria.
- Items intentionally excluded from Cords should be recorded under [Decisions and scope](#decisions-and-scope), with a rationale, rather than checked as completed.
- The boxes start **unchecked** unless a complete release-level acceptance case is established. The checkpoint audit already identifies many completed lower-level components; see [Existing implementation baseline](#existing-implementation-baseline).
- Features may be developed in parallel, but **do not ship a milestone** before its dependencies and acceptance criteria pass.

## Release overview

| Milestone | Focus | User-visible outcome |
| --- | --- | --- |
| **v0.1** | Identity and account lifecycle | Safely create, unlock, switch, recover, and manage accounts/devices |
| **v0.2** | Server ownership, admission, and permissions | A private server with consistent channels and enforced member privileges |
| **v0.3** | Reliable text messaging | Everyday encrypted channel messaging with a complete message lifecycle |
| **v0.4** | Direct messaging | Secure one-to-one conversations via mutual-server couriers |
| **v0.5** | Attachments and rich messages | Encrypted file/media sharing, reactions, mentions, and pins |
| **v0.6** | Everyday usability | Search, notifications, organization, and accessible interaction |
| **v0.7** | Voice | Authenticated voice calls and persistent voice channels |
| **v0.8** | Video and operator tooling | Video, screen sharing, and mature moderation/administration |
| **v0.9** | Feature freeze and release candidate | Hardened, tested, documented, distributable product |
| **v1.0** | Stable release | Supported production-ready release with verified upgrades |

## Existing implementation baseline

The October 9, 2026 checkpoint audit marks significant portions of the following as implemented: persistent account/device identity, account vault encryption, server identity verification, storage/migrations, authenticated sessions, core authorization, encrypted conversation state, durable message delivery, retries, and real-time synchronization. **This is not proof that the corresponding product milestone is complete.** The same audit records outstanding work in account recovery, packaged account switching/locking acceptance, admission lifecycle, policy propagation, channel lifecycle, multi-device enrollment, message edits/replies/deletion, DMs, attachments, and operational readiness.

- [ ] Reconcile the audited `v0.2.0` tree and current HEAD against the roadmap before setting release statuses.
- [ ] Link concrete tests, reports, and release artifacts to each checked acceptance criterion.
- [ ] Resolve or document the `cargo deny check` blocker identified in the October 9 audit (RUSTSEC-2026-0173) before declaring a security-clean release.

---

## v0.1: Identity and accounts

**Goal:** Account identity and local account management are safe and usable without developer intervention.

### Deliverables

- [ ] Account creation, login/unlock, logout, and persistent sessions work end to end.
- [ ] Account picker displays saved accounts with display names and safely loaded profile images.
- [ ] Account switching and account removal cannot expose another account's decrypted data.
- [ ] Password-protected local vaults, password changes, and clearly explained insecure-password warnings work as specified.
- [ ] Manual lock, inactivity auto-lock, suspend lock, and OS-session lock behave consistently.
- [ ] Device identity and authorization are visible in the client; revocation is effective server-side.
- [ ] Secure enrollment of another device works without cloning routine-use device private keys.
- [ ] Root identity backup/recovery and lost-device workflows are documented and tested.
- [ ] Root-key burning/self-revocation has a specified irreversible trust effect and a tested client handling path, or is explicitly identified as a **pre-v1 blocker** with a separate milestone owner.
- [ ] Login failures, wrong passwords, revoked devices, and corrupted local state produce actionable errors.

### Release acceptance

- [ ] Two independent local accounts survive client restart, switch without leakage, and lock on configured triggers.
- [ ] Enrollment and revocation are exercised with separate physical or isolated client instances.
- [ ] Recovery does not silently give a server access to E2EE keys or restore a revoked root's authority.
- [ ] Packaged-client acceptance passes on each declared supported desktop platform.

**Checkpoint references:** §§2–4, 11, 16, 20–21.

## v0.2: Server foundations and sane defaults

**Goal:** A new server has one legitimate owner, explicit admission, consistent shared state, and deny-by-default authority.

### Deliverables

- [ ] Fresh-server bootstrap creates a one-time owner claim secret in operator-controlled logs/output; it expires or is consumed atomically and is never exposed to ordinary joiners.
- [ ] Only the first **valid claim** establishes the owner; racing/invalid claims cannot take ownership.
- [ ] Server membership is distinct from account/device identity and has explicit pending, approved, rejected, left, suspended, and banned states.
- [ ] Unapproved users receive an accurate approval-pending state and cannot read or mutate protected resources.
- [ ] Owner approval/rejection, member removal, ban, rejoin, and leave are durable and auditable.
- [ ] Role/capability model supports Owner, Administrator, Moderator, and Member without relying on display names for enforcement.
- [ ] Role and channel permissions are enforced by the server for **every** mutation and read path.
- [ ] Default server is private, approval-gated, and gives ordinary members no channel/role/server administration rights.
- [ ] Server name, icon, description, membership roster, and basic settings are functional.
- [ ] Channels have stable shared identifiers; creation, listing, renaming, deletion, and synchronization work across accounts and restarts.
- [ ] Authorized users see the same channel objects; identical channel names never create false impressions of shared state.
- [ ] Policy changes propagate to active clients; authorization failures do not corrupt local state.
- [ ] Ownership transfer and server deletion have authenticated, deliberate flows.
- [ ] Admin operations produce an appropriate audit trail without leaking protected message contents.

### Proposed initial defaults

| Policy | Initial value |
| --- | --- |
| Admission | Owner approval required |
| Server visibility | Private |
| Initial owner | Single-use authenticated bootstrap claim |
| New member role | Member, no administrative capabilities |
| Create/delete channels and manage roles | Owner/Admin only |
| Invite/approve members | Owner/Admin initially |
| Post/read messages | Approved users with channel permission |
| Ban/kick | Owner/Admin initially; delegated moderation optional |
| Administrative logging | On, metadata-minimized |
| Protected content | Never exposed to unauthorized members |

These are **defaults**, not permanent protocol restrictions. All permission changes must be validated server-side.

### Release acceptance

- [ ] Owner, approved member, unapproved applicant, and banned account tests pass using separate clients.
- [ ] A new non-owner cannot create channels or obtain administrative authority by joining.
- [ ] Both authorized accounts observe one shared `test` channel, not separate same-named channels.
- [ ] Permission changes and bans take effect in existing sessions and after restart.
- [ ] Unauthorized API calls, including direct calls bypassing UI, are rejected without protected-state leaks.

**Checkpoint references:** §§1, 4–6, 15, 20, 23.

## v0.3: Reliable encrypted text messaging

**Goal:** Small groups can use Cords as an everyday text messenger without losing, duplicating, or misattributing messages.

### Deliverables

- [ ] Create and participate in authorized encrypted text channels.
- [ ] Send, receive, edit, and delete messages with authenticated authorship and defined deletion semantics.
- [ ] Reply to messages with encrypted references and stable target identifiers.
- [ ] Message timestamps, edit indicators, sending/failed states, and retry affordances are visible.
- [ ] History pagination, catch-up, and message ordering remain correct after reconnect.
- [ ] Markdown, inline code, fenced code blocks, and spoiler text render safely.
- [ ] Offline send queue and permanent-vs-retryable error handling are predictable.
- [ ] Multiple authorized devices converge on consistent conversation and message state.
- [ ] Deleted/inaccessible channels and out-of-order lifecycle events synchronize safely.
- [ ] Conversation membership and encryption membership cannot silently diverge indefinitely.

### Release acceptance

- [ ] Two accounts exchange messages, replies, edits, and deletions through a real server.
- [ ] Repeated uploads, lost connections, restarts, and out-of-order events cause no duplicate visible messages or invalid state.
- [ ] Unauthorized edits/deletions and revoked-device sends are rejected.
- [ ] The server cannot read encrypted message bodies or protected lifecycle semantics.

**Checkpoint references:** §§6–10, 14, 18, 20–21, 23.

## v0.4: Direct messaging and private relationships

**Goal:** People can communicate privately without needing a public text channel.

### Deliverables

- [ ] One-to-one DMs address stable account identities.
- [ ] Mutual-server courier routing transports E2EE DM packets without server plaintext access.
- [ ] DM requests, acceptance/decline, and contact initiation policy are explicit.
- [ ] Block/unblock behavior covers new messages, existing conversations, device additions, and presence metadata.
- [ ] All authorized participant devices are handled correctly in DM cryptographic membership.
- [ ] DM conversations survive reconnects, server restarts, and device churn.
- [ ] Group DMs are implemented **if included in the v1 scope**, with a separate membership and authorization acceptance suite.
- [ ] DM routing failure and loss of a mutual courier produce understandable states.

### Release acceptance

- [ ] Independent accounts exchange encrypted DMs through a permitted mutual server.
- [ ] Courier/server operators cannot read DM plaintext or impersonate a participant.
- [ ] Blocking, revocation, and device changes behave correctly in automated and real-client tests.

**Checkpoint references:** §§7, 11–12, 14–16, 20.

## v0.5: Encrypted attachments and rich messaging

**Goal:** Sharing ordinary content is practical without abandoning Cords' privacy model.

### Deliverables

- [ ] Attach files, images, audio, and video with bounded size and quota enforcement.
- [ ] Attachment content is encrypted client-side using fresh attachment-specific key material.
- [ ] Keys and protected metadata are delivered only through the intended E2EE conversation.
- [ ] Chunked uploads/downloads, interruption recovery, integrity verification, and garbage collection work.
- [ ] Safe file opening, previews, drag-and-drop, and clipboard image paste work on supported clients.
- [ ] Emoji reactions are authenticated, encrypted as applicable, and synchronized.
- [ ] Message pins, mentions, and relevant notification events work with access control.
- [ ] Link previews are opt-in or privacy-preserving; no silent remote fetching that exposes user activity.

### Release acceptance

- [ ] Recipients verify attachment integrity and reject corrupt, truncated, or tampered data.
- [ ] Storage quotas, orphan cleanup, and failed uploads cannot trivially exhaust the server.
- [ ] Unauthorized users cannot retrieve attachment plaintext or decryption keys.

**Checkpoint references:** §§10, 13, 15–16, 20.

## v0.6: Everyday usability and organization

**Goal:** Cords remains manageable across multiple active servers and conversations.

### Deliverables

- [ ] Desktop notifications, mention counts, mute settings, and DND behavior are configurable.
- [ ] Unread/read state and first-unread navigation work across sessions and authorized devices.
- [ ] Search and jump-to-message work over permitted history without violating the E2EE boundary.
- [ ] Channel categories, ordering, and conversation navigation are usable.
- [ ] Typing indicators and optional presence have documented privacy semantics and expiration.
- [ ] Drafts persist appropriately without cross-account leakage.
- [ ] Keyboard navigation, accessible control labels, and screen-reader semantics cover core messaging.
- [ ] Trust, delivery, offline, and failure states are explained in human-readable language.
- [ ] Theme, text scale, and basic application preferences are available.

### Release acceptance

- [ ] A user can navigate several servers, locate old messages, and silence noisy channels.
- [ ] Search never causes the server to gain plaintext it was not already authorized to know.
- [ ] Core workflows are operable without a mouse and expose accessible security warnings.

**Checkpoint references:** §§9, 14, 16, 21–22.

## v0.7: Authenticated voice communication

**Goal:** Real-time voice works reliably and respects the existing identity and server policy model.

### Deliverables

- [ ] Persistent server voice channels and one-to-one voice calls.
- [ ] Authenticated signaling and participant/device identity binding.
- [ ] Documented NAT traversal, relay/TURN, and voice transport security model.
- [ ] Mute, deafen, push-to-talk, voice activity, device selection, and volume controls.
- [ ] Speaking indicators, participant limits, and server-side permission enforcement.
- [ ] Network reconnection and call failure states are usable.
- [ ] Relay/group-call trust assumptions and any limits to end-to-end protection are clearly disclosed.

### Release acceptance

- [ ] Multi-party voice works under tested network conditions and across supported clients.
- [ ] Unauthorized participants cannot join protected calls.
- [ ] Calls cannot silently downgrade their documented security guarantees.

**Checkpoint references:** §§14, 20–22.

## v0.8: Video, screen sharing, and mature administration

**Goal:** Complete planned real-time collaboration and make routine server administration practical.

### Deliverables

- [ ] One-to-one and supported group video calling.
- [ ] Full-screen and selected-window sharing with explicit user consent.
- [ ] Stream quality controls, participant layouts, and clear capture-state indicators.
- [ ] Call signaling and relay/SFU trust model are documented and tested.
- [ ] Moderation controls: member management, timeouts/suspension where supported, and audit history.
- [ ] Message moderation/reporting has explicit disclosure semantics for user-submitted evidence.
- [ ] Rate limits cover joins, messages, uploads, invites, and other abuse-sensitive operations.
- [ ] Server operators can configure policy without holding user root keys or message plaintext keys.
- [ ] Server backup, restore, and recovery procedures are usable and tested.

### Release acceptance

- [ ] Video and sharing work across supported clients without unauthorized capture or disclosure.
- [ ] Admin/moderator actions are permission-checked and auditable.
- [ ] Restored server state retains correct ownership, memberships, and routing identity.

**Checkpoint references:** §§15, 17, 20, 22–23.

## v0.9: Feature freeze and release hardening

**Goal:** No planned feature additions; close security, reliability, deployment, and accessibility gaps before v1.0.

### Deliverables

- [ ] Freeze v1 scope and document intentionally omitted features.
- [ ] Run comprehensive authorization, cryptographic membership, sync, and recovery adversarial tests.
- [ ] Verify account-root burning, lost-device handling, and key rotation/recovery procedures.
- [ ] Test fresh installation, upgrade, downgrade refusal, and rollback/recovery across supported versions.
- [ ] Validate backup/restore, storage exhaustion, dependency outage, clock skew, and database corruption behavior.
- [ ] Automate CI builds, tests, lint, formatting, dependency checks, and secret scanning for supported platforms.
- [ ] Sign or otherwise verifiably authenticate release artifacts and document provenance.
- [ ] Test client/server mixed-version compatibility and forward-compatibility rules.
- [ ] Complete operator guides, security/privacy documentation, user help, and accessible UX review.
- [ ] Close critical and high-severity security, data-loss, and privilege-escalation bugs.
- [ ] Publish a release candidate and collect reproducible acceptance evidence.

### Release acceptance

- [ ] The applicable production-readiness checklist (§23) passes against release artifacts and supported deployment topologies.
- [ ] Independent clean-environment installation and verification succeed.
- [ ] No unresolved release-blocking regressions remain.

**Checkpoint references:** §§0–24, especially 17–24.

## v1.0: Stable release

**Goal:** Publish a production-quality, self-hosted messenger with clear compatibility and support commitments.

- [ ] All v1-scoped beta milestone acceptance criteria have passed or have documented approved scope changes.
- [ ] All applicable `CHECKPOINT.md` production-readiness criteria have evidence-backed completion.
- [ ] Supported server/client platforms and minimum versions are documented.
- [ ] Versioned protocol compatibility and migration guarantees are published.
- [ ] Release artifacts, signatures/provenance, install instructions, and upgrade guides are available.
- [ ] Security reporting, maintenance, and end-of-life policies are published.
- [ ] Stable v1.0 is tagged and released from the verified source revision.

**Checkpoint references:** §§18–19, 23–24.

---

## Common release gates

Apply these to **every** beta milestone, not just v0.9:

- [ ] New functionality works end to end on server and applicable clients.
- [ ] Security and permission boundaries are enforced and tested against bypass attempts.
- [ ] Required state survives restarts, reconnects, and relevant failure scenarios.
- [ ] Automated positive, negative, and tamper/replay tests cover the change.
- [ ] Relevant migrations and backward/forward compatibility behavior are tested.
- [ ] User-facing errors and recovery states are understandable.
- [ ] Documentation, threat-model notes, and operational guidance are updated.
- [ ] Release artifacts and their source revisions are identifiable and verified.

> These are recurring **per-release gates**. Checking them for one release does not mark them complete for all future releases. Keep per-version evidence in the release notes or acceptance report.

## Decisions and scope

Track decisions here before treating optional features as mandatory:

- [ ] Confirm whether **group DMs** are required for v1.0 or deferred.
- [ ] Confirm whether **voice, video, and screen sharing** are v1.0 blockers or may ship after a stable text-first v1.0.
- [ ] Confirm whether **root-key burning** must be fully implemented in v0.1 or is a separately gated pre-v1 security requirement.
- [ ] Specify **account recovery** guarantees, including what cannot be recovered after lost keys.
- [ ] Specify **DM courier discovery and failure handling** without assuming federation.
- [ ] Define which platforms are officially supported for each release.
- [ ] Explicitly decide whether federation, bots, integrations, and mobile clients are outside the initial v1 scope.

## Progress summary

**Do not manually infer percentages from checked technical subcomponents.** A release is complete only when its own deliverables, acceptance criteria, and common release gates pass.

| Release | Status | Evidence / release notes |
| --- | --- | --- |
| v0.1 | Not verified | TBD |
| v0.2 | Not verified | TBD |
| v0.3 | Planned | TBD |
| v0.4 | Planned | TBD |
| v0.5 | Planned | TBD |
| v0.6 | Planned | TBD |
| v0.7 | Planned | TBD |
| v0.8 | Planned | TBD |
| v0.9 | Planned | TBD |
| v1.0 | Planned | TBD |

**Maintenance rule:** A checked feature can regress. Reopen the relevant box when evidence shows it no longer satisfies its acceptance criteria. The technical checklist and this roadmap should be updated together.
