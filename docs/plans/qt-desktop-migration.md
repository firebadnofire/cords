# Native Qt desktop migration

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
| Message retrieval, encryption, send, route sync | Rust `Client`; cached history, no history pagination API | Qt list model; preserve IDs and history |
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

## Remaining removal gates

- Native profile image editing/conflict resolution; account removal UI; complete
  role/profile contextual workflows and authoritative member role grouping.
- Bounded history loading/scroll anchors; channel unread/mute state; automatic
  directory refresh and canonical imagery across message/member surfaces.
- Actual OS lock/suspend acceptance, Windows OS notification delivery/clicks,
  Linux/macOS lock/notification adapters, OS motion/theme adapters and file dialogs.
- Three-server background acceptance with one unavailable peer, reconnects and
  nonselected-server notifications. Existing two-server core success is narrower.
- Server presentation and moderation API gaps remain explicit; do not activate
  staged schema-15 RBAC as a frontend shortcut.
- Clean-machine Windows runtime/redist tests; Linux and macOS native build,
  packaging/runtime acceptance; CI and release deployment/notices/signing.
- Only after parity: switch default build/release entry points, remove the old
  frontend and obsolete dependencies, then validate the Qt-only repository.

The migration is incomplete. The existing frontend remains temporarily as the
behavioral reference required by the removal gate, not as the intended final
architecture. No Linux/macOS runtime or production release is claimed.
