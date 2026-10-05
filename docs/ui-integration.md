# `cords-ui` to Svelte integration record

Updated October 4, 2026. The React `cords-ui` sibling is the visual and interaction reference. The production client remains Svelte inside the existing Tauri application; no React runtime, iframe, second client, fixture backend, or parallel application-state architecture was added.

## Surface mapping

| Donor surface | Production Svelte surface | State source | Initial availability |
| --- | --- | --- | --- |
| App frame and `CordsRail` | `App.svelte`, `CordsRail.svelte` | Native status plus encrypted local preferences | Real pinned server, Jewel, settings and DM navigation; multi-server switching remains unavailable |
| `ConversationSidebar` and `CordsHeader` | Matching Svelte components | Real channels, route, origin and local search | Channel selection, create, refresh and administration work; DM creation and notification actions are development-marked |
| `ConversationView` | `ConversationView.svelte` | Real decrypted local history and native send command | Sending, search and copy actions work; replies, edits, deletion, reports and attachments are development-marked |
| `ConversationDetails` | `ConversationDetails.svelte` | Real server pin, MLS epoch/roster and KeyPackage commands | Add/remove devices and publish work where authorized; retention remains development-marked |
| `IdentityPanel` | `IdentityPanel.svelte` | Real account/device/session/connection state | Functional and never receives private key material |
| `AddServerDialog` | Donor-derived connection/trust dialog | Existing trust and authenticate commands | URL-only signed discovery, initial pin and authentication work; the core still permits one pinned server per installation |
| Settings overlay and preference pages | `Settings.svelte` | Encrypted local preferences plus public identity/session views | Profile/Jewel image, theme, compact mode and reduced motion work; all other future controls remain visible and lime-marked |
| Identity and recovery pages | Settings identity/recovery sections | Real current authorization, generation and revocation state | Current-device revocation works; root unlock, enrollment and recovery are development-marked and do not simulate crypto |
| Server administration | `Admin.svelte`, `AdminUnavailable.svelte` | Real contacts, capabilities, channels, origin and pin | Overview, ownership claim, contact search, channel creation/selection and real read-only state work; unavailable mutations remain visible and marked |
| Dialogs, menus and donor primitives | Native `<dialog>`, semantic menus and shared CSS primitives | Local UI state | Navigation and safe copy actions work; inactive actions use one semantic development state |
| Donor fixtures | Not imported | None | Empty/unavailable states replace fake membership, owner, presence, audit, moderation and integration claims |

The complete donor navigation remains present: account, identity/devices, profile image, recovery, Jewel, server trust, local history, appearance, accessibility, notifications, privacy, blocked users, connections, devices, sessions, security and advanced settings; plus server overview, members, roles, channels, invites, moderation, audit log, notifications, integrations, advanced and ownership.

## Development-only controls

`--dev-unimplemented: #b7ff4a` and `.dev-unimplemented` are the single semantic treatment for visible controls whose intended operation is unavailable. The state changes text and icon foreground only, retains normal dark-surface backgrounds, sets the control inactive, exposes `aria-disabled`, and uses `Not implemented yet` as its tooltip. It must not be used for a temporarily busy real action or an authorization-denied action.

Currently marked groups include DM creation, multi-server behavior beyond the pinned server, replies/edits/deletes/reactions/attachments, notification policy, role and capability mutation, server metadata editing, categories/topics/channel deletion, invite issuance, moderation, audit filtering, integrations, ownership transfer/deletion, device enrollment, account-root interaction, recovery, blocking and exports. These controls must connect to authoritative native/server APIs before the marker is removed.

No donor sample identity, conversation, presence, role, owner, invite, moderation count, audit event or integration appears as connected state. The invite URL is retained only as a visibly labeled format example.

## Security and platform boundary

The existing `cords_client_core::client::Client` and narrow Tauri commands remain application truth. The WebView receives public identifiers, display-safe messages, capabilities and local presentation preferences; private keys, bearer tokens and MLS state stay native. Normal certificate validation and the persistent server-signing-key pin both remain required.

Images are still validated/rasterized through the existing bounded native/frontend path. Stored images are bounded PNG data, active/external SVG content is rejected, remote loading is explicit HTTPS without application credentials, and the existing `img-src 'self' data:` CSP remains unchanged.

## Performance regression record

Measurements are regression guards, not cross-platform benchmarks.

| Measurement | Before migration | After migration |
| --- | ---: | ---: |
| Production CSS | 9.12 kB / 2.70 kB gzip | 33.11 kB / 7.58 kB gzip |
| Production JavaScript | 83.64 kB / 30.26 kB gzip | 145.47 kB / 48.10 kB gzip |
| Frontend test cases | 16 | 20 |
| Locked debug client working set | Not recorded | approximately 42.4 MiB |
| Locked release client working set | Not recorded | approximately 26.6 MiB |
| Locked native idle CPU after 10 seconds | Not recorded | no measurable increase: debug remained at 0.0625 s and release at 0.03125 s |

The dedicated locked shell rendered immediately in the local browser; both native debug and release processes remained responsive. The Windows release build also produced a 7.30 MiB MSI and 5.07 MiB NSIS installer. A 1,200-message component workload initially renders only the newest 500 messages: approximately 431 ms, 8,018 DOM nodes and 44.2 MiB test-process heap growth under jsdom on this host. Expanding by another 500 messages took approximately 283 ms. These figures include jsdom/test overhead and are not native WebView frame timings.

History rendering is deliberately bounded to 500 messages with an explicit “show earlier” action. This prevents unbounded message count from immediately becoming the dominant DOM cost while preserving the donor timeline. Virtualization is not required for the current milestone; revisit it when route pagination exists or target-native profiling shows visible scroll/frame degradation. A real synchronized long-conversation WebView scroll trace was not available in the automated local environment, so target-native scrolling remains a validation gap rather than a claimed pass.

Known hotspots and follow-up work:

- The richer chassis and tree-shaken Lucide icons increased the production JS and CSS sizes shown above; the result remains a single modest application chunk.
- `App.svelte` requests a view refresh once per second after unlock while native synchronization also runs independently. Profile a connected idle client and replace polling with native events if it produces measurable wakeups.
- The 500-message window keeps initial work bounded but still grows to 1,000 when the user requests earlier history. Route-level pagination or virtualization should precede very large histories.
- Re-run memory, idle CPU and scroll-frame measurements on packaged Windows, Linux and macOS clients with a representative synchronized route before claiming cross-platform performance proof.

## Validation contract

Frontend formatting, Svelte diagnostics, lint, unit/security/performance tests and production build must pass. Rust formatting, Clippy and relevant client tests must pass without changing the Tauri command contract. Browser rendering, native debug launch and packaged build are separate evidence; neither frontend compilation nor archive inspection alone proves target-native interaction or real-server messaging.
