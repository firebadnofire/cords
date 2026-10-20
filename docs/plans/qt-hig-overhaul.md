# Native desktop HIG implementation ledger

The adopted repository-root [HIG](../../UI-DESIGN.md) governs design; security and
operational semantics remain governed by AGENTS.md and relevant ADRs. This ledger
records implementation, not a second design specification. Updated 2026-10-19.

## Implementation sequence

- [x] M0: adopt HIG metadata/contributor authority; verify tools/output storage;
  inventory conformance and actions; capture the compiled real-service baseline.
- [ ] M1: shared native controls and theme, all interaction states, accessibility,
  sizing/density and rendered comparisons (HIG 2, 4, 6.3, 8, 9).
- [x] Early admission slice: submit/persist/retrieve/review/approve or reject/
  requester transition, independent vaults and native presentation (HIG 5.5, 7).
- [ ] M2: complete messenger transformation and separate side-by-side region
  reviews; no broad settings expansion before this gate passes (HIG 3, 6, 10).
- [ ] M3: settings and existing operations (HIG 5); independent of missing RBAC.
- [ ] M4: independently gated RBAC, invitations/rules, moderation/message units.
- [ ] M5: complete native accessibility/platform/package regression acceptance.

Each unit follows implement -> validate -> record. Diagnose failures before
dependent work. Personal state, production services and staged RBAC stay untouched.

## Environment and existing capabilities

Windows x64; Rust/Cargo 1.97.1, Python 3.13.14, Qt 6.10.3 MSVC, VS 2022
toolset 14.44 and bundled CMake 3.31.6 were inspected. A new build remains a
separate gate. Dot-source `scripts/qt-workspace.ps1` before builds/tests; override
`CORDS_QT_OUTPUT_ROOT` for another host. This host uses external E: storage because
C: is nearly full. Build, Cargo, temporary, cache, fixture, evidence and package
locations must be external; installed tools and existing Cargo credentials stay put.

Source has native send/history, signed account profiles, JXL crop/publication,
server presentation, durable admission and an administrator request queue.
Current normal PostgreSQL schema is 17, superseding older migration-ledger
schema-15 observations. Preparatory roles are not enforced RBAC; staged schema 18
must not be activated by this UI work. Historical test reports are not new proof.

## Conformance and interaction inventory

| Surface / HIG | Baseline discrepancy / implementation unit | Real operation and authority | Verification required |
| --- | --- | --- | --- |
| Shared controls / 2,4,8 | Navy tokens, flat typography, incomplete busy/error/menu states and mixed glyphs | Local presentation/preferences; no policy evaluator | Keyboard, contrast, states, theme/density native captures |
| Rail/navigation / 3.1-3.3 | Oversized sync action; inconsistent hierarchy/context actions | Typed select/join/refresh; eligible account memberships | Scoped restoration, independent sync, narrow captures |
| History/composer / 3.4-3.5 | No grouping, generic empty text, separate Send button | Rust history/send; current admission and channel/MLS access | Anchors, retries, disabled reasons, draft isolation |
| Members/profile / 3.6,6 | Sparse rows; profile dialog rather than contextual card | Verified directory/profile; no invented presence | Profile propagation, JXL row hover, Members/Info replacement |
| Requests / 5.5,7 | Needs native workflow proof and clearer identity/state presentation | Request/status/list/approve/reject; current staff authority | Independent vaults, denied/empty/failure, duplicate/restart |
| Settings / 5 | Existing forms/draft guards need coherent presentation | Revision-aware signed saves and sealed preferences | Save/cancel/conflict/restart, wide admin tables |
| Roles/moderation / 5.3-5.6 | Backend enforcement incomplete | Existing security checkpoints, not visibility-based grants | Separate service/HTTP/WS/sync, MLS, migration acceptance |

For each changed control record: expectation -> typed command/local action ->
authority -> authoritative result -> refreshed state -> feedback/recovery -> evidence.

## Acceptance evidence

Capture normal and interaction/error states at 1440x900, 1024x768,
150% scaling, narrow and large-font variants. Inspect side-by-side references for
each messenger region, separately evaluating geometry, density, typography,
hierarchy, control placement, avatars, selection, contextual behavior and states.
Applying colors/rounding alone never passes M2. Record offscreen rendering,
compiled execution, interactive acceptance, real-service tests and extracted
package checks separately. Linux/macOS interactive acceptance remains outstanding.

### 2026-10-19: environment, shared components and admission

Evidence filenames below resolve under the selected output root, rather than
inside the checkout. This host's root is `E:\CordsBuild`; the existing external
Cargo target `E:\codex-build\cords-qt-cargo` was explicitly reused. No caches,
credentials or personal state were moved. Public dependency cache files were
copied into `cache/cargo-home`; new dependency, temporary, QML and package output
is external. Dot-sourcing the setup twice succeeded. C: remains nearly full;
E: had over 100 GB free. Optional Vulkan headers are absent; the Windows build
uses the supported native renderer without requiring Vulkan.

**M0 evidence:** `evidence/baseline-build.log`: fresh MSVC native build,
QML lint, all four CTest suites and package install passed. Compiled baseline
captures are `evidence/baseline-{wide,standard,scaled,narrow}.png`, using disposable
real TLS/PostgreSQL accounts, not a personal vault. Capture dimensions were
1440x900, 1024x768, 900x650 logical at 150%, and 680x520. The first wide baseline
was captured from the preexisting native build before theme edits; subsequent
fresh-build evidence is distinct. One early timed capture exited without writing
an image; longer capture/quit intervals produced the required files. Baseline
wide/narrow and the supplied Discord administration reference were inspected.

**Implemented M1 units:** centralized charcoal/light/midnight semantic surfaces;
text scaling and independent server icon sizes; native stroke icons; shared
button/field/check/menu/dialog/empty-state treatments; keyboard tooltips and
explicit busy, error and success presentation. Text and icon sizes are typed,
validated, account-scoped encrypted preferences, preserving unknown preference
fields. Shared menus replace the previous unstyled contextual menus. The local
removal dialog now scrolls within the window when real Windows fonts need more
space. No removal operation was performed against personal state.

`evidence/m1-rust-tests.log`: four bridge unit tests and eight real-vault account
tests passed, including valid/invalid sizing values and sealed restart. The first
new test used a nickname-containing password; the policy correctly rejected it.
The fixture was corrected; password policy was preserved.

`evidence/m1-controls.log`: 20 QML tests passed with Windows fonts, including
keyboard operation, disabled/busy actions, menu Escape and semantic contrast in
all three themes. Contrast checks cover ordinary text at 4.5:1 and input borders
at 3:1. They caught insufficient warning/success/muted/destructive contrast and
the QML-sensitive `onAccent` property name; the foreground token is now
`textOnAccent`. HIG 2.2 explicitly allows these baseline color corrections.

`evidence/native-admission-interaction.log`: native Windows execution, four
QtTest cases passed (including setup/cleanup). The actual requester/admin QML
surfaces executed rejection, status retrieval, resubmission, approval and active
requester transition against the live TLS/PostgreSQL fixture. Pending clients had
no channels, member rows or approval authority; after approval the directory had
both admitted accounts. The native test deliberately retains the rejected review
row, matching the server API, rather than treating rejection as deletion.
`evidence/native-admission-fixture.log` passed its post-UI vault reopen, active
membership, two-account directory and untouched staged-RBAC-default checks.
Both temporary fixture servers/containers were stopped through their harness.

`build/admission-native-{pending,review,confirmation,denied,approved}.png` were
captured; pending/review/denied/approved were inspected. The pending surface names
the server and provides a real status action; denied explains resubmission.
`build/controls-{dark,midnight,light}-{100,150,200}.png` and `controls-menu.png`
provide native component evidence, explicitly labelled presentation fixtures.
Dark 100%, light 200% and the menu were inspected. Screenshot review caught a
collapsed menu width and it was corrected and recaptured.

`evidence/admission-tests.log`: three independent-client TLS/PostgreSQL admission
tests passed, including duplicate requests, rejected and pending restart,
resubmission, forbidden retrieval/actions and approved directory visibility.
Additional concurrency evidence is tracked separately; native UI success does
not prove every race case.

**Checks still required / limits:** strict Clippy currently fails on existing
avatar cast warnings and existing runtime style warnings, outside the new sizing
logic (`m1-bridge-clippy.log`, `m1-bridge-only-clippy.log`). These are recorded,
not suppressed or represented as a pass. Full screen-reader acceptance and
cross-platform native runs remain outstanding. `m1-package/installed-*.png`
passed clean-environment package startup on this SDK-equipped host at all four
capture sizes, but predates the last component fixes and is not clean-machine
proof. The latest rebuild/package result is recorded separately when complete.

**Visual gate remains open:** the baseline discrepancies above are an inventory,
not a claim that each remains unchanged. M2 implementation and remaining acceptance
are recorded below. Palette adoption alone does not satisfy M2. M1 is not declared
fully accepted while shared-component/accessibility checks remain outstanding.
No broad settings expansion or staged RBAC activation has occurred.

### 2026-10-19: native messenger implementation and refinement

Implemented the four-region geometry, edge selection marker, native header icons,
loaded-history search (Ctrl+F/Escape), verified-account message grouping with
five-minute/day boundaries, individual accessible timestamps, supported contextual
copy actions, integrated multiline composer, scoped session drafts, member search,
and shared contextual account cards. Profile editing continues through the existing
revision-aware editor. Roles are displayed only when their real IDs resolve through
the existing catalog; no role enforcement or presence was invented.

The composer explains pending/rejected admission, archive/succession, unavailable
MLS membership and offline sending separately. Drafts survive navigation and
recoverable failures within the unlocked shell. **They are not durable across lock
or restart yet**; HIG 3.5 remains an open M2 dependency. No plaintext draft storage
was introduced. Existing history bounds and anchor logic were retained.

`evidence/m2-final-build.log`: fresh native MSVC build, QML lint, all four CTest
suites, and Windows package deployment passed. `evidence/m2-native-sizing.log`
separately passed real Windows rendering across three themes and 100/150/200%
text sizes. The native geometry assertion now waits for a completed rendered frame;
a pre-polish sample had reported the previous size. Native execution is distinct
from the offscreen CTest run.

`evidence/admission-concurrency-tests.log`: three real TLS/PostgreSQL tests passed,
including concurrent pending-status refresh/approval and approval/channel restart
persistence. The third is an isolated existing authorization-migration test; it
does not activate staged RBAC in the normal desktop fixture. Together with the
previous independent-vault native approval/rejection evidence, the early admission
slice is accepted independently of the remaining administrative units.

The earlier messenger fixture timed out during extended validation and its harness
removed its disposable container. This was a failed fixture completion, not a
successful restart assertion (`evidence/messenger-fixture.log`). A fresh isolated
fixture was started for refinement; production state was never used.

A computer-use inspection of the extracted native package verified canonical
Alice/Bob profile retrieval, actual animated JXL avatars, contextual card loading,
readable punctuation and the card close icon. Focus inspection exposed a delegate
lifetime risk: profile refresh can replace the opening member row. The helper's
focused-element metadata was inconsistent, so it did not alone prove a product
focus failure. Restoration now resolves account identity after Qt finishes closing
the popup; a model replacement regression case exercises this path. `evidence/m2-native-refinement-tests.log` passed five native Windows cases
(including setup/cleanup), including the replacement regression, composer scope
and search/menu/profile interactions, and the three-theme sizing matrix. Synchronization is moved into the supported server menu and
member refresh into a named header icon; both retain their typed operations.

| Element | Expectation / typed command or local action | Authority and outcome | Failure / recovery and evidence |
| --- | --- | --- | --- |
| Server menu refresh | Explicit `synchronize()` | Rust eligible account sessions; refreshed snapshot | Busy/failed outcomes remain visible; native menu test/inspection |
| Loaded history search | Filter loaded message bodies, Ctrl+F/Escape | Designated local filter only; no server plaintext query | Explicit loaded-data scope, distinct no-match; controller test |
| Message group/action | Same verified account/time boundary; copy body/ID | Local presentation/clipboard; unique IDs retained | No unsupported edit/reaction controls; grouping/context-menu native tests |
| Composer | Enter send, Shift+Enter newline; scoped input | Typed Rust send and current server/MLS authority | Retain failed text; explicit restrictions; native scope test; durable lock draft remains open |
| Member search/refresh | Filter loaded directory; `loadDirectory("")` | Authenticated admitted-account directory | Loading/empty/offline/failed differ; real service package and native tests |
| Profile card | `loadProfile(account)`; Details, playback, Escape | Verified canonical signed profile/JXL decoder | Loading/retry/asset errors; focus restoration after model replacement |

Side-by-side evidence is `evidence/messenger-comparison.html`: baseline/current
native package and the supplied Discord navigation/administration reference.
It explicitly identifies regions not shown in the supplied screenshots; those
require HIG anatomy/component comparisons rather than an invented reference.

| Dimension | Current comparison finding | Acceptance still required |
| --- | --- | --- |
| Geometry | Default 72/240/flexible/240 matches HIG anatomy and reference rail proportions | Recheck latest compact refresh at all target sizes |
| Density/spacing | Compact rows, tighter grouped messages, one composer surface replace baseline prototype spacing | Large datasets and all narrow-window states |
| Typography/hierarchy | Semantic body/caption/heading sizes, stronger names and quiet time labels | Full screen-reader and large-font messenger review |
| Control placement | Stable search/Members/Info; contextual profile and action controls | Native focused/hover/menu captures after final refinement |
| Avatar treatment | Real independent circle/square JXL media and bounded card avatar | Broader animated/offscreen/reduced-motion platform regression |
| Selection indicators | Rail-edge indicator and persistent selected channel surface | Full keyboard-selected states and theme matrix |
| Contextual interactions | Supported menus, Details, outside/Escape dismissal, restored member focus | Latest real-service focus regression validation |
| Visual states | Pending/denied/approved captured; loading/search-empty/offline reasons implemented | Complete denied/network/empty matrix, not inferred from a successful render |

M2 remains open pending these checks and durable draft protection. M3 broad settings
expansion has not begun. Strict Clippy warnings and Linux/macOS/package limitations
recorded above still apply.

The native tests retain a Qt temporary-directory cleanup warning. Inspection found
an ordinary `accounts.db` file (not marked read-only) in the retained fixture;
the exact handle/lifetime cause is not yet established. These files are confined
to the external disposable test root, and the warning is not treated as proof of
successful cleanup. No personal database was inspected.

The in-app browser refused the local comparison HTML because `file:` URLs are
blocked by its URL policy. No alternate browser/workaround was used. Source
reference images and native captures were inspected directly; the HTML is a
review artifact, not claimed evidence of browser-rendered visual acceptance.

**Final refinement evidence:** `evidence/m2-refinement-final-build.log` passed
QML lint (no new lint warnings), all four CTest suites and package deployment.
`evidence/m2-package-final-captures.log` passed clean-stderr package startup at
1440x900, 1024x768, 680x520 and 900x650 logical at 150%. The resulting
`evidence/m2-package/installed-{wide,standard,narrow,scaled}.png` captures use
real independent fixture profiles and were inspected at wide/narrow/scaled sizes;
the standard capture is retained for review. They have correct UTF-8 punctuation
and loaded canonical names/media. A temporary file-edit encoding error was caught
in earlier captures, corrected in source and rebuilt; those earlier captures are
not the final evidence.

Computer-use checks on the final extracted package opened Bob's canonical signed
profile, observed animated media, dismissed with Escape, then used Space to reopen
the same Bob card. This verifies the actual keyboard target. Windows accessibility
reported an inconsistent focused label, so that metadata is not used as proof of
full accessibility acceptance. The supported server menu and compact member
refresh were exercised; the admitted directory remained visible without an error.
The native client exited cleanly and its log was empty. An intermediate admission
success notice still shifted conversation geometry during inspection; the broader
feedback/state matrix remains an open visual-review item.

`evidence/messenger-refinement-fixture.log` completed successfully after the stop
sentinel, including its post-UI checks and untouched staged-RBAC defaults. Its own
PostgreSQL container and SSH tunnel were cleaned up. Three server-side connection
closure diagnostics occurred during the repeated package open/close cycles; they
are retained in the log rather than hidden. The server test used the explicitly
hashed previously source-built executable; this is not a claim of another fresh
server compilation. Fresh source-build evidence remains in the earlier Rust and
service test logs.

Next dependent M2 units remain encrypted durable drafts (including lock/restart),
account-strip verified avatar propagation, feedback layout stability, and the full
state/keyboard/accessibility comparison matrix. No milestone is marked complete
merely because a package launches.

### Add-server admission regression (HIG §§5.5, 7, 10)

User reports exposed a gap in the previously accepted approval slice: the native
Add Server command only pinned trust. It never submitted admission, so background
sync attempted ordinary authentication, displayed a permission-denied error, and
the administrator correctly retrieved an empty request queue. The prior native
fixture pre-submitted admission and therefore did not exercise this entry path.

Minimal unit: after verified trust of a claimed server, run the existing typed
admission operation and present its authoritative outcome. Keep unclaimed-server
ownership bootstrap separate. Explain admission in the add dialog. Do not infer
pending membership from a generic 403 or grant access to an applicant.

Regression inventory: Add Server → `Trust` → verified TLS/pinned discovery →
`request_admission` → durable server request → pending UI → administrator request
retrieval → reject/resubmit/approve → requester status transition. Start the
real-service native fixture without trust or admission; assert the original queue
is empty and repeated Add Server produces one request. Existing pending isolation,
native screenshots, denial and approval checks remain required.

Validation: `evidence/admission-add-build.log` records a fresh release bridge/native
build, QML lint, all four CTest suites passing, and deployment to
`packages/current`. `evidence/admission-add-fixture.log` records a fresh release
server test build and disposable PostgreSQL 17.10/TLS service. The Windows-platform
`liveAdmissionWorkflow` passed (`evidence/admission-add-native.log`), including an
initially empty admin queue, Add Server submission, duplicate-safe repetition,
pending isolation, native reject/confirm/status/resubmit/approve interactions,
and the admitted two-account directory. Fixture shutdown reopened the requester
vault and verified active membership, directory persistence and untouched staged
role defaults; the fixture's container and SSH tunnel were removed successfully.

Inspected `build/admission-native-pending.png` and
`build/admission-native-review.png` against the reported screenshots: the raw 403
and generic empty conversation are replaced by the explicit approval explanation
and status action; Requests contains the verified applicant, submission time,
awaiting-review state and working decision controls. Existing saved/trusted-only
servers can submit through Authenticate / request admission, followed by admin
Refresh; no existing account or server state is rewritten automatically.

Deployment still reports missing optional `dxcompiler.dll`/`dxil.dll` and unset
`VCINSTALLDIR` warnings; successful compilation and native runtime testing are
recorded separately from package deployment. This fix does not establish packaged
runtime acceptance or complete the broader M2 visual/accessibility gate.
