# Qt prototype integration — local Windows validation

Date: 2026-10-16. Scope: native presentation and avatar interaction; no protocol,
account authority, server policy, codec format, or upload-limit changes.

## Plan and reference inspection

Read UI-DESIGN.md, ADR 0011 and the embedded-avatar ADR. No VISION.md exists in
this checkout. Inspected the sibling React/Vite prototype manifest, shell,
conversation, rail, details, settings structure and styles; launched its existing
`npm run dev -- --port 5178` script and inspected conversation/settings in-browser.
The current Qt shell already provided real server/channel selection, message
history, composer, account-first member directory, profile loading, settings and
administration. Those operations remain connected to the existing Rust bridge.

Implementation units: validate independent native media timelines first; then
wire hover/profile contexts and adjust the shell/components; then validate native
QML interactions and responsive layouts. Native-test failures were diagnosed and
fixed before accepting each unit.

## Implementation

- Adopted the reference's neutral layered palette, quiet selected navigation,
  compact header search, server selection indicator/shape transition, and
  40-pixel avatar gutter beside message metadata and selectable body text.
- Member rows use quiet hover styling and whole-row hover playback. Message
  hover uses HoverHandler over the complete interactive region, including body.
- Expanded profiles use a larger avatar, card header, bounded scroll area and
  fixed Close footer. Playback starts on opening and stops on dismissal.
- Settings have named Account, Appearance, Notifications, Privacy, Password and
  Identity sections, retaining the existing draft and Rust save operations.
  Save/Discard remain accessible; narrow privacy labels wrap.
- Repeated avatar instances share the decoded default still image keyed by
  account and content. Each active instance owns its timeline; stopping restores
  the shared still. Unchanged playing/source properties do not restart it.
- Reused the native incremental Rust JXL decoder and QtConcurrent worker path,
  two 128 MiB sandbox slots, bounded 256-pixel rendered images, rational timing,
  loop counters, generation guards and visibility cleanup. No GUI-thread JXL
  decoding, WebView, React runtime or new dependency was introduced.
- Static images never acquire an animation timeline. The profile Play/Pause
  control appears only after an image is identified as animated. Reduced-motion
  and disabled-animation preferences still apply, with explicit profile override.
- Added reproducibly generated static and finite-loop JXL fixtures and native
  tests. The production-QML test injects presentation data only inside the test
  executable; the application has no fixture mode or fake backend dependency.

## Deliberate omissions and remaining functionality

Prototype DMs, enrollment/recovery simulations, fake identities/rosters, inert
message actions, arbitrary image URLs, preview banners and example photos were
not copied. Existing implemented administration operations were retained rather
than replaced by prototype simulations. No unsupported presence dots were added.
Message grouping was not invented where the prototype renders individual rows.

The Qt avatar upload/crop editor is an existing migration gap and remains absent.
The existing GIF/APNG/WebP-to-JXL processing code, signed profile representation,
5,000,000-byte limit and safety budgets were not changed. Their end-to-end upload
workflow was not validated by this UI work. Thus this is not complete Qt feature
parity or completion of all requested integration acceptance.

## Validation

- MSVC Release application and native tests build on Qt 6.10.3.
- QML lint passes (only pre-existing unused-import information).
- CTest: qml-controls, native-media, native-controller pass.
- Native Windows media tests cover static stills, finite loops, independent
  repeated appearances, stop-to-first-frame, unchanged-property stability,
  reduced-motion override, hidden/offscreen behavior and decoder-slot fairness.
- Production QML on the native Windows platform passes message-body hover,
  member-row hover, independent playback, return-to-still, expanded-profile
  autoplay and close. Captures reviewed at 1180x760 and 680x520 include chat,
  expanded profile and settings. These are presentation fixtures, not live-server
  acceptance. Existing real-vault/controller interruption tests also pass.
- Build outputs and captures live in `target/qt/windows-x86_64`; native logs are
  `native-media-tests.txt`, `native-media-windows.txt` and `ui-tests.txt`.
- Existing Qt/MSVC header warnings and test temporary-directory cleanup warnings
  remain visible. Offscreen Qt reports missing SDK fonts; visual inspection used
  the native Windows platform with system fonts.

No SSH host was accessed. Docker was absent from PATH and its standard Windows
installation path; no Podman or PostgreSQL executable was found. PostgreSQL-backed
live messaging, authenticated avatar publication, packaged clean-machine checks,
Linux/macOS and display-scale acceptance were not executed and are not passing
claims. No existing database or persistent application state was modified.

## Console identity selector follow-up

Plan: inspect the existing account registry and ADR 0004; implement the native
selector against existing authentication; validate real vault interaction; persist
missing navigation preferences in the existing encrypted account store; rebuild
and run local Qt acceptance. No new membership authority or backend was added.

The selector now has a centered heading, circular identity tiles, Add new, native
cubic background bands, responsive wrapping, selection/focus outlines and
reduced-motion-aware scaling. Password and Log in controls appear only after
selection. Selection does not authenticate. Enter submits, failed authentication
keeps the selection, and Escape/Back deselects and clears password inputs. Add new
uses real account creation; existing authenticated local removal remains available.
Cached JXL avatars use the shared native renderer and stay on their default still.
The shell's switch-identity control locks the active account before showing the picker.

AccountRegistry continues to own stable account IDs, picker-safe labels/avatar
metadata and separate encrypted vaults. The core continues to own each vault's
joined servers, credentials and active server. The Qt bridge now persists stable
server-bar order and last selected channel per server in `qtNavigation` inside
that same encrypted preference document. New servers append to the saved order;
there is no new drag-reordering control. Unknown/removed servers are pruned and
restored channel IDs are checked against the current account's cached channels.
Navigation IDs never establish membership or authorization. Existing account/server
command scopes, lock cancellation and model clearing isolate asynchronous work.

Local Rust tests verify malformed navigation rejection, separate preferences after
registry restart, wrong-password rejection and that preference IDs cannot create
memberships. Existing account tests cover restart, local removal preserving another
vault, platform lock and interruption of stalled TLS. The native QML test creates
real disposable identities and exercises the empty/one/multiple-account states,
selection, password focus, failed and successful Enter authentication, adding a
second identity, Escape deselection and resizing. It captures the actual QML at
1180x760 and 680x520. Authentication is real; chat/roster layout fixtures elsewhere
in the suite are presentation-only.

Two identities joining the same live server, independent join/leave propagation,
server-bar restoration against a live server, and cross-account live message
isolation remain unexecuted because local PostgreSQL infrastructure is unavailable.
The vault/navigation tests do not replace those acceptance cases.

Final verification: all three CTest suites passed (33.93 seconds); native Windows
controller tests passed 6/6 and media tests 8/8, including Qt setup/cleanup cases.
Rust bridge unit tests passed 3/3 and account integration tests 5/5. Strict Clippy
with warnings denied passed. The rebuilt Cords application started with an isolated
fresh state directory, captured `app-startup.png`, and exited with code 0 and no
stderr. A later narrow-window screenshot exposed delayed scroll positioning;
content-layout changes now trigger login scrolling, and the focused native picker
test passes an explicit full-login-button visibility assertion after resize.
The corrected `picker-selected-narrow.png` was visually reviewed.
