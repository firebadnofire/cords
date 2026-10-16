# ADR 0011: Native Qt Quick desktop and typed Rust presentation bridge

Status: Accepted under the explicit 2026-10-16 migration directive; replacement
activation remains gated by functional parity and native acceptance.

## Decision

Qt 6 Quick/QML replaces Tauri/Svelte as the desktop frontend. This supersedes
AGENTS.md sections 1.2 and 5 and framework-specific desktop/build prescriptions
only. Rust client-core remains authoritative for identities, network sessions,
cryptography, storage, profiles, policy and synchronization. Existing account
directories, encryption and protocol formats remain unchanged.

Use CXX with a thin QObject/model adapter as the equivalent narrow typed bridge
permitted by the migration directive. CXX shared structs and explicit functions
make the Rust boundary inspectable without a JSON command bus. C++ owns only Qt
presentation models and platform integration. Business operations remain Rust.
This avoids requiring Qt headers for tests of the Rust orchestration layer and
keeps Qt model/view behavior in standard QAbstractListModel implementations.

The bridge crate uses `unsafe_code = deny` with an allow on the CXX macro module
only: generated extern symbols necessarily use unsafe FFI. Handwritten Rust has
no unsafe blocks. Other workspace crates retain `unsafe_code = forbid`. This is
a scoped native-integration exception, not a crypto or codec safety exemption.

A Rust worker exclusively owns the unlocked Client. The GUI submits bounded
typed requests and receives immutable presentation snapshots. Network/database
awaits do not hold a global account mutex. Only short snapshot exchanges use a
mutex; Qt models are changed exclusively on the GUI thread. No Rust task captures
a QObject pointer. Closing the window cancels pending network work and joins the
worker after local storage transitions and SQLite shutdown finish, as described
below. Passwords are transient input, immediately cleared in
the view and zeroized in Rust; no secret state is returned to QML.

Qt runtime libraries are dynamically deployed. Pin Qt 6.10.3 and CXX in build
metadata; use the workspace Rust toolchain and Cargo.lock. Review LGPLv3/GPLv3 or
commercial Qt terms, module-specific notices and replacement/relinking rights
before distribution. The application's AGPL license does not remove Qt duties.

## Native media boundary

Reuse the pinned Rust sandbox codec from ADR 0009. A native incremental decoder
returns one coalesced 8/16-bit RGBA frame plus exact rational timing through CXX.
Qt paints the pixels directly; no browser, canvas, JavaScript codec binding or
WebView participates. Retaining the bounded sandbox bytecode preserves the
existing codec security boundary; it is not an application-shell dependency.
This supersedes the desktop browser-worker/canvas presentation prescriptions,
not signed profiles, asset formats or server validation requirements.

Two decoder slots are shared in FIFO order at cycle boundaries. Hidden,
offscreen and minimized items release their decoder; reduced motion shows a
still frame. Texture size and the visible queue are bounded. Native playback
metadata and scheduling have separate tests. Broader image editing and server
asset acceptance remain migration gates.

## Gates

Do not remove the working frontend until corresponding Qt behavior is validated.
Compilation is not runtime, media, notifications or cross-platform acceptance.
Document incomplete workflows and platform results in the migration ledger.
Android is a future layout target, not an implementation requirement here.

## Lock interruption

Manual and OS lock requests use a priority signal outside the command queue.
The controller immediately clears account presentation and discards stale
unlocked snapshots. Rust cancels network operations and closes SQLite before
dropping the vault owner; unfinished SQL transactions roll back, while a committed
transaction remains durable. A remote operation may have been accepted before
interruption, so locking never silently retries it. The locked picker reports
that the result needs review after unlocking.

Registry creation/migration/removal, password rewrap and preference mirroring
complete their local storage transitions before closing. These operations span
filesystem/registry boundaries and must not be cancelled halfway through. The
presentation remains hidden during that bounded local completion. Native OS
suspend can pause the process before completion; this does not promise erasure
of all process memory before the OS enters sleep.

Windows session notifications and power broadcasts translate to typed Rust
policy checks. Registration failure is visible. Synthetic native-event and real
stalled-TLS tests are separate from an actual workstation lock/suspend test.

## Native notification boundary

Rust classifies newly synchronized authenticated messages, suppresses initial
history, duplicates, own-account messages, focused conversations, locked content,
muted servers/channels and app Do Not Disturb. Preferences remain in the sealed
account vault. Hidden previews are the default. Presentation is separately gated
by the native adapter's availability and OS notification state; unavailable or
failed OS checks suppress presentation and expose a reason.

The current Qt tray presenter cannot identify a particular historical OS alert
when clicked. Clicking restores Cords and opens the sole pending conversation or
a pending-activity chooser. Rust owns those bounded, account-scoped targets and
validates the selected server/channel. The adapter never guesses a message ID.
Locking clears pending content and targets. An already displayed OS alert cannot
be recalled reliably by this presenter; hidden previews avoid exposing message
text by default. Linux/macOS privacy/DND adapters and actual Windows delivery
remain acceptance gates, independent of Rust classification tests.
