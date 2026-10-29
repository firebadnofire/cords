# Native Qt desktop migration

Current HIG overhaul work and evidence are tracked in
[the native HIG implementation ledger](qt-hig-overhaul.md). Its source-verified
observations supersede older status snapshots below; the removal gates remain.

Date: 2026-10-16. User-authorized replacement of Tauri/Svelte. This is an
implementation ledger, not a claim of parity or platform acceptance.

## Plan

1. Inventory the current adapter and core; preserve the dirty checkout and data.
2. Build Qt Quick with a typed Rust bridge; test registry/vault compatibility,
   worker ownership, failure propagation, locking and clean shutdown.
3. Port shared controls, four-region shell, accounts, navigation and messaging.
4. Port supported profiles, members, roles, requests, channel lifecycle and owner
   workflows; preserve revision checks and authoritative server errors.
5. Implement native media and platform services, account-wide synchronization,
   packaging and regression acceptance. Retire the old frontend only after parity.

Each unit follows implement -> validate -> record. Diagnose failed gates before
dependent work. Do not delete unrelated changes or change account/wire formats.

## Audit of current source

| Feature | Current implementation | Replacement boundary |
| --- | --- | --- |
| Account registry, creation, unlock, legacy migration, password change, removal | Rust `accounts.rs`; Tauri adapter adds picker privacy and native watchdog | Keep core and vault paths; typed account commands |
| Server trust, possession authentication, ownership, departure/burn | Rust `Client`; Tauri `Action` adapter | Direct typed operations, no QML secrets |
| Channels, immutable confidentiality, succession, archives, roster/MLS | Rust `Client`; functional Svelte workflows | Typed commands and channel models |
| Message retrieval, encryption, send, route sync | Rust `Client`; authenticated cached history with bounded indexed windows | Qt list model; preserve IDs and visible message anchors |
| Admitted account directory, requests, devices | Verified Rust API and existing Svelte member panel | Account rows separate from privileged devices |
| Canonical profile history, conflicts, images | Dirty-tree core/protocol/server implementation; ADR 0010 acceptance still gated | Preserve signed graph and expected heads |
| Roles, assignments, role audit | Signed preparatory catalog API; enforced-RBAC work is separately checkpoint-gated | Never imply roles activate unshipped enforcement |
| Categories, server presentation editing, kick/ban/timeout, deletion, invites | Not available through the current client-core API; some server candidates exist | Explicitly incomplete; no fabricated UI success |
| Synchronization | Current Tauri task operates on selected server and holds runtime mutex during network waits | Worker ownership; account-wide scheduling still required |
| Notifications | `Client::notifications` is a WebSocket wakeup, not approved OS notification classification | Do not mislabel socket wakeups as desktop alerts |
| Media | Pinned local libjxl WASM validation, browser workers/canvas playback | Native Qt playback must be validated before removal |
| Packaging | Tauri/Vite build scripts, Forgejo Linux and Windows cross-build | Native Qt deployment and target-native evidence required |

## Runtime constraints

Host: Windows x86_64, PowerShell, Rust 1.97.1, MSVC Build Tools installed.
CMake exists in Visual Studio but is not on PATH. Qt 6.10.3 MSVC was installed
in isolated external build storage, outside the repository and user vault.
C: and D: have limited free
space; use E: for new SDK/build data. Linux/macOS runtime acceptance is unavailable
on this Windows host. No repository-root VISION.md is present.

## Validation ledger

- Audit: inspected UI-DESIGN.md in full, Tauri commands, core APIs, account/key
  contracts, ADRs 0004/0008/0009/0010, workspace/build configuration.
- No existing local data has been opened, migrated or modified by this task.
- Typed CXX static library and Qt Quick executable build with MSVC 2022 and
  Qt 6.10.3. The narrow generated FFI module is the only unsafe-code exception.
- Real registry/vault acceptance: account creation, encrypted reopen, wrong
  password, lock clearing, runtime shutdown and original vault-lock release pass.
- Independent TLS/PostgreSQL bridge acceptance passes: two vaults, existing
  history, send/receive, canonical signed name edit, admitted account directory,
  catalog read without activating RBAC, persisted message after restart.
- Core TLS multi-server lifecycle acceptance passes with a message received on
  the background server while another server remains selected after reload.
  Failure/interrupted-background-persistence tests preserve foreground selection.
- Native incremental JXL codec tests pass for 8/16-bit fixtures, exact rational
  durations, pixel output, malformed input and sandbox bounds. Qt renderer tests
  pass timing, two-slot decoder fairness, pause/resume and offscreen/hidden pause.
- Windows native UI inspected against disposable real-service accounts: messages,
  account-centric directory, canonical profile/name propagation, animated JXL,
  Members/Info navigation. Found profile-avatar sizing and fingerprint wrapping
  defects; corrected source and rebuilding for repeated visual inspection.
- Measured one Windows development client with a cold JXL decoder at peak working
  set 386,068,480 bytes. This is not worst-case whole-client resource acceptance.
- Qt Quick control tests and QML lint pass. Unused-import lint information and
  warnings inside Qt headers remain; no runtime QML errors observed in this run.
- Priority lock cancels a stalled TLS operation, clears Qt models synchronously,
  closes the vault and allows reopening. Rust policy and a synthetic Windows
  session-lock event pass; actual workstation lock/suspend remains untested.
- Real TLS bridge acceptance passes startup notification suppression, one generic
  alert for a new message, Rust-owned conversation navigation, persisted server
  and channel mutes, and lock cleanup. Classifier unit tests cover own-account,
  focused chat, deduplication, app DND and unavailable platform suppression.
- Installed Windows notification settings showed presentation unavailable or
  suppressed on this desktop. No OS alert or OS-click delivery is claimed.
  Native status now distinguishes tray absence, message support, OS query failure
  and Windows suppression states. Linux/macOS notification adapters remain absent.
- Installed Windows startup/layout passes at wide, narrow and 150% scaling
  without Qt SDK on PATH or source migration overrides. This is not a clean
  machine runtime/redist or signed release acceptance.
- Automatic metadata refresh now fetches one verified directory page or canonical
  account profile per tick, within the existing 30-second synchronization budget.
  Directory refresh stages the loaded page range before replacing public rows;
  profile retries and failures are visible. Shared compressed avatar sources are
  limited to 128 accounts and 16 MiB and cleared on account/server changes.
  Real TLS two-vault acceptance passes directory/name/JXL propagation to message
  and member presentation without opening a profile popout. Native layout/media
  inspection passes at wide/narrow sizes and 150% scaling, including native
  circle/square JXL avatars in message and member rows.
- Server Docker builder copies the new Qt Rust workspace member. Reconstructing
  its exact COPY inputs passes locked/offline Cargo metadata for all 13 members.
  An isolated checkout on `.54` also builds the complete OCI image successfully;
  running its binary with `--version` succeeds as the non-root image user. This
  does not claim deployment or database/runtime acceptance.
- Bounded history uses an indexed server/route-scoped cursor, decrypts at most
  300 rows, initially loads 100, and grows in steps of 100 before pruning the
  newest edge while reading older messages. The upper bound stays frozen across
  new arrivals; Latest and a successful send restore the current 100-row tail.
  Account/server/route changes, locking and restart reset the window. Core tests
  cover tied sequence values, cross-scope cursors, persistence and authenticated
  cache tampering. Independent-vault TLS acceptance passes new arrivals during
  browsing, bounded pruning, stale-route rejection, send, lock and reopen.
- Native model tests preserve persistent indexes across prepend/prune/update.
  QML viewport tests cover variable-height rows, retained ID/pixel offsets,
  pruning, Latest and resize. Installed Windows interaction preserves the visible
  row through two prepends and returns to the full latest message. Long sender
  labels now elide within the message column at narrow sizes.
- Redirected package tests now capture Qt diagnostics. A debugger trace identified
  an attached tooltip's dynamic generic Controls import outside deployment scans;
  buttons use an explicitly imported Basic tooltip instead. A visible native QML
  hover test verifies showing and hiding its help text.
- Local account removal is available from the locked picker after selecting one
  account; settings explain the route. Its modal identifies the local target,
  explains permanent vault/key/history deletion and recovery-copy preparation,
  and requires that account's password. It does not issue remote revocation or
  identity burn. Input clears on cancel/submit; busy actions cannot duplicate
  submission. All five bridge vault tests pass, including wrong-password
  rejection and preservation of a second vault/device after removal and restart.
  QML tests pass target binding, cancel, input cleanup and bounded dialog layout.

- Native avatar playback is shared by verified account ID plus image-content
  digest across message, member and profile views; the locked picker supplies
  its local registry identity. One entry owns the decoder, animation clock,
  visibility polling and implicitly shared frame pixels. Crop/shape remain
  local to each view. Anonymous images and different accounts never share on
  display names. Source replacement selects a new entry and stale in-flight
  decode results cannot update it. Weak registry/FIFO references release the
  entry after its final consumer disappears. The two sandbox slots and
  64-distinct-image bound remain enforced; repeated appearances do not consume
  additional slots or entries.
- Reduced motion and the stored disable-animation preference apply to every
  avatar. The profile popup offers explicit Play/Pause without resuming other
  paused appearances. Hidden/offscreen views retain their still frame and
  resume on the account's current frame; decoding stops when no eligible view
  needs it. Native tests exercise 80 appearances sharing the same decoded
  pixels/frame sequence, independent pause/offscreen handling, account/image
  isolation, crop/mask independence and final-consumer cleanup. All six media
  test cases and all three CTest suites pass in native Windows and Linux
  optimized builds. Both official build scripts produce complete Qt packages
  and checksums with this change; Linux extracted-package startup passes with
  the SDK environment removed (headless backend). Extracted Windows
  startup against the disposable TLS fixture also passes with no SDK PATH or
  source migrations at wide/narrow sizes and 150% scaling; repeated account
  avatars visibly show the same frame and all captures exit with clean stderr.

## Remaining removal gates

Build-script checkpoint (2026-10-16): Windows native script produces a complete
Qt ZIP and checksum; its extracted development and optimized packages pass real TLS
startup and imagery at wide/narrow/150% scaling without SDK PATH or source
migrations. Linux native script produces an optimized Qt tarball and checksum
on an isolated `.54` checkout, passes QML lint and all three CTest suites
(optimized media suite approximately 10 seconds), and its extracted package lists accounts and renders
the native picker with a clean environment using the deployed offscreen backend.
X11/Wayland platform plugins are included explicitly. Actual X11/Wayland sessions,
desktop portals and clean-distribution compatibility remain unverified. The
unoptimized Linux media run exceeded its 90-second test startup bound; no timeout
was relaxed to pass the optimized build.

Active CI now defines native Qt Linux/Windows/macOS jobs with pinned SDK/build
tools. YAML and embedded shell/PowerShell syntax checks pass; hosted runs and
Windows/macOS runner availability remain unverified. Publication remains disabled.
macOS PKG/DMG builders have native Qt paths and shell syntax validation, but no
macOS build/runtime evidence. Legacy packaging helpers reject standalone old
frontend artifacts rather than silently omitting the Qt runtime.

- Native profile image editing/conflict resolution; complete
  role/profile contextual workflows and authoritative member role grouping.
- Channel unread state; automatic
  directory/profile refresh failure/pagination and native imagery acceptance at
  larger scale. Basic two-vault automatic propagation passes.
- Actual OS lock/suspend acceptance, Windows OS notification delivery/clicks,
  Linux/macOS lock/notification adapters, OS motion/theme adapters and file dialogs.
- Three-server background acceptance with one unavailable peer, reconnects and
  nonselected-server notifications. Existing two-server core success is narrower.
- Server presentation and moderation API gaps remain explicit; preserve current
  server policy activation and enforcement boundaries. Current source requires
  PostgreSQL schema 15 (baseline commit 469b67b); the synthetic catalog fixture
  remains preparatory. Earlier schema-14 notes were stale, not an instruction to
  downgrade the repository or activate policy from the frontend.
- Clean-machine Windows runtime/redist tests; Linux desktop-session acceptance;
  macOS native build, packaging/runtime acceptance; CI and release
  deployment/notices/signing.
- Client build scripts now select Qt explicitly at the user's request. Release
  integration and interactive Linux/macOS acceptance are still pending. Only after
  parity: remove the old frontend and obsolete dependencies, then validate the
  Qt-only repository.

The migration is incomplete. The existing frontend remains temporarily as the
behavioral reference required by the removal gate, not as the intended final
architecture. Linux package checks are headless; no interactive Linux/macOS
desktop acceptance or production release is claimed.

## 2026-10-16 prototype interaction amendment

The prototype alignment request supersedes the shared animation clock described
above. Still pixels remain shared by account/content; active message, member and
expanded-profile instances now have independent timelines. Message/member rows
play on hover and reset to the still frame on exit. Expanded profiles play while
visible, subject to loops and motion preferences. See
[the local implementation and validation report](../reports/qt-prototype-integration.md).
Previous Linux results do not validate this new interaction policy.

## 2026-10-21 account-vault presentation cache

The native snapshot now hydrates visible message/member/directory avatars from
the unlocked account's existing verified profile history and sealed image cache,
without waiting for a network refresh. Signed display names already used that
history. The cache is shared across servers within one vault; lock/account switch
still clears presentation. Initials remain appropriate for unknown or evicted
pictures. This adds no identity authority, membership, external image requests,
database migration or dependency. Existing 128 MiB sealed image and 16 MiB native
presentation limits remain in force, and network refresh errors remain visible.

Windows validation: client-core and Qt bridge `cargo check --locked` passed;
`cached_foreign_profile_survives_restart_and_is_vault_isolated` passed (foreign
signed name/image, offline reopen, independent vault, invalid ID, corrupt media,
image eviction); native bridge
`native_avatar_edit_persists_signed_profile_and_removal` passed with an added
assertion that the JXL image and shape are present immediately after unlock.
Client-core strict Clippy passed. Strict bridge Clippy is blocked by existing
avatar casts and runtime warnings, including oversized functions and a nested
server-icon condition. These are core/bridge checks, not packaged GUI or live
independent-client/network acceptance; those were not executed for this change.
An additional Cargo test rebuild hit disk exhaustion (Windows error 112) while
creating the bridge static library. Rebuilding stopped; the existing compiled
test executable passed `existing_vault_restart_password_failure_and_lock_clear_plaintext`.
No build-cache cleanup or packaged-client replacement was performed.
