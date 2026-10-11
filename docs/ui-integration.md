# Production desktop UI integration record

Updated October 10, 2026. The production interface is maintained directly in Svelte
inside the existing Tauri application. Its current layout remains intact; future
polish follows production behavior and user needs rather than an external reference.
The native client-core remains the authority for identity, messaging and persistence.

## Production surfaces

| Surface | Source of truth | Current behavior |
| --- | --- | --- |
| App frame, server rail and sidebar | Native status, pinned server records, verified channels and encrypted local preferences | Multiple server selection, leave/archive menus, channel selection/creation and administration |
| Conversation | Verified signed public messages or locally decrypted MLS history | Sending, local search and copy actions; other unsupported message actions remain explicitly disabled |
| Conversation details | Verified member directory, MLS roster, server pin and KeyPackage commands | Named device additions/removals where authorized; public-channel access explanation |
| Identity and account picker | Public device identity and filtered local registry presentation | Independent local account vaults, password unlock, lock/switch/sign-out and authenticated account removal |
| Settings | Encrypted local preferences and public identity/session views | Profile/Jewel image and crop, nickname, theme, compact mode, motion, locked presentation and inactivity policies |
| Server administration | Verified contacts, signed admission requests, capabilities and durable owner state | Admission decisions, channel creation/replacement/retirement and existing ownership lifecycle workflows |
| Archives | Sealed local channel metadata and received history | Partial read-only channel archives; no complete backup or automatic attachment recovery claim |

Unsupported actions use explicit unavailable states rather than fixture data.
Key material, bearer tokens and MLS state remain in the native core. Reliable
OS-session-lock integration still requires per-platform implementation and packaged
validation; inactivity and suspend-gap policies use the existing native watchdog.

## Development-only controls

`--dev-unimplemented: #b7ff4a` and `.dev-unimplemented` are the single semantic treatment for visible controls whose intended operation is unavailable. The state changes text and icon foreground only, retains normal dark-surface backgrounds, sets the control inactive, exposes `aria-disabled`, and uses `Not implemented yet` as its tooltip. It must not be used for a temporarily busy real action or an authorization-denied action.

Currently marked groups include DM creation, replies/edits/deletes/reactions/attachments,
notification policy, role/capability mutation, server metadata editing, categories/topics,
invite issuance, moderation, audit filtering, integrations, device enrollment, recovery,
blocking and exports. Each action must connect to authoritative native/server APIs
before its unavailable marker is removed. No fixture identity, presence, role or
moderation result is presented as connected state.

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

History rendering is deliberately bounded to 500 messages with an explicit “show earlier” action. This prevents unbounded message count from immediately becoming the dominant DOM cost while preserving the timeline. Virtualization is not required for the current milestone; revisit it when route pagination exists or target-native profiling shows visible scroll/frame degradation. A real synchronized long-conversation WebView scroll trace was not available in the automated local environment, so target-native scrolling remains a validation gap rather than a claimed pass.

Known hotspots and follow-up work:

- The richer chassis and tree-shaken Lucide icons increased the production JS and CSS sizes shown above; the result remains a single modest application chunk.
- `App.svelte` requests a view refresh once per second after unlock while native synchronization also runs independently. Profile a connected idle client and replace polling with native events if it produces measurable wakeups.
- The 500-message window keeps initial work bounded but still grows to 1,000 when the user requests earlier history. Route-level pagination or virtualization should precede very large histories.
- Re-run memory, idle CPU and scroll-frame measurements on packaged Windows, Linux and macOS clients with a representative synchronized route before claiming cross-platform performance proof.

## Validation contract

Frontend formatting, Svelte diagnostics, lint, unit/security/performance tests and production build must pass. Rust formatting, Clippy and relevant client tests must pass while preserving the narrow Tauri security boundary. Browser rendering, native debug launch and packaged build are separate evidence; neither frontend compilation nor archive inspection alone proves target-native interaction or real-server messaging.


## Profile and membership polish — October 10, 2026

Messages and member lists resolve nicknames and pictures from native-verified signed
member contacts. Device IDs remain available for disambiguation; nicknames remain
self-declared presentation labels, not proofs of global identity. Profile edits
invalidate cached sessions so subsequent device-proof renewal publishes the updated
signed card. Offline local preference saves remain possible. Other clients refresh
the directory during synchronization and retain verified presentation cards sealed
in their vault. HTTPS image failures fall back to initials with an unavailable hint.

Locked-picker nickname hiding uses numbered labels and a neutral icon rather than
initials from the hidden name. Genericize hides pictures only on locked surfaces.
Neither option changes the unlocked nickname or the shared profile.

Encrypted channel details offer named server-device selection, with device suffixes
and explicit instructions to join the server and publish a KeyPackage. Actual MLS
addition still requires authorization and a usable key package. Public channels
explain admitted-member access and do not suggest an MLS invitation is required.

Leave server includes a separate confirmed local-only removal option, available
even while the server returns an HTTP failure. It does not claim remote membership
removal. Previously retained channel archives survive active cache removal.
