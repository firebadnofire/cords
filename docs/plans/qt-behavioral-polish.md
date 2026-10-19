# Qt behavioral polish implementation record

Updated 2026-10-18. This is an implementation record, not an architecture authority
or a declaration that the comprehensive UI overhaul is complete.

## Plan

Keep native QML, the typed CXX bridge and Rust authoritative. Work in vertical
units: password policy; admission and the administrator queue; native profile
pictures; signed server presentation; navigation and draft safety. Validate each
unit through its persistence and authorization boundary before extending it.

Runtime: Windows/MSVC, Qt 6.10.3, Rust release builds. Real-service checks use
separate client vaults, certificate-validated test TLS and a disposable PostgreSQL
17 container on `192.168.86.54` through a loopback SSH tunnel. They do not change
the installed server or user vaults. Server presentation requires schema 17;
staged RBAC remains unsupported by normal startup (schema 18).

## Implementation

| Workflow | Current result | Limits / next unit |
| --- | --- | --- |
| Ignore weak passwords | Explicit override reaches vault creation, migration and password rewrap; includes empty input. Confirmation and current-password authentication remain. | Deliberately weak passwords reduce resistance to offline guessing; ADR 0004 records the requested tradeoff. |
| Pending admission | Durable expected state, human status, status refresh and background checks; approval enables normal synchronization. Concurrent approval and status refresh use the same identity/request lock order. | Genuine network and authorization failures remain errors. |
| Membership requests | Authoritative admin retrieval, timestamps, search, approve/reject, refresh and distinct unloaded/empty/failed states. | Endpoint supplies no applicant avatar or application message; UI does not invent either. |
| Account pictures | Native file picker, crop/zoom/shape editor, bounded JXL processing, signed save/removal and sealed restart persistence. | Locked account-picker previews remain separate from sealed public-profile media. |
| Server profile | Name/description/background color, native cropped icon upload/removal, preview, reviewed revision, device-scoped idempotency, current server authorization, signed reads, durable audit, discovery/sidebar propagation. | Uploaded banners remain separate. Backgrounds and icons appear in the profile preview and conversation details; rail icons have a bounded snapshot budget. |
| Native settings | Draft preservation on errors/concurrent refresh, discard guard, clean close during background work. | Installed application/manual interaction still needs a separate acceptance pass. |
| Administration scope | Account/server changes clear member selection, filters, ownership codes and channel drafts, and dismiss administration confirmations. Confirmed actions retain their reviewed account/server/channel context. | Backend authorization remains mandatory regardless of UI state. |
| Roles | Existing catalog and assignments remain explicitly preparatory. | Do not describe these as fully enforced RBAC or activate the staged schema from UI work. |
| Access and moderation | Existing supported operations retain their current backend contracts. | Editable admission rules, complete kick/ban workflows, and unsupported policies need separate backend-to-client units. |

## Validation

- `scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -ForwardPort 58205`:
  **59 passed**, including independent desktop workers, admission duplicates/rejection,
  real transport failures, signed server presentation, unauthorized/stale writes,
  idempotency, restart, encrypted messaging and migration rollback.
  Logs: `target/qt/polish-server-regression.log` and `target/qt/polish-icon-server-tests.log`. The interactive fixture test is
  excluded by the harness; this is not manual desktop or packaged-release proof.
- `cargo test --locked --release -p cords-qt-bridge --lib --test accounts`:
  **4 library + 7 account tests passed**. Covers policy override, authentication,
  account isolation, native PNG/animated avatar edits and signed removal/restart.
  Log: `target/qt/polish-rust-tests.log`.
- Native Qt controller tests: **10 passed, 0 failed**. They cover real QML layouts, filtering/virtualization,
  password validation, draft conflicts, avatar crop bounds and settings dismissal.
  Screenshots use explicitly synthetic presentation data; they do not replace
  service integration evidence. Final scope-switch results are recorded in
  `target/qt/polish-context-tests.txt` (superseding `target/qt/polish-ui-tests.txt`).
  Windows temporary-directory cleanup warnings remain; these do not represent failed assertions.
- A deterministic concurrency test reproduced PostgreSQL deadlock `40P01` before
  the approval fix and passed afterward. Evidence:
  `target/qt/polish-admission-race-before.log` and `target/qt/polish-admission-race-after.log`.
  The regression now also changes the applicant's signed display card while
  approval waits: fresh presentation is revalidated, rather than rejected as a
  stale request. The final full-suite run covers this case and the desktop workflow.
- Icon integration covers owner upload, invalid codec data, unauthorized member
  writes, idempotent retry, stale revisions, retrieval by a second account,
  sealed client restart, service restart and durable removal. The desktop-worker
  case additionally imports/crops a PNG and checks icon data in the server rail.
  The upload semaphore remains held through commit; Qt icon snapshots have a
  16 MiB budget. Native dialogs are bounded by the window and Save requires a
  decoded preview. No test uses the user's personal image files or vaults.
- The compiled Windows application loaded its embedded QML against a disposable empty
  state directory, captured the account picker, and exited cleanly with code 0.
  This offscreen smoke used the installed Qt runtime, not a standalone package.
  Offscreen mode cannot register Windows session-lock events and displayed that
  limitation explicitly. Evidence: `target/qt/polish-compiled-startup.png`.
- Legacy Svelte diagnostics passed with zero errors/warnings earlier in this pass.
  That check does not prove native Qt behavior.

No publication, installed-client replacement, production migration or RBAC activation
is implied by these checks. Keep personal vaults intact. Restore UI source independently
if necessary; never rewind a vault to undo a password change. Server migration rollback
requires a database backup and a compatible server version, not blind schema-number edits.
