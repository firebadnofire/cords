# Native role management — 2026-10-21

## Plan

See the continuation in [the existing RBAC milestone](../plans/member-rbac-milestone.md).
The user explicitly expanded the role UI task to completing enforcement and MLS
gates. The comprehensive task remains in progress. The dated units below are a
chronological ledger; the latest continuation records schema 22 startup activation
and supersedes earlier statements that activation was still unwired.

## Implementation

Unit 1 replaces the modal role editor with an inline settings editor beside the
searchable hierarchy. Existing create/update/delete calls retain server revision,
idempotency, protected built-ins, validation and error behavior. Display and
Permissions are separate tabs; search covers capability group, label, description
and stable ID. Display adds color presets and a chip preview. Dirty switching,
leaving and account/server changes are guarded. Escape returns to the overview.
Profile chips sort by hierarchy and react to role/member model changes even when
row counts stay equal. Long chip labels elide within their available width.

Existing signed role APIs/evaluator/transactional storage are reused. Unit 1 adds
no backend endpoints, activates no permissions and changes no cryptographic rules.
Manage Members, counts, atomic drag/keyboard reorder, hoisting and inline moderator
assignment are not implemented by this unit. No Links tab or decorative controls
were introduced. Existing owner-only catalog warnings remain truthful.

## Validation

Native Qt 6.10.3 ran on .54 with offscreen/software rendering. Five application test
functions passed (seven Qt harness records including setup/cleanup): inline role
switch/discard/search/navigation/Escape/layout; same-count profile role updates;
existing draft/conflict/virtualization, account/server scoping and administration
context regression. Source and tests were copied into the retained admission-PFP
native build workspace; the changed C++ tests were recompiled. Presentation data
are controlled test inputs, not proof of moderator authorization or network sync.

Evidence: `evidence/role-editor-20261021/editor.txt`, `editor.xml`,
`roles-inline-wide.png`, `roles-inline-narrow.png`. One final run reported an
intermittent QTemporaryDir cleanup warning for an empty temporary directory; a
subsequent directory inspection found no retained files. No service/database
resources were created by this presentation selection.

Windows native compilation and QML lint passed during unit development. Compiler
output contains Qt-header C4702 warnings; lint contains existing unused-import
information. Final full-source builds/checks are repeated after subsequent units.
No Windows GUI, packaged-artifact, production deployment or complete RBAC result
is established by this unit.

## Enforcement continuation

The candidate remains outside automatic startup. Current source supports explicit
`enforced-roles-v1` negotiation, persists session negotiation independently of V1
membership signatures, rejects old sessions with an upgrade-required error, and
uses transactional current permissions for messaging and roster mutations. Signed
effective-action hints bind server/account/device/channel scope. Active catalogs
are readable by ordinary admitted accounts; mutation authority remains independent.
The client seals newest observed policy revisions and rejects known rollback or
enforcement downgrade, including after restart.

Role changes atomically queue signed channel-access removal evidence and deny
delivery. Affected encrypted sends are fenced while history/control remains
available to eligible participants. MLS adds recheck target admission and channel
eligibility at reservation and commit. Completed removals retain device exclusions;
only fresh authorized MLS enrollment clears them. Private application framing must
match the channel and current epoch, preventing queued stale ciphertext from
entering a later epoch. A definite send refusal clears its pending operation and
is surfaced to the caller, leaving control work possible.

A newer authenticated enrollment sequence permits a returning client to validate
a fresh Welcome in isolated replacement MLS state. It cannot reset to an old
Welcome, inherit skipped history, or weaken roster verification. Earlier cached
history remains local. Delegated channel creation initializes current policy and
the restricted creator-account boundary atomically. New public/approved/owner
admission writes separate account/device admission records; account-authorized and
administrator-approved device modes remain distinct. Review endpoints use approval
capabilities, independently of broad server management.

Validation on disposable PostgreSQL 17 at `192.168.86.54`: twelve staged enforcement
tests passed, including an independent TLS/MLS two-vault scenario with role loss,
fencing, signed removal, unaffected-channel delivery, stale-epoch refusal, client
restart, rollback/downgrade rejection and fresh enrollment. This is client restart
proof; server-process restart and no-committer recovery remain separate outstanding
gates. Evidence: `evidence/role-editor-20261021/active-admission.txt`. Local targeted
crypto and fixed-signing-vector tests passed; server/client Rust checks passed.
Generated CXX compiler warnings remain visible in the remote log.

The owned fixture is `cords-e2e-role-editor-20261021`, with a loopback-only random
PostgreSQL port and private credentials. Existing services are untouched. The role
UI, reorder/member management, activation migration, final builds and acceptance
are still in progress. Do not interpret this ledger as activation approval.

### Recovery and native boundary continuation

The expanded independent two-vault TLS/MLS scenario now passes actual owner-leaf
removal, loss of the last eligible committer, authenticated fresh-group succession,
separate partial archives, client restart and reconstruction of the service/TLS
listener from durable state. This is not a separate server-process or package
restart test. Current epoch bindings verify the current committer's certified
device, independently of the immutable channel identity's original creator.

All thirteen staged enforcement tests pass in
`evidence/role-editor-20261021/recovery-removal-takeover.txt`. The additional
discretionary-removal service test proves that signed evidence survives the
requester, and another participant with CommitMls can supersede its reservation
without ManageChannelMembers. Evidence: `removal-proof-takeover.txt`. Only an
identical target/epoch removal may supersede a reservation; adds and different
targets remain protected. Canceled reservations retain audit state and no longer
occupy the one-pending-operation index. An initial duplicate-index failure was
diagnosed and corrected before the passing rerun.

Native controls use verified current effective actions for active policies.
Owner recovery candidates are fetched through the typed bridge; raw QML cannot
provide recovery proofs. The native screen distinguishes eligible committers from
fresh-group recovery and requires confirmation of separate identity, fresh keys,
no history transfer and partial local archives. Scope changes dismiss the dialog;
locking immediately clears candidate presentation. Qt 6.10.3 offscreen/software
tests on .54 passed recovery presentation and the two role-editor/chip regressions
(three functions, five harness records): `recovery-native.txt` and XML. These are
presentation tests, distinct from the real TLS/MLS scenario. Native onscreen and
Windows packaged acceptance remain unverified. Activation and the rest of the
role management request remain in progress.

Separate-process continuation: `recovery-process-restart.txt` now passes the same
independent TLS/MLS flow with two distinct child server processes against the
persisted schema and original private identity. The reopened owner verifies active
policy and sends in the fresh group after the second process starts; recovery
audit remains singular. This uses the actual router/TLS service inside a test
process harness. The packaged `cords-server` startup gate remains outstanding.
Eight native boundary migration tests passed (`migration-before-activation.txt`).
The new explicit startup activation function also passed fresh/repeated activation,
wrong-server rejection and malformed-policy refusal in its disposable schema. It
is not yet wired to automatic startup.

The broader pre-activation service run returned 68 passes and two failures. One
was burned-identity review visibility; preserving its prior staff-visible record
without its PFP or an admission grant fixed the targeted regression. The other
was the native test's 40-second wait for cold debug image processing, whose
existing pipeline deadline is 120 seconds. A targeted diagnostic identified
operation 8 (prepare server icon); only media waits were adjusted to the existing
deadline plus handoff overhead, with ordinary waits unchanged. Its rerun and the
complete corrected suite are still pending. These failures continue to block
automatic activation until validated.


The corrected stable-snapshot pre-activation run passed all 73 service tests
(`server-before-activation.txt`), with the three explicit manual fixture entry
points excluded. Cold media acceptance passed with only its media wait aligned
to the existing production deadline. The earlier source-mutated run is invalid
and is not counted as evidence.

Role metadata now has separate signed actor/device/server/catalog-bound workspace
and target snapshots. Counts aggregate active admitted accounts in PostgreSQL;
no unbounded account list or avatar payload is materialized. Target assign/remove
hints reuse current policy protections and confer no mutation authority. Client
validation checks signature, scope, catalog membership, action consistency and
monotonic revision. Current ordinary members receive read-only metadata; inactive
catalogs retain owner-only visibility and expose no assignment hints. The targeted
PostgreSQL hierarchy/count/delegation test and the verified TLS client acceptance
passed (`role-workspace.txt`, `role-workspace-tls.txt`). The native bridge now exposes
per-role counts/edit/delete metadata and create eligibility. Remaining role-member
and reorder controls, automatic activation and packaged startup remain in progress.


Native metadata controls passed four focused functions (six harness records) on
.54 (`role-metadata-native.txt`). The new regression initially lacked a visible
parent window; diagnostics confirmed correct metadata but intentionally disabled
editing while invisible. It passed with the established visible-parent fixture.

Reorder controls now use one typed complete-catalog ID/priority proposal with
revision/idempotency protection. Only the contiguous editable segment is
renumbered; protected anchors retain their priorities and default remains zero.
Duplicate priorities receive strict order where an integer range exists; exhausted
ranges and invalid admin/moderator order are explicit errors. Native offscreen
mouse drag, Alt+arrow keyboard, retry-ID and scope cancellation passed
(`role-reorder-native.txt`). TLS client/server persistence, idempotent retry and
one immutable audit entry also passed (`role-reorder-tls.txt`). These presentation
checks do not replace the real PostgreSQL atomic reorder test recorded earlier.

## Integrated continuation — current implementation

The native editor now has Display, Permissions and Manage Members tabs alongside
the persistent settings navigation and compact role list. Display provides name,
preset/custom hex color and message/chip previews on conversation and elevated
surfaces. Permission search remains outside the scrolling content; grant controls
are right aligned. Role grants remain a set, independently of resource override
allow/deny/inherit semantics. No Links tab, fake Administrator bypass, premium
styles, role icons or new hoisting model were added.

Manage Members uses signed pages of at most 16 account IDs with current target
actions, actor/device/server/catalog binding and byte-ordered opaque cursors.
Assigned counts aggregate active admitted accounts, not devices. The default role
is implicit and cannot be assigned or removed. An admitted account with expired
device evidence receives no new grant; its existing stored assignments remain
visible for authorized removal. Existing device/contact budgets fail explicitly
before unbounded deserialization. Canonical signed profile/avatar loading is
reused; page objects contain no copied profile pictures or identity grants.

The selector excludes already assigned accounts and permits up to 64 selections
across pages at one reviewed revision. Each account uses the existing independently
reauthorized assignment transaction and durable idempotency receipt. Operations
stop on the first failure, report the number confirmed, preserve the stopped
account's exact request ID/revision for uncertain retry, and refresh current state.
These are separate commits, not an atomic batch. Search explicitly covers the
current bounded page; Next page and Refresh navigate authoritative server data.
Global server-side nickname search is not provided.

Profile chips follow priority/ID hierarchy and react to model updates. The inline
searchable `+` selector and per-chip removal use exact target/revision assignable
and removable hints; every write rechecks server policy. Personal controls stay
read-only, including for the owner, whose authority remains independent of roles.
Ordinary members see read-only profiles. Worker refresh observes external role
metadata within 30 seconds while unlocked, clears stale action hints, and refreshes
a visible membership page. Lock/account/server changes clear scoped presentation.

Reserved invite, moderation, emoji and structured-role-mention permissions are
explicitly unavailable/read-only; saved values are preserved. Mentionability is
also read-only because no integrated structured mention operation exists. This
change does not claim implementation of those separate product milestones.

The enforcement candidate is now normal PostgreSQL migration
`202610210005_enforced_rbac.sql`, schema 22. `serve` performs native verification
and activation before opening a listener. Repeated startup preserves identity,
assignments and audit records. The one-way upgrade archives preparatory assignments
without granting them. Account admission, device approval and MLS membership stay
separate; policy loss denies delivery and fences affected encrypted application
sends until verified removal. No-committer recovery uses authenticated succession
to a fresh channel/group, with no historical key transfer. Operator guidance and
ADR 0008 now describe this startup contract.

## Integrated validation and limits

- Real PostgreSQL on `.54`: the complete service regression passed 76 tests with
  four manually selected fixture entry points excluded. This includes native
  fresh admission, current-policy enforcement, independent TLS/MLS removal,
  committer takeover, fresh-group recovery, separate-process restart, atomic
  reorder, PFP admission, metadata hierarchy/counts and role-page pagination.
  `server-role-final.txt` records the final run. The focused role-page test also
  covers two pages totaling 19 unique accounts, implicit default membership and
  the contact budget. Signature tests change every member-page scope field.
- Two real independent native workers over TLS passed live metadata refresh,
  assignment/removal, ordinary-member denial, paged members and lock clearing:
  `native-role-members-network.txt`.
- Actual Qt controller/QML against the explicitly selected active TLS fixture
  passed two-account Manage Members assignment, an ordinary member viewing
  another member's read-only chips, removal through Manage Members and a profile
  without leaving the card, count refresh and locking. The fixture independently
  verified zero remaining assignments and two assignment/two removal audit entries:
  `native-role-live-ui.txt` and `native-role-live-service.txt`. These fixture
  identities and private keys remain outside the repository.
- Qt 6.10.3 on `.54`, offscreen/software: the complete native suite passed 21
  harness tests, with the two explicit live fixtures skipped in that invocation.
  The selected live role flow above ran separately. After final preview/control
  alignment, three focused functions passed again (`role-final-presentation.txt`).
  Wide/narrow editor, long member names, chip updates, keyboard/mouse reorder,
  stale scope, unsupported controls and failed-receipt behavior were exercised.
  Rendered screenshots were inspected, including `roles-inline-narrow.png`,
  `role-members-narrow.png` and `role-members-live-assigned.png`. A final 32-role
  profile at 380 by 520 pixels passed ordering, bounded popup and scroll-content
  assertions and was inspected (`profile-many-native.txt`,
  `profile-many-roles-narrow.png`). The expanded selected live flow and three
  focused regressions passed six harness records. One Qt temporary-directory
  cleanup warning was followed by explicit removal after its workers exited.
- Protocol/crypto: 21 protocol unit tests, one identity-vector integration test
  and two crypto tests passed (`protocol-crypto-final.txt`). Client/server/typed
  bridge `cargo check --locked` passed. Library Clippy completed with warnings;
  strict warnings-as-errors is not clean because of codec casts and
  server lint findings. Newly added missing debug fields were corrected.
- The actual current `cords-server` executable passed normal migration/native
  activation, restart continuity and invalid-policy refusal before listen:
  `startup-binary.json`. The check ran twice initially and again after rebuilding
  the integrated binary. It uses the intended loopback HTTP backend behind the
  TLS edge; independent client tests supply separate TLS 1.3/pinning evidence.
- Windows MSVC/Qt production target built `E:/CordsBuild/build/Release/Cords.exe`.
  Adding a QML source first required an explicit CMake configure to refresh the
  generated MSBuild project; the subsequent build passed. The Linux Cords target
  also builds against Qt 6.10.3, using the existing debug Rust cache. Build logs
  retain generated CXX/Qt warnings. These are builds, not packaged-platform proof.

No production deployment, release publication, Windows GUI execution, onscreen
Linux session, macOS runtime or packaged cross-platform acceptance is claimed.
Member search is page scoped and multi-account assignment is explicitly partial
on failure. Unsupported role appearance/operation controls remain omitted or
clearly unavailable. The earlier staged-schema and pre-activation entries above
are historical evidence, superseded by this integrated continuation.

The explicitly labeled `cords-e2e-role-editor-20261021` PostgreSQL container was
stopped after acceptance; unrelated services were not touched. Owned private
fixture identities/logs and the stopped container remain outside the repository.
