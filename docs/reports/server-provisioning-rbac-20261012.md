# Server provisioning and RBAC implementation evidence — 2026-10-12

## Scope and status

Work follows [the implementation plan](../plans/server-provisioning-rbac.md) and
[ADR 0008](../adr/0008-server-role-policy.md). This is an in-progress report, not
end-to-end acceptance. Existing picker-crop changes staged before this work are
preserved. No production deployment, role activation, commit or push has occurred.

Implemented so far:

- Stable role and permission types without changing legacy signed membership bytes.
- Deterministic evaluator for explicit admin inheritance, hierarchy, overrides,
  account/device gates and mention authorization. It is not yet used by live routes.
- Transactional role catalog/assignment persistence, expected-revision conflicts,
  idempotent receipts, atomic audit metadata and rollback tests.
- Additive PostgreSQL migration `202610120001_role_policy.sql`, schema 12 → 13.
  Existing memberships, messages, keys and channel identities are untouched.
- PostgreSQL migration connections close after success/failure and on cancellation,
  preventing SQLx's session advisory lock from leaking back into the pool after failure.

## Migration and rollback boundary

Schema 13 is preparatory: `role_policy.active` has a database constraint requiring
false. Initialization creates no role assignments, inferred signatures, device approvals,
or MLS memberships. The eventual validated activation must remove that constraint
through a NEW migration and increment the schema. Preparatory binaries must then
reject the new schema rather than ignore its restrictions.

Before migrating a live deployment, back up PostgreSQL with its matching server
signing identity, configuration and persistent object storage; rehearse restoration
only on isolated copies. No live migration has been performed here. Schema-12
binaries reject schema 13. Roll back using code compatible with schema 13 or repair
forward, not by rewinding live cryptographic state. A failed migration rolls its SQL
transaction back; a corrected migration may be retried after the failed migration
connection is closed. Successfully applied migration files must never be edited.

## Commands and environment

Windows PowerShell development; native Rust release-profile tests. Linux Docker
host `william@192.168.86.54`, existing verified SSH identity; BatchMode and
StrictHostKeyChecking enabled. Docker 29.7.2-ce. Disposable PostgreSQL 17.10
(Debian 17.10-1.pgdg12+1), pinned image:

`postgres@sha256:9b18b78397054fce88a9552e9d5a3ad5bb7fd258c5b3cc1c5028e46373d6ea8f`

```powershell
cargo fmt --all
cargo test --release -p cords-server-core authorization::tests --locked
cargo clippy --release -p cords-server-core -p cords-protocol -p cords-storage --all-targets --locked -- -D warnings
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -TestFilter websocket_requires_normal_session_protocol_and_enforces_limits
cargo test --release -p cords-protocol -p cords-storage --locked
```

The fixture script creates a uniquely named container and anonymous volume, binds
PostgreSQL to remote loopback, forwards through verified SSH to local loopback port
58201, generates an ephemeral password in memory, and removes only its own fixture
and forwarding process in `finally`. PostgreSQL TLS is disabled only inside this
loopback/SSH fixture; application TLS policy is unchanged. No secret values are
written to this report or repository. Docker administrators can inspect the temporary
container environment; this is a test-only credential and compromise boundary.

## Validation log

- Eight evaluator tests passed.
- Clippy initially reported structural/style issues; corrected. Subsequent strict
  Clippy run passed for server-core/protocol/storage, all targets.
- First real PostgreSQL run intentionally failed a migration, then hung on SQLx's
  retained session advisory lock. Verified through `pg_stat_activity` and SQLx 0.8.6
  source. Terminated only the disposable fixture's connections, recorded failure,
  removed fixture `cords-rbac-80c9063bec96`, and corrected connection lifecycle.
- Next full run: 34 passed, one existing WebSocket test failed with Windows error
  10054. Both new PostgreSQL tests passed, including migration failure/retry and
  concurrent policy writes. Added phase-specific WebSocket diagnostics without
  changing its assertions. Isolated rerun passed (1/1); this alone does not resolve
  the full-suite failure. Fixture `cords-rbac-abed55ae44ca` and isolated rerun fixture
  `cords-rbac-3eea0cc0a229` were removed.
- A fast subsequent run exposed fixture readiness racing PostgreSQL's temporary
  Unix-socket-only initialization server (23 database connection failures).
  Corrected readiness to require TCP via `pg_isready -h 127.0.0.1`; removed fixture
  `cords-rbac-538d9e485eec`. This was test infrastructure failure, not acceptance.
- Following the TCP-readiness fix, the full suite passed 35/35. Protocol tests
  (7), identity vector (1) and storage tests (3) passed.
- Added a separate signed role catalog and change receipt, native/client API and
  Svelte editor. Protocol signature/vector tests passed (2). The independent-vault
  TLS role API test passed, including owner-only access, stale revisions, exact
  retries, inactive assignments granting no access, and client/server restart at
  the same origin. An initial restart test changed ports and correctly failed
  origin verification; the fixture now restarts at its original origin.
- Frontend: 43 tests in 13 files passed; Svelte diagnostics reported zero errors
  and warnings; ESLint and Vite production build passed. Native release cargo
  check passed. Strict Clippy passed for server/client/protocol/storage.
- Fresh full rerun after adding the API and desktop fixture: 35 passed, one
  WebSocket revocation test failed while observing transport closure (Windows
  connection reset 10054). The test now recognizes only connection-reset,
  connection-aborted and broken-pipe transport errors as closure, including a
  reset during Pong flush. Application frames, other errors and timeout still
  fail; reconnect must still receive HTTP Forbidden. No runtime behavior changed.
  The subsequent full rerun passed: 36 tests, 0 failures, 59.98 seconds; only the
  separately exercised manual desktop fixture was filtered out. Strict server
  all-target Clippy passed. Disposable fixture `cords-rbac-f8087c576bd1` was removed.
- `cargo deny check` passed (advisories, bans, licenses and sources), with duplicate
  dependency and unused advisory-ignore warnings. No dependency changes or new
  advisory suppression were introduced by this work.

## Packaged Windows catalog acceptance — 2026-10-13

```powershell
$env:CORDS_DIST_DIR='D:/codex-build/cords-rbac-20261012'
./build-scripts/windows-client.ps1
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -DesktopFixtureDirectory D:/codex-build/cords-rbac-20261012/gui-20261013-a
```

Package version 0.1.0; Windows x86_64, Rust 1.97.1, Node 24.18/npm 11.16.
ZIP `cords-client-windows-x86_64-0.1.0.zip` SHA-256:
`2dfc814eabcfe4fee409bf72ef5431fa608ba1ecc4ad891b72bcbc0f93b036b3`.
Extracted `Cords.exe` SHA-256:
`6404075f9b0ea4aa0e8b2d1da0ab33eb8e05d8dccce4c7c5575d6a0ffccb09b6`.

Used a fresh installation directory, disposable account/vault, independently
generated server identity and locally trusted test CA over TLS 1.3. This explicit
test CA is not a certificate-verification bypass. The native package unlocked the
test account, connected, loaded the owner-only Roles screen, initialized built-ins
at revision 1 and created an empty, non-mentionable `GUI role` at revision 2.
The UI showed the signed server acknowledgement and kept the inactive-enforcement
warning visible. The fixture independently verified exactly one matching durable
PostgreSQL role. Manual fixture result: 1 passed, 0 failed, 592.11 seconds.
The app closed normally; fixture `cords-rbac-c292638bf00c`, its anonymous volume and
SSH tunnel were cleaned up. Personal vaults and production services were untouched.

This proves packaged catalog editing only. Packaged role enforcement, assignments,
restart, independent-client channel access and other platforms remain unvalidated.
The separate TLS API test supplies client/server restart evidence, not packaged
restart evidence. Save required scrolling to the editor bottom.

## Remaining acceptance

### Account assignment increment — 2026-10-13

Plan: add a signed, account-bound snapshot carrying its catalog and assignment set
at one revision, expose it through native commands and the member details UI, then
validate current-owner authorization, signature substitution, retry and restart.
Keep enforcement inactive. No schema or legacy signed object changes in this increment.

Implemented `CORDS-ACCOUNT-ROLES-V1`, owner-authorized
`GET /api/v1/role-policy/accounts/{account}`, pinned-key client verification and
Members → account roles controls. `$default` is implicit and cannot be assigned.
Assignments use stable account/role IDs and compare the exact snapshot revision.
The UI does not optimistically display success, and retains the exact failed request
for a retry. Reads and writes still require current owner authority.

Fixed committed assignment retries after target membership loss: the authenticated
actor can retrieve the immutable original receipt without reapplying the grant.
New requests still validate the current target; payload substitution is rejected.
The real TLS test verifies the audit count remains unchanged by these retries.

Validation commands:

```powershell
cargo test --release -p cords-protocol roles::tests --locked
cargo clippy --release -p cords-server-core -p cords-client-core -p cords-protocol --all-targets --locked -- -D warnings
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54
cargo check --release -p cords-client --locked
# In bins/cords-client/ui:
npm test
npm run check
npm run lint
npm run build
```

Results: three protocol signature/vector tests passed; strict Clippy passed;
46 frontend tests in 14 files passed; zero Svelte diagnostics; lint and production
build passed; native release check passed. Full real PostgreSQL/TLS suite passed
36 tests in 61.28 seconds, including signed assignment reads, ordinary-member and
unknown-account denial, path rejection, exact retries, and same-origin restart.
Fixture `cords-rbac-e7795f27b3c6` was removed.

Fresh packaged assignment acceptance passed (1 test, 286.48 seconds). Built with
`CORDS_DIST_DIR=D:/codex-build/cords-rbac-20261013-assignments` and the official
Windows script, then extracted into a new `installation-a` directory. Build log:
`D:/codex-build/cords-rbac-assignments-build.log`. The native UI unlocked the fresh
test vault, connected over verified TLS, initialized the role catalog, created
`GUI role`, selected its test account in Members, assigned the role, and reloaded
the authoritative assignment at revision 3. The fixture independently verified
one durable role and one account assignment. No MLS access was activated.
Fixture `cords-rbac-c9aaea01e583` and its tunnel/volume were cleaned up after the
app closed normally. GUI removal and packaged restart remain untested.

```powershell
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -DesktopFixtureDirectory D:/codex-build/cords-rbac-assignments-gui-20261013
```

ZIP SHA-256: `c4fff54834877dffa32aaddf649d1bdae7c5b7e04bf8120bcdebe5314c08022c`.
Executable SHA-256: `059d19a48cfcee25b8a75a76adcf60a3fe03b62a6d3cf70c89441f12e2fbfdb0`.

### Audit checkpoint status before the 2026-10-14 follow-up

Role-audit increment is under validation: separate `CORDS-ROLE-AUDIT-PAGE-V1`
objects, a current-owner-only GET route, native verification of server/cursor,
contiguous revisions and exact bounded page size, and an Audit Log screen limited
to role changes. Page size is 50; records include actor device, operation and target.
The database transaction timestamp is display metadata; revision defines ordering.
This is signed server evidence, not a transparency log or proof against a malicious
server. The API neither exports request bodies nor exposes keys/tokens/message text.
The four protocol tests passed. Audit-inclusive TLS acceptance passed (19.21 seconds);
48 frontend tests in 15 files passed, Svelte check reported zero diagnostics, and
ESLint, strict Clippy and native release check passed. Fresh audit package build
completed successfully. Packaged audit viewing remains incomplete: the user stopped
Computer Use with physical Escape during fixture setup, before opening Audit Log.
The stop file was issued so the disposable fixture could exit and clean up; its
expected final GUI-role assertions are not an acceptance pass.

The first audit fixture ended before inspection; no passing result was recovered.
Its container `cords-rbac-6697bbe645ec` was confirmed absent. A fresh source rerun
failed `--locked` after concurrent avatar work added `cords-avatar-source` to the
workspace without its lockfile update. That fixture (`cords-rbac-508b79ffe053`) was
cleaned up. The unrelated changes were preserved. The next isolated attempt used
the already-built test executable explicitly, not a claim of fresh workspace validation:

```powershell
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -DesktopFixtureDirectory D:/codex-build/cords-rbac-audit-gui-20261013-c -TestExecutable C:/Users/william/Desktop/git/cords/target/release/deps/cords_server_core-263e50a82b113017.exe
```

Test executable SHA-256: `5536c9a5f4870abf5a316f792d12429859b64317aba62efff40eecfcfbf0703e`.
Audit package ZIP SHA-256: `cd3128d1ea14966b1277ef7e421acee9038d4f1fcf40b1b72cd7fdbb1bb0ef54`.
Audit executable SHA-256: `06c01ab73d098822e51f2afe92f116786d569cadf2c14d9790cd6c6139fa7d8b`.
Logs: `D:/codex-build/cords-rbac-audit-build.log` and
`D:/codex-build/cords-rbac-audit-gui-c.log`. Full 50-entry pagination boundary and
packaged audit viewing remain acceptance gates. Later enforcement units remain inactive.

Live endpoint enforcement, current-state permission
reconciliation, desired MLS access, separate device approvals, mentions/notifications,
presets, SSH provisioning and the creation wizard are not complete. No functioning
role system or server-creation flow is claimed. The repository storage test uses
opaque identity fixtures and trusted internal subjects; it does not prove HTTP
authentication. New signed API/TLS and packaged catalog evidence is recorded above;
independent-client enforcement and complete requested workflows remain required.

## Audit checkpoint follow-up — 2026-10-14

The interrupted 2026-10-13 GUI fixture timed out after 1806.87 seconds and cleaned
up; it is not counted as a pass. Added a real PostgreSQL boundary test generating
52 authorized changes, then verifying signed contiguous pages of 50, 2 and 0,
future-cursor rejection, non-owner denial and revoked-session denial. It passed
in 35.22 seconds in an isolated role-work source snapshot while the concurrent
avatar workspace/lockfile change was unfinished. Strict Clippy passed there after
correcting explicit-default style diagnostics. Snapshot directory:
`D:/codex-build/cords-rbac-source-20261013-a`; baseline Cargo.toml came from HEAD,
and unrelated avatar sources were excluded. This was not whole-workspace evidence.

After the concurrent lockfile update arrived, current-workspace strict Clippy
passed and the full server regression suite passed: **37 passed, 0 failed** in
56.54 seconds. Log: `D:/codex-build/cords-role-regressions-20261014.log`.

Packaged audit viewing and refresh passed using the previously hashed audit ZIP
and fresh server/vault fixture. `-SeedAudit` explicitly uses the normal authorized
TLS API to create initialization, role-save and assignment records before opening
the app; it adds no production bypass. The native package unlocked the fixture
account, authenticated, displayed revisions 1–3 with actor/target/timestamp, and
refreshed without duplicate records. The database fixture passed its content checks
in 146.51 seconds. Its content checks alone do not prove GUI behavior; the separate
native UI observation above supplies that evidence.

```powershell
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -ForwardPort 58202 -DesktopFixtureDirectory D:/codex-build/cords-audit-20261014-a -SeedAudit
```

GUI log: `D:/codex-build/cords-audit-20261014-a.log`. Containers
`cords-rbac-acd1007eb430` and `cords-rbac-d032c495bd05` and their tunnels/volumes
were removed. The deployed `.54` Cords service was left running. This completes
the audit-view gate, not live role enforcement or the broader feature request.

## Departed-target role removal — 2026-10-14

Plan: distinguish removal of a durable assignment from granting fresh authority.
Previously both required an active target device, preventing cleanup after departure.
Keep current actor authorization, stored target rank, self/owner protections,
revision checks and atomic audit/idempotency. Do not change legacy messaging or schema.

Implementation: separate evaluator revocation authorization and a durable-account
lookup for removals. Grants still require a valid active contact and reject burned
accounts. Removal does not confer account or device admission. ADR 0008 now states
this distinction explicitly.

Validation:

```powershell
cargo test --locked -p cords-server-core authorization::tests --lib
cargo clippy --locked -p cords-server-core --all-targets -- -D warnings
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54
cargo deny check
git diff --check
```

All nine evaluator tests passed. The full server suite passed **38 tests, zero
failures**, in 61.20 seconds against disposable PostgreSQL 17.10 on `.54`, reached
through verified SSH forwarding. The TLS test removes a departed account's role,
checks its signed receipt and empty assignment list, verifies exact retry with one
audit record, and rejects an unknown target without advancing the audit head.
Evaluator tests cover burned targets and revoked actors as well as hierarchy and
owner protection. Strict Clippy and diff checks passed. Cargo deny passed all four
categories with duplicate-dependency and unused advisory-ignore warnings; no
suppression was added. These warnings are not a clean-warning result.

Log: `D:/codex-build/cords-role-removal-20261014.log`. Fixture
`cords-rbac-d4239748a236` was removed and its absence verified. Production
`cords-server` remained healthy. No packaged GUI test was run for this server-only
fix. Role enforcement remains inactive; channel/category controls and the remaining
requested workflows are still incomplete. Changes are local and have not been pushed.
