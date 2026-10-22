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

### 2026-10-15 directory continuation

Implemented the bounded, authenticated `account-directory-v1` read independently
of device contacts and pending requests. It groups active, unrevoked devices under
admitted account IDs, excludes burned identities, and rechecks current ViewServer
policy/session state in a transaction. Pages contain up to 16 accounts and 64
devices per account, with a 4 MiB stored-contact budget and explicit errors.
PostgreSQL byte collation makes opaque account cursors agree with client ordering.
The native client verifies device evidence and page identity/order before exposing
it. The default right rail uses these pages, persists visibility and offers search,
device expansion, refresh and pagination. Role grouping, canonical profile state,
moderation and the account-centric administration replacement are still open.

The complete server regression passed 53 tests, excluding only the manual desktop
fixture, against disposable PostgreSQL 17.10 (fixture `cords-rbac-3e91f450e769`,
cleaned). Core/server Clippy passed with warnings denied. Browser reviews are
supplemental; packaged native acceptance remains separate. Evidence is in the
existing `docs/reports/ui-rework-20261015.md` ledger.

### 2026-10-16 shared account profiles

Canonical signed profiles and separate hash-based JXL storage are integrated with
the native vault, account directory presentation, message headers and My Account.
The account-profile automatic PostgreSQL migration occupies schema 15. The staged
RBAC activation candidate now advances 15 to 16; it remains outside automatic
migrations. The former duplicate migration version was diagnosed and corrected,
and migration/restart regressions passed. Do not activate the candidate from this
profile work. Existing historical schema-14/15 reports describe earlier binaries.

Independent TLS clients backed by disposable PostgreSQL on `.54` passed image
publication, authenticated retrieval from a separate account, second-server
replication, sealed restart, offline edits/retry, removal and revoked-device denial.
Causal protocol tests cover independent-field merges and explicit conflict review.
These do not complete every multi-device, legacy-candidate migration, background
multi-server or packaged-platform acceptance gate. See the maintained UI ledger
for commands, logs and current native review status.


### 2026-10-21 native role management and enforcement continuation

The user requested the comprehensive native role workspace and explicitly expanded
this task to completing the enforcement and MLS activation gates. This does not
authorize production deployment, replacing identities, resetting vaults, or
publishing a release. Preserve ADR 0008, ADR 0011 and UI-DESIGN.md. No VISION.md
exists in this checkout. Existing admission-PFP changes remain in place.

Implementation plan, executed in independently validated units:

1. Integrate the role editor in the existing settings workspace, preserving the
   role list/navigation, scoped dirty drafts and idempotent revision-aware saves.
   Separate Display and searchable Permissions; use role-color presets/preview.
   Fix profile-chip ordering and invalidation on same-size model updates. Native
   offscreen Qt tests on .54 validate switching, search, stale/error preservation,
   scope changes, model updates and wide/narrow renders. These presentation tests
   do not establish server authority.
2. Audit every authentication, mutation, HTTP, WebSocket and synchronization
   boundary. Replace active-policy reliance on legacy membership capabilities
   with current transactional decisions. Add versioned, independently signed
   effective-action/policy-removal objects and explicit feature negotiation;
   unsupported clients fail with an upgrade-required error. Preserve the inactive
   legacy boundary while building and validating the candidate.
3. Make role changes atomically reconcile lost encrypted access with immediate
   delivery denial and durable signed removal evidence. Fence only affected
   encrypted application sends, keeping authorized history/control catch-up and
   removal commits available. Recheck add targets and commits against current
   policy. Preserve device exclusions, admission and joined-sequence bounds.
   Implement authenticated no-committer recovery through existing channel
   succession; never remove general safety guards or reuse old group keys.
4. Validate the activation migration and native boundary preparation on fresh and
   upgraded disposable PostgreSQL schemas. Preserve definitions/owner; archive
   preparatory assignments; refuse incomplete/unsupported state. Wire supported
   startup/new admissions only after the previous service/client tests pass. Run
   independent TLS/MLS clients through deny/removal/catch-up/restart/recovery and
   unchanged-channel continuity before moving the candidate into automatic startup.
5. Add authoritative role/member snapshots and counts, atomic hierarchy reorder,
   and member changes using the existing evaluator/storage/audit/idempotency.
   Expose target-specific editable/assignable/removable actions through the typed
   bridge. Add the Manage Members tab, searchable multi-select assignment, drag
   and keyboard reorder, and protected inline profile-chip controls. Normal
   members receive a read-only view. Keep unsupported decorative options absent.
6. Validate server authorization, stale sessions, self/owner/equal/higher-role
   denial, persistence/restart and transport errors; then run native GUI flows on
   .54, inspect wide/narrow/long-name/many-role renders and build Windows. Record
   exact evidence separately from packaging and production acceptance.

Runtime constraints: native Qt 6.10.3/CXX/Rust; PostgreSQL integration and GUI tests
run on 192.168.86.54 in disposable resources, keeping existing services untouched.
Windows compilation uses E: build caches. TLS, server pins, account/device key
separation and independent MLS roster verification remain mandatory. No new UI
framework, duplicated role repository or client-only authorization is introduced.

Recovery unit detail: retain the current owner/create and scoped management gates
on succession. Keep ordinary replacement blocked by incomplete roster operations.
Provide bounded authenticated recovery metadata to the current owner without
exposing inaccessible rosters or message history. A distinct versioned device-signed
recovery object binds the complete existing signed succession, current policy
revision, channel epoch and pending-removal digest. Recheck all these values and
the absence of a currently eligible committer transactionally before adopting a
fresh successor. Preserve the original channel, audit the explicit recovery and
mark obsolete operations canceled rather than treating them as successful commits.
Show the blocked state and explicit fresh-group/partial-archive consequences in
native administration. Test stale/substituted proofs, unauthorized initiators,
available committer, pending operations, fresh-group cryptography and restart.

Restart validation unit: reuse the real service router and TLS 1.3 test transport
in a separate child process, with the same private server identity, isolated
PostgreSQL schema and trusted test CA. Persist child configuration only in the
owned private fixture directory; expose no production credentials. Stop and reap
the owned child, start a second process against the same state, reopen independent
client vaults and verify current policy, fresh MLS delivery and immutable recovery
audit continuity. This proves process persistence of the candidate service, not
packaged-server startup or final automatic-migration readiness.

Atomic reorder unit: add a bounded full-catalog list of role IDs and proposed
priorities to the existing revision/idempotency mutation. Reject duplicate or
missing IDs, validate both previous and proposed authority for each changed role,
then validate the complete proposed catalog before writing any definition. Commit
all positions with one revision and audit entry. Preserve default priority zero,
admin/moderator ordering and current self/equal/higher protections. Validate pure
policy substitution/escalation cases and real PostgreSQL rollback/retry behavior
before wiring drag and keyboard controls. This is a discrete metadata operation;
it does not bypass the still-pending automatic enforcement activation gate.

Role metadata unit: expose separately signed workspace and per-target action
snapshots using the existing policy evaluator. Bind actor account/device, server,
catalog revision and target identity. Return authoritative account member counts,
definition edit/delete/create hints and target assign/remove sets; these are
presentation hints and every mutation rechecks current policy. Normal admitted
members receive read-only metadata. Preserve inactive owner-only catalog behavior
and return no assignment hints before enforcement. Reuse account profiles/avatar
loading rather than copying pictures into role snapshots. Verify signature/scope,
hierarchy, owner/self protection, stale state and propagation before native use.


Native reorder unit: send the complete typed ID/priority proposal through the
existing revision/idempotency command. For drag or keyboard movement, renumber
only the contiguous editable segment between protected role anchors, preserve
all protected priorities and keep default at zero. Refuse an exhausted integer
range explicitly. Require a review confirmation, retain the request ID on an
uncertain retry, and let the server validate the complete final hierarchy.
Validate duplicate priorities, protected anchors, built-in order, keyboard and
scope/stale confirmation behavior before accepting the unit.


Automatic startup unit: promote the validated enforcement SQL to normal schema
22, retaining native verification as a separate required step before the listener
opens. Keep legacy-upgrade regression fixtures explicitly at schema 21; those
fixtures must fail the production current-schema check. Migration tests use the
normal migration directory and prove native completion, idempotent activation,
identity mismatch and malformed-policy refusal. Run the full fixed-snapshot
service suite before exercising the actual server executable on an isolated
schema/private identity, restart it, and prove invalid state never opens a port.
The binary startup check uses the intended loopback HTTP backend behind the TLS
edge; independent existing TLS/MLS client checks remain separate evidence.


Manage Members unit: add bounded signed assigned/unassigned account pages for one
role, bound to actor/device/server/catalog revision and cursor. Recheck current
account eligibility and hierarchy; default membership is implicit. Return only
account IDs and selected-role action hints, reusing canonical signed profile and
avatar loading. Search explicitly covers loaded pages, with continuation controls
and an authoritative total rather than pretending that a partial page is complete.
Use the existing assignment endpoint for up to 64 selected accounts, preserving
a per-account idempotency receipt and stopping on the first failure. Surface exact
partial completion and refresh authoritative counts/rows after any committed work;
do not claim the multiple existing operations are one atomic transaction. Protect
scope/revision changes, unknown target hints, self/owner/equal/higher roles, and
exhausted/empty pagination in service/client/native tests before acceptance.

Unsupported permission presentation unit: inventory implemented server operations
before exposing permission edits. Preserve existing stored grants but disable
reserved invite/moderation/emoji/structured-mention controls and role mentionability
with an explicit unavailable explanation. Keep implemented permissions searchable
and editable under existing policy. Verify these controls cannot emit edits in
native presentation tests; this does not implement the separate moderation milestone.

Live native role unit: provide an explicitly selected disposable TLS/PostgreSQL
fixture with two independent sealed account registries and active policy before
admission. Run the actual Qt controller/QML Manage Members and profile controls
against it, then verify durable assignment/audit outcomes server-side. No runtime
unlock shortcut or production credential is added. Keep this selected fixture
out of the normal service suite and distinguish offscreen GUI from packaged OS
acceptance. Stop it through its owned stop file after collecting evidence.

Final role presentation unit: align permission controls to the right of their
labels while retaining the existing grant-set semantics and accessibility names.
Show the live role chip with sample message content on conversation and elevated
surfaces. Reuse existing controls/colors; validate search/control state and inspect
wide/narrow renders before rebuilding the desktop artifacts.
