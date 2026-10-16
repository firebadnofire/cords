# RBAC milestone checkpoint evidence

## Baseline and plan

Contract: [seven checkpoints](../plans/member-rbac-milestone.md). Existing dirty
avatar/account/role changes are preserved. No VISION.md exists. Current roles are
preparatory; production schema is 14. Windows Rust 1.97.1/Cargo 1.97.1; C: has
approximately 1.2 GB free, so compilation/temp output uses D:. Disposable PostgreSQL
acceptance uses verified SSH to the existing Linux test host; production services
are not modified. No release or packaged cross-platform claim is made.

## Checkpoint 1 — migration candidate validated

Implementation:

- Staged candidate `migrations/rbac/202610150001_enforced_rbac.sql` archives old
  assignments, preserves definitions, freezes encrypted device eligibility and
  separately preserves excluded devices. It advances only the isolated candidate
  schema; automatic migration paths and current runtime activation are unchanged.
- `authorization::migration::prepare` verifies bound server identity, root/device
  authorization, contact bindings and server membership signatures under table and
  migration locks. It commits the account boundary once without touching device
  flags, MLS state, ownership, history boundaries or pending removals.
- Server-account admission and per-device approval are persisted separately from
  channel account restrictions. Tests retain server approval for an otherwise
  valid channel-excluded device while preserving its channel exclusion; revoked
  devices acquire no approval. Categories have separate durable override storage.
- Native completion does not enable RBAC. The candidate stays outside automatically
  applied migrations until checkpoint 2/3 integration gates can safely ship it.
- Server startup now checks the understood schema after `serve --migrate` as well
  as without migration. A supplied migration directory cannot skip that check.
- ADR 0008 and the approved contract record the recovery, deletion and local
  notification semantics, including authenticated succession and per-device archives.

Validation:

- `cargo test --locked --release -p cords-server-core authorization:: --no-run`:
  compiled successfully, with build/temp files on D:.
- `scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -TestFilter authorization:: -ForwardPort 58205`:
  16 passed, zero failed, including all four new native migration tests on real
  PostgreSQL 17.10. SQL interruption rollback, fresh initialization, repeat startup,
  archived assignments, shared-account excluded/revoked leaves, expired/forged/
  burned/inactive evidence and malformed-data rollback are covered.
- Full server validation initially reported 42 passed/one failed: an existing TLS
  acceptance assertion still required avatar URL publication, which the preserved
  local-avatar changes intentionally removed. Updated only that assertion to check
  that neither removed URLs nor private PNG previews are published. No transport,
  encryption, admission, revocation, persistence or archive checks were removed.
- Full rerun using `scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -ForwardPort 58205`:
  **43 passed, zero failed**, 66.27 seconds; manual desktop fixture excluded as
  intended. This includes independent TLS/MLS clients and existing succession tests.
- `cargo clippy --locked --release -p cords-server-core --all-targets -- -D warnings`:
  passed after removing an unused async helper and splitting a long test assertion
  helper. `cargo fmt --all -- --check`: passed.
- Disposable PostgreSQL containers were removed by the fixture runner. The failed
  run was `cords-rbac-e722f25e95a6`; successful full rerun was
  `cords-rbac-64b1156a4e37`. No production database or services were changed.

The isolated migration gate is satisfied. The migration remains staged: automatic
startup still uses schema 14 and preparatory roles. Enforced initialization cannot
be claimed until checkpoints 2/3 are integrated. No UI/platform completion is claimed.

## Checkpoint 2 — durable authorization and role API subunits validated

Plan: derive verified actors and resource access first; connect and test role
administration next; integrate remaining request/delivery paths before activation.

Implementation:

- `authorization/runtime.rs` loads current signed membership/contact/root evidence,
  account admission, device approval, revocation/burn, ownership and assignments.
  It holds ownership/account advisory locks and the policy serialization lock in
  the existing order. Session authentication remains the caller's mandatory gate
  inside the same transaction. Decisions cannot be reused across deliveries.
- Channel decisions apply durable category/channel overrides and the migrated
  account restriction, independently checking excluded/removing devices and actual
  active delivery roster entries. Ownership can manage without acquiring MLS
  message/control participation. Server-only decisions expose no unchecked route
  permissions. Inactive or unfinished migration fails closed.
- `messaging/roles.rs` uses that snapshot for the staged enforced schema. Catalog
  and assignment views require definition or assignment authority; audit requires
  audit authority. Mutations retain distinct definition/assignment checks, current
  hierarchy, audit and idempotent receipts. The preparatory schema stays owner-only;
  its assignments cannot enable these paths in ordinary schema-14 operation.
- `messaging/tests/enforced_roles.rs` exercises real service handlers with valid
  sessions and the isolated staged schema. It does not establish packaged UI or
  upgraded protocol-client interoperability.
- Application acceptance and history retrieval now recheck the current staged
  policy inside the database transaction holding the current session. History
  reads use that transaction; accepted application writes retain legacy lifecycle
  guards. Shared channel/policy row locks protect the authorization boundary, with
  ownership -> account -> policy -> channel acquisition order. This is still a
  subset of the required endpoint integration.

Validation:

- Authorization subset after separate admission storage: 16 passed in 25.69s,
  disposable PostgreSQL fixture `cords-rbac-a95b5264264d`.
- Authorization subset including runtime gates, device exclusions/ownership and
  live role/category/device-approval reload: 19 passed in 31.66s, fixture
  `cords-rbac-a7479db7deb0`.
- Full suite including runtime locking: 47 passed in 78.27s, fixture
  `cords-rbac-71da5460a780`. A concurrent root-revocation writer waits for the
  authorized transaction and the next decision denies the revoked device.
- `-TestFilter enforced_roles::`: one passed in 19.55s, fixture
  `cords-rbac-6533fe4766b9`. A pre-delegation session can assign an authorized lower
  role, cannot edit its definition, escalate itself, manage the owner or grant a
  higher role. Removing delegation immediately denies new changes/catalog access
  on the same session. Identical historical receipt retries add no audit rows.
- Full rerun after role-handler integration: 48 passed in 74.02s, fixture
  `cords-rbac-f2dbdb77606a`; no failed/ignored tests, manual desktop fixture excluded.
- After transactional application/history integration, `-TestFilter enforced_roles::`
  passed both tests in 15.34s, fixture `cords-rbac-33f46866b336`. Current send denies
  leave sequence/event state unchanged, read denies suppress retrieval, and device
  exclusions deny both despite retained account eligibility and legacy roster
  flags. Opaque relay fixture bytes test authorization only, not MLS decryption.
- Full regression after that integration: **49 passed, zero failed** in 76.96s,
  fixture `cords-rbac-3d0b41c8d62f`, including independent TLS/MLS clients,
  persistence/restart, public-channel succession and the new authorization tests.
  The manual desktop fixture remains intentionally excluded. Fixture cleanup
  succeeded. Final combined server/core strict Clippy, workspace formatting and
  `git diff --check` passed (Git reports existing LF/CRLF conversion notices).
- Strict Rust Clippy passed. Initial new-test compile/lint findings (wrong fixture
  helper name, oversized test and unused response) were corrected before executing
  the acceptance test. No lint allowance was added.
- Server binary strict Clippy and workspace formatting verification passed after
  closing the `serve --migrate` schema-check gap. The native PostgreSQL migration
  test proves schema-14 rejection of candidate schema 15. A separately launched
  server-process startup rejection is not yet runtime-tested.
- `cargo test --locked --release -p cords-server` compiled successfully and ran
  zero unit tests; it supplies no additional server-process runtime evidence.

Remaining: finish messaging/channel/Welcome/KeyPackage/roster/lookup and live
WebSocket boundaries, authoritative action protocol, mutation access consequences,
enforced startup, new admissions and protocol/client compatibility. Channel fencing
and authenticated recovery remain checkpoint 3 requirements. Neither checkpoint 1
as a shipped enforced installation nor checkpoint 2 as complete endpoint coverage
is claimed. The staged migration must not be moved into automatic startup yet.

## Checkpoints 3–7

Not complete. Runtime enforcement, policy/MLS proof integration, moderation/deletion,
directory/UI and notifications remain required by the approved contract.

The admitted-account directory and default Members rail now have a validated
implementation slice; see the directory continuation in the existing milestone
and `docs/reports/ui-rework-20261015.md` for scope and PostgreSQL/browser evidence.
Role grouping, canonical profiles and account-centric administration remain open;
this does not complete checkpoints 3–7 or activate RBAC.

## Follow-up schema allocation, 2026-10-16

The automatic account-profile migration now owns schema 15, with migration ID
`202610150000`. The unactivated RBAC candidate keeps migration ID `202610150001`
and advances schema 15 to 16. Earlier schema 14-to-15 activation descriptions in
this report are historical. The duplicate-ID failure was diagnosed before dependent
work continued; 20 focused migration regressions and the 55-test real PostgreSQL
suite passed after correction. No production migration or RBAC activation was run.
