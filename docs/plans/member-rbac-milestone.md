# Members, RBAC, moderation and notifications

## Consolidated desktop rework amendment (2026-10-15)

The approved UI rework includes all outstanding milestones below, not just their
frontend. Execute: audit/shared UI foundation; complete authorization/MLS gates;
shared media foundation; canonical account profiles/avatar publication;
directory/roles/moderation with immediate UI integration; server presentation;
account-wide synchronization/local notifications; shell/visual refinement.
Independent UI units may proceed while backend gates are incomplete. Gate backend,
UI integration and packaged/visual evidence independently. Preserve existing work.

`UI-DESIGN.md` governs UI behavior. ADR 0010 governs canonical account profiles
and shared media; the existing avatar plan retains codec/fidelity acceptance.
Update these existing contracts rather than introducing competing implementations.
Use authenticated revision-aware server presentation storage, independently of
encrypted attachments. Synchronize every active joined server for the unlocked
account, with independent connections/cursors/errors, four concurrent fetches,
sixteen WebSockets plus fair overflow polling, and 1-60 second jittered backoff.
Lock/switch/shutdown cancels workers and submissions; never unlock accounts in
background. Notifications use authenticated content, not WebSocket metadata.

Track actual work and evidence in [the UI checkpoint report](../reports/ui-rework-20261015.md)
and the design ledger. Existing backend acceptance evidence remains authoritative
only for its tested scope. No deployment or release publication is authorized.

## Existing security milestones (requirements retained)

Approved implementation contract, 2026-10-15. Preserve Rust client-core/Tauri/Svelte,
account/device/MLS separation, current working-tree changes and existing role rules.
Each checkpoint follows plan -> implement -> validate -> record evidence. Failed
security, continuity or migration tests block dependent checkpoints. No production
deployment, reset, attachment delivery, DM implementation, presence or profile bio.

1. **Authorization and development migration.** Preserve definitions/ownership;
   archive preparatory assignments without granting them. Reconstruct restricted
   encrypted-account boundaries from active, delivery-enabled admitted devices with
   valid root/device and server membership signatures. Preserve exclusions,
   revocations, burns, pending removals, MLS state and joined-sequence boundaries.
   Candidates are staged until the integrated service is ready; old binaries refuse
   the advanced schema. New installations ultimately enforce RBAC immediately.
2. **Authoritative RBAC.** Current durable evaluator checks at every service, HTTP,
   WebSocket and sync boundary. Transactional reauthorization, hierarchy, owner
   protection, explicit effective actions, immutable audit/idempotency receipts.
3. **MLS transitions/recovery.** Desired access differs from actual leaves. Revoke
   delivery immediately and fence protected sends only in affected channels until
   authenticated removal commits. No committer means visibly blocked, with current
   eligible-device catch-up or explicit authenticated succession to a fresh group.
   Preserve owner/create plus scoped management authority. Bind predecessor and
   successor identities under initiator signature and server acceptance; reject
   substitution. Never present replacement as original-group continuity. Explain
   that per-device partial archives can differ. No old secrets, revoked committers,
   automatic history transfer or cryptographic rollback.
4. **Moderation/deletion.** Kick, ban/unban, account-wide server-time timeout and
   removal; own-account versus administrative deletion. Durable authenticated
   event-bound tombstones, redacted history/replay and deletion-before-presentation.
   Never suppress MLS controls. Test pinned library omission boundaries, consecutive
   drops, multiple senders, offline/restart and epochs. Unbridgeable gaps require
   validated fresh rejoin or checkpoint-3 recovery. Future attachments bind stable
   objects to live parent events; no dummy endpoints or claimed cleanup.
5. **Account directory/state.** Paginated admitted accounts, separately authorized
   diagnostics/requests, consistent verified presentation and role chips. Atomic
   role reordering and shared authoritative refresh. Owner-only archive review and
   explicit assignment restoration, with revision/idempotency checks.
6. **UI.** Grouped server account sidebar, header toggle and preserved Info/details;
   reusable profiles/searchable role chips; account-centric Members, Devices and
   Requests; two-pane Roles and actual permission overrides/moderation. Explicit
   pending removals, unavailable committer, unrecoverable original and succession
   states. Keyboard/responsive/large-list checks; no fabricated capabilities.
7. **Local notifications.** Authenticated structured mentions and native Tauri
   presentation. All/Mentions/Nothing, hidden previews by default, local DND/mutes,
   per-server/channel overrides, own-account/focused-route suppression. Capture
   authoritative first-sync high-water marks before history; serialize fetches and
   persist dedup/decisions with message/tombstone/cursor state independently of read
   state. Test arrivals during sync, WS races, reconnects, deletion and repeats.
   Recheck access/session/deletion before submission. Persist before OS delivery:
   interrupted submission may lose a banner rather than duplicate it on restart.

Run focused tests at every checkpoint, real PostgreSQL migration and restart tests,
independent TLS/MLS clients, frontend checks/lint/tests/build and affected Rust/CI
checks. Packaged notification delivery/navigation/privacy must be tested separately
on Windows/Linux/macOS where available; use platform CI where practical. Source,
cross-compilation and mocks are not runtime proof. Missing evidence stays incomplete.
Record paths, commands, outcomes and remaining issues in the checkpoint report.
