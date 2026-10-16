# Cords UI Design Contract

> **Status:** Living product and implementation design contract  
> **Primary UI:** Qt 6 Quick / QML, with a custom Cords style and no embedded WebView  
> **Platforms:** Windows, Linux, macOS; adaptable to Android and other supported form factors  
> **Application core:** Existing Rust client core, connected to Qt through a narrow, typed bridge  
> **Reviewed:** 2026-10-15  
> **Location:** Repository root, `UI-DESIGN.md`

## 0. How to use this document

This document defines **what Cords looks like, how its interface works, and how the native Qt client implements those decisions**. It is a standing contract for design, implementation, code review, and regression tests. Read it before making user-facing changes; update it when a conscious design decision changes.

- **MUST** is a release-blocking requirement for an applicable implemented feature.
- **SHOULD** is the default choice; deviations require an explicit reason in the decision log.
- **MAY** allows a deliberate option, not a mandatory feature.
- **NOT YET** identifies an intended capability; it MUST NOT appear as a working action.

**Precedence:** Protocol, cryptographic, server-authentication, MLS, and RBAC specifications determine what the client *may* do. This document governs what the user *sees and can attempt*. Never weaken security to imitate Discord. If a conflict appears, correct the interface and record the conflict. Follow ADR 0008 and the current role-policy specifications for Cords-specific semantics.

**Reality check:** A planned or written API is not evidence of a completed implementation. Every screen must be audited against the actual core/server before being marked functional.

**Definition of done:** A screen is complete only when its visible interactions work through real authorized operations, display truthful results, respond properly to loading/offline/error states, and pass keyboard, accessibility, and layout checks. A pretty QML scene with dead buttons is not finished.

---

## 1. Product thesis: familiar because familiar works

**Cords: Right on the wire.** Self-hosted communication should feel natural to people who know Discord, while protecting Cords' independent-server, account/device, and encryption model.

### 1.1 The reference is Discord's interaction grammar, not its trademarks

Default to these recognizable conventions unless Cords has a concrete reason to diverge:

1. **Server rail** at far left: visual server identities and obvious selected/unread states.
2. **Channel tree** beside it: categories, text channels, collapsible sections, and a predictable server menu.
3. **Conversation** as the dominant center: fixed channel header, messages in chronological reading order, composer at bottom.
4. **Right member rail**: member names, avatars, and role groupings; optional, not replaced by permanent diagnostics.
5. **Profile popouts** on clicking a name/avatar, with roles and contextual actions.
6. **Context menus** for common message/member/channel actions; **dedicated settings** for persistent management.
7. **Roles** as visible social labels and administrative capabilities, with separate member/role-management flows.
8. **Clear unread/muted/selected states**, instant-feeling navigation, and an unambiguous active conversation.

Do **not** copy Discord's logo, branded colors, proprietary assets, Nitro-style upsells, activity games, or unavailable voice/DM features. Do **not** copy every modern Discord redesign merely because it is newer.

### 1.2 What user feedback actually suggests

These are **qualitative signals, not representative polling**. Preserve the stable conventions described by beginner guides and repeatedly raised in user feedback:

| Repeated observation | Cords design response |
| --- | --- |
| Users recognize the left-to-right server → channel → chat → member organization. | Keep this four-region mental map; do not replace it with a novel dashboard. |
| Dense, legible conversation space is important; toolbars can consume too much of it. | Keep chat primary, controls compact, and panels independently resizable/collapsible. |
| People object to smaller, less distinguishable server icons and inconsistent alignment. | Favor recognizable icons; offer icon-size/density controls; align rails, header, composer, and account strip. |
| Fixed layout changes disrupt muscle memory. | Keep frequent controls stable; make optional customization additive and remember preferences. |
| Extra outlines and spacing can distract from messages. | Use subtle boundaries, disciplined spacing, and limited accent colors. |
| Some users welcome dark-theme options, adjustable lists, and calmer layouts. | Offer meaningful appearance/density settings without abandoning familiar navigation. |
| Hidden or clipped controls make even attractive layouts unusable. | Test minimum windows, display scaling, dialogs, and keyboard paths in packaged apps. |

**Research anchors (read as background, not binding API specifications):**

- [WIRED, How to Use Discord](https://www.wired.com/story/how-to-use-discord): describes the recognizable server, channel, chat, and member-list structure.
- [Discord community feedback, March 2025 desktop layout](https://support.discord.com/hc/en-us/community/posts/30941505914007-Horrible-new-discord-desktop-app-layout): complaints about icon size, alignment, composer proximity, and unnecessary bars.
- [Discord community feedback, refreshed desktop interface](https://support.discord.com/hc/en-us/community/posts/30927878600215-Refreshed-Desktop-GUI-Interface): reports legibility/density tradeoffs and appreciates resizable channel lists.
- [r/discordapp accessibility feedback](https://www.reddit.com/r/discordapp/comments/1jmfb2s/the_new_ui_presents_many_accessibility_issues/): clipping, unreachable controls, and server-icon sizing.
- [r/discordapp discussion of UI variants](https://www.reddit.com/r/discordapp/comments/1i97ddy): some positive responses to darker themes and changed placements.
- [Discord Members Page](https://support.discord.com/hc/en-us/articles/15946797617431-Members-Page) and [Discord Roles and Permissions](https://support.discord.com/hc/en-us/articles/214836687-Discord-Roles-and-Permissions): public-facing reference for familiar administration patterns.

**Decision rule:** When choosing between an established, understandable Discord-style interaction and a more inventive one, choose the established interaction unless tests or a Cords requirement support the change. Familiarity is a usability feature.

### 1.3 Non-goals

- Pixel-for-pixel Discord replication; identical CSS colors, shadows, or proprietary assets.
- A marketing landing page masquerading as a chat client.
- Exposing device generations, MLS epochs, key packages, or raw UUIDs in normal conversation.
- Conflating role labels with cryptographic group membership or presence.
- Inventing placeholder friends, DMs, voice calls, bios, activity statuses, attachments, or integrations.
- Using visual hiding or disabled buttons as security enforcement.
- Using the browser DOM, HTML/CSS, Electron, Tauri, or an embedded WebView to implement the target native client.

---

## 2. Native cross-platform UI technology contract

### 2.1 Chosen toolkit

**MUST:** The target native Cords client uses **Qt 6 Quick/QML** for its application shell, views, controls, scene graph, and interaction logic. Its design language is **Cords**, not the host operating system's default widget theme. Qt Widgets are not the main application UI, and no WebView is required to render chat/settings.

- Use Qt Quick Controls and **Qt Quick Templates / a custom `Cords` style**, with the cross-platform **Basic style** as a deliberate fallback where helpful. Do not customize Qt's native Windows/macOS styles; their rendering and metrics are platform-specific.
- Use QML for presentation, layout, animation, focus, and simple presentation state. Keep application protocol, authorization, key handling, MLS, persistence, and synchronization in the **Rust client core**.
- Prefer a narrowly scoped, typed **CXX-Qt / QObject bridge** for Rust–QML interoperation. The concrete bridge/API, build tooling, and version pins belong in a separate architecture decision record; do not introduce a second application-logic core in QML or C++.
- Use Qt's native desktop infrastructure for windows, accessibility, clipboard, file selection, menus/dialogs where appropriate, and notifications via a documented native integration. Platform hooks may differ; the Cords interaction semantics MUST remain consistent.
- The app is **native compiled with a custom-rendered UI**. This does **not** mean every control is a native OS widget. Do not market custom Qt Quick controls as literal Win32/Cocoa widgets.
- Before distribution, review applicable **Qt and bridge licensing, plugin/module licenses, and static/dynamic linking obligations**. Do not assume all Qt modules share identical terms.

### 2.2 Architecture boundary

```text
┌─────────────────────────────────────────────────────────────────┐
│ Qt 6 Quick / QML                                               │
│ Cords style, shell, message/list delegates, dialogs, profile  │
│ popouts, editor forms, focus/keyboard, responsive layouts      │
└──────────────────────────────┬──────────────────────────────────┘
                               │ typed models / commands / signals
┌──────────────────────────────▼──────────────────────────────────┐
│ Narrow Qt ↔ Rust integration                                    │
│ QObject models, immutable IDs, revisioned commands, async      │
│ results, errors, cancellation, platform event presentation     │
└──────────────────────────────┬──────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────┐
│ Existing Rust client core                                      │
│ Accounts, signed profiles, devices, RBAC views, MLS, sync,     │
│ message state, notifications, encrypted storage, media codecs  │
└──────────────────────────────┬──────────────────────────────────┘
                               │ authorized network protocol
                        Independent Cords servers
```

**Rules:**

- NEVER store private keys, authentication secrets, decrypted MLS state, or server-authoritative permission evaluators in QML properties or UI models.
- A view receives the **minimum verified presentation data** and authorized actions needed to render it.
- Long-running I/O, image decoding, crypto, and network operations MUST NOT block Qt's UI thread. Use queued signals/slots and cancellable asynchronous tasks; marshal view-model updates onto the correct Qt thread.
- Use stable account/server/channel/message IDs and revisioned changes, not QML delegate indices, as action targets.
- Server selection changes which conversation is **visible**, not which unlocked joined servers synchronize.
- An OS notification is emitted only after client-core classification of an authenticated, authorized, newly synchronized message.
- Do not duplicate effective RBAC evaluation or profile-conflict resolution in QML bindings.

### 2.3 QML module and component conventions

Use one coherent Cords component library, for example:

```text
Cords.UI /
  Theme /           Palette, Typography, Spacing, Motion, Density
  Controls /        Button, IconButton, TextField, Menu, Popover,
                    Dialog, SearchField, Toggle, RoleChip, Tooltip
  Navigation /      ServerRail, ChannelTree, ChannelHeader, RightRail
  Conversation /    MessageList, MessageRow, Composer, MessageMenu
  People /          MemberList, MemberRow, ProfilePopout, RolePicker
  Administration /  SettingsShell, RolesEditor, MembersPage,
                    ChannelEditor, ModerationPage, ServerOverview
  Media /           Avatar, ServerIcon, Banner, AnimatedImage
```

Names illustrate ownership; the repository may choose a different directory structure, but MUST keep components reusable and styling centralized. Prefer controls built on `QtQuick.Templates` or customizable Basic-based controls over repeated ad hoc `Rectangle + MouseArea` buttons.

- Theme primitives SHOULD be QML singletons or equivalent typed, testable objects; avoid unrelated hard-coded color strings in screens.
- QML files MUST expose stable accessible names, roles, and object identifiers for tests where appropriate.
- Complex, long lists SHOULD use Qt models (`QAbstractListModel` or equivalent appropriate bridge) and `ListView`/similar virtualized views; enable delegate reuse where beneficial and reset ephemeral state on reuse.
- Keep account/server-specific identifiers, permission decisions, and request lifetimes out of delegate-local state.
- Use asynchronous image providers/decoders for avatars, icons, and banners. A successful stored JPEG XL file does not prove that Qt renders its animation; build and test the actual decode-to-scene-graph path.
- Avoid runtime QML downloads from servers, dynamic untrusted scripts, or treating remote HTML as UI components.
- Do not treat Qt Labs Technology Preview styling components as the stability foundation without an explicit reviewed exception.

### 2.4 Cross-OS consistency and platform adaptation

**Windows, Linux, macOS:** same Cords layout vocabulary, component metrics, navigation positions, colors, and actions by default. Adapt only OS conventions such as title-bar integration, window management, system dialogs, shortcuts, notifications, accessibility backends, and packaging.

**Android/future mobile:** preserve the same server → channels → conversation → members mental model, but use drawers/stacked screens and touch-sized interactions. Never force a desktop four-column arrangement onto a phone. Android compatibility is an architectural target, not proof that an Android client already exists.

A platform is supported only after a packaged build passes functional, accessibility, visual, media, and notification validation on that OS. Qt portability alone does not constitute completed support.

---

## 3. The desktop shell: recognizable before anything else

### 3.1 Layout

```text
┌────────┬────────────────────────┬────────────────────────────────────────┬────────────────────────┐
│ SERVER │ SERVER + CHANNELS      │ #channel         Search  Members Info │ MEMBERS                │
│ RAIL   │ ────────────────────── │────────────────────────────────────────│                        │
│        │ Categories             │                                        │ Role headings          │
│ icons  │  # text-channels       │ Messages and timestamps                │   avatar + name        │
│        │  # another-channel     │ Replies, embeds when supported         │   avatar + name        │
│        │                        │                                        │                        │
│        │                        │                                        │                        │
│        │ Current account        │ Message composer                       │                        │
└────────┴────────────────────────┴────────────────────────────────────────┴────────────────────────┘
```

**Hierarchy:** conversation area has first claim on horizontal space; the rest are navigation and context. Keep a stable channel header and composer. Avoid unnecessary full-width top bars, oversized empty gutters, or competing bottom bars.

Suggested starting metrics (Qt logical pixels, not physical monitor pixels; tune with actual screenshots):

| Region | Initial size / rule |
| --- | --- |
| Server rail | 64–72 dp wide; icon size preference independent from global UI scale |
| Channel/navigation rail | ~240 dp preferred, user-resizable within sensible bounds |
| Member rail | ~240 dp preferred, toggleable; can be resized if practical |
| Conversation | flexible remainder; protect minimum readable width |
| Channel header | compact one-line row, consistent vertical alignment |
| Composer | anchored at bottom; multiline growth bounded without hiding history |

Do not use fixed widths that clip on small laptop windows or high scaling. Persist optional panel visibility and user-adjusted sizes per application/workspace as appropriate.

### 3.2 Server rail

- Show each joined server with its actual image/icon or a deterministic letter fallback; clear active and unread indicators. The server rail represents **real separately hosted Cords servers**, not Discord-style channels masquerading as servers.
- Clicking selects a server and restores the last selected eligible channel; double-clicking must not trigger surprising actions.
- Hover/focus reveals a helpful tooltip with the server's display name; URL/signing identity remain available through details.
- Joining/connecting to a server is a discoverable functional action. Do not expose a fake Home/Friends area until appropriate features exist.
- Provide **compact/comfortable density** and server-icon sizing. Keep icons large enough to distinguish without zooming the entire application.
- Context menu MAY offer supported rename/display, disconnect/archive/drop, and server settings actions, distinguishing destructive and security-sensitive operations.
- Switching servers MUST NOT suspend background synchronization of other eligible joined servers.

### 3.3 Channel tree

- Server name/header at top opens a practical menu. Below: ordered categories, collapsible sections, and text channels with familiar `#` visual cues.
- Indicate selected channel, unread content, muted state, inaccessible channels (only if visibility policy allows), and temporarily read-only states distinctly. No invented voice-channel controls.
- Channel row click selects channel; context menu gives only supported actions. Creation/edit controls visible to authorized accounts.
- Keep expand/collapse state across navigation. Channel list scrolls independently from the conversation.
- Do not turn a channel permission change into a silent MLS enrollment promise.
- Bottom account strip shows the current account/avatar and access to account/settings/lock; keep it aligned with neighboring pane and composer.

### 3.4 Conversation workspace

- Header identifies selected channel and provides **member-list toggle**, **Info**, and real search/settings actions where supported. Header controls keep stable positions between channel changes.
- Messages read top-to-bottom; newest near bottom, with thoughtful auto-scroll only while at/near the bottom. Preserve reading position when history arrives above or new messages arrive below.
- Show author avatar, account display name, time, grouping of consecutive messages by same author, role-colored names where readable, and clear mention/reply styling only when backed by data.
- Clicking a sender opens the same profile popout as clicking a member row. Right-click/long-press opens contextual message actions that the user can actually perform.
- Contextual message actions may include copy, reply, own deletion, authorized moderation deletion, and other **implemented** operations. No fictional reactions/edit/threads/attachments.
- Composer stays visible, supports multiline input, keyboard send behavior and user preference where applicable, and makes permission/connection/MLS restrictions intelligible. A disabled composer MUST state whether the cause is RBAC, timeout, device approval, connection, lock, or pending cryptographic transition.
- Initial history loads without flooding the user with messages marked as new notifications. Visible chat updates and unread counters must not depend on native notification permission.

### 3.5 The default right rail is people, not diagnostics

- Wide-window default: **public admitted server member list**, grouped once per member under highest eligible separately displayed role, with group counts.
- Clicking a row opens a profile popout; context menu shows authorized actions. Show real account avatars and names, not device contact UUIDs.
- The toggle should be in the familiar channel header. Hide rail without disturbing current message/history state.
- **Info** opens channel confidentiality, server signing identity, retention, enrollment, and other technical conversation details as a mutually exclusive right-side surface. Do not stack Members + Info + Settings simultaneously.
- At narrow widths, right-side surfaces become a dismissible overlay/drawer; they do not shrink the chat to unusability.
- This is a **server membership** list, not proof of access to the selected encrypted channel. Never leak private rosters or enrollment state through the public rail.
- Presence is NOT YET. Do not color avatars green or claim online/offline from historical connections.

### 3.6 Density, personalization, and navigation stability

- Provide at least a default **compact/comfortable** density setting (or one well-chosen default with equivalent layout controls), adjustable server icon size, text scale, and motion preference.
- Do not require users to relearn ordinary server/channel/member navigation when themes change.
- Preserve cursor, selection, scroll, and sensible focus when changing channels or closing popouts.
- Do not move high-frequency chat actions into a novel secondary menu solely to save a few pixels.
- Align top header, left account strip, message composer, sidebar separators, and status text; avoid random offsets and uneven padding.

---

## 4. Cords design language

### 4.1 Visual principles

**Quiet structure, strong content, recognizable people.** Chat is not a dashboard. Chrome should recede, messages should be readable, and the user should always know what is clickable.

- Default dark appearance with optional lighter/darker themes where implemented; maintain sufficient contrast instead of optimizing for the blackest possible background.
- Differentiate server rail, channel rail, conversation, and member rail with subtle, purposeful surfaces. Avoid a stack of glowing borders.
- One restrained Cords accent for primary actions/selection. **Do not reuse bright green indiscriminately** for links, statuses, buttons, and labels.
- Role color affects identity display, not administrative capability. Keep colored text legible regardless of chosen role hue.
- Destructive actions clearly differ from ordinary Save/Cancel. Loading/pending and success/error must not be colors alone.
- Prefer compact desktop spacing. A change must improve legibility or discoverability to justify consuming conversation width/height.
- Use the OS's installed accessible font stack or a properly licensed bundled font; do not rely on remote fonts.

### 4.2 Central theme tokens

Define via `CordsTheme` (QML singleton or equivalent), with clear semantic aliases:

| Token family | Examples |
| --- | --- |
| Surfaces | `window`, `serverRail`, `navigation`, `conversation`, `memberRail`, `popover`, `dialog`, `elevated` |
| Text | `primary`, `secondary`, `muted`, `inverse`, `link`, `onAccent` |
| Interactions | `accent`, `hover`, `pressed`, `selected`, `focusRing`, `disabled` |
| Status | `success`, `warning`, `danger`, `pending`, `unread`, `offline` |
| Geometry | spacing scale 4/8/12/16/24; radii; control heights; row/icon sizes |
| Typography | page heading, section heading, body, metadata, code/ID |
| Motion | hover, popout, panel, loading; zero/reduced alternatives |
| Density | compact, comfortable; server-icon size separately controllable |

Do not hardcode unrelated one-off dimensions/colors in every view. Use Qt logical pixels, layouts, implicit sizing, and constraints. Color values belong in one reviewed palette, not scattered QML snippets.

### 4.3 Required reusable native controls

`CordsButton`, `CordsIconButton`, `CordsTextField`, `CordsSearchField`, `CordsCheckbox/Toggle`, `CordsSelect`, `CordsMenu`, `CordsPopover`, `CordsDialog`, `CordsToast/InlineError`, `CordsTabs`, `CordsAvatar`, `CordsRoleChip`, `CordsMemberRow`, `CordsSettingsRow`, `CordsEmptyState`, and standardized progress/error views.

Every reusable control MUST provide hover, focus, pressed, checked/selected, disabled, busy, and error semantics where relevant. Do not substitute a colored `Text` element for an actionable `Button`.

### 4.4 Media presentation

- Account avatars derive from the verified **canonical account-level signed profile**. Multiple devices must not create multiple public avatar identities.
- Server icon and banner are **server presentation assets** with distinct storage/authorization; presentation edits never change server URL, signing key, or trust pin.
- Support the separately specified bounded static and animated JPEG XL workflow. Use native decoding and Qt rendering rather than WebView/WASM as the target playback mechanism; keep decoder, memory, and animation timing tests.
- Preserve frame timing, compositing, alpha, loops, crop/position, and reduced-motion preferences. Stop decoding/playing hidden or recycled delegates.
- Do not assume `Image` can natively play every JXL animation. Provide an implemented decoder/provider/rendering path and validate on each platform.
- Reuse shared media processing, but keep signed account-profile publication distinct from server-managed icon/banner uploads.

---

## 5. Universal interaction contract: no decorative buttons

### 5.1 Every affordance has a real contract

Every visibly actionable item MUST have:

1. **Real behavior:** a connected command or local-only action, not a blank QML handler.
2. **Actual authority:** capabilities from the server/client core; never a QML permission guess.
3. **Predictable target:** stable resource ID, correct account/server/channel context.
4. **Honest feedback:** pending, success, validation, conflict, failure, cancellation.
5. **Pointer, keyboard, and accessibility support:** the same operation through each path.
6. **Verification:** tests or reproducible, recorded interaction with actual services.

When unavailable: **finish and connect** an in-scope feature, **render read-only**, **omit**, or **disable with a useful reason** when discovery itself matters. Never turn the UI into a museum of inert green captions.

### 5.2 Settings and mutation discipline

- A server fingerprint or immutable channel confidentiality mode is read-only selectable text, not a disabled edit field pretending to be configurable.
- Save requires dirty valid data; preserve edits after recoverable failure. Cancellation and navigation protect unsaved work.
- Use revision-aware updates, conflict resolution, and idempotency for uncertain network outcomes. A disappearing spinner is not success.
- Confirm impactful moderation, device revocation, identity burn, server drop, and channel succession with clear target/consequences.
- An action submitted through a menu, role chip, or settings page MUST share the same client-core operation and response semantics.
- Do not invent client-only authorization just to make an inaccessible page appear functional.

### 5.3 Explicit states

Distinguish: `Loading`, `Empty`, `No permission`, `Disconnected`, `Locked`, `Pending cryptographic transition`, `Unsupported server version`, `Invalid input`, `Conflict`, `Failed`, and `Succeeded` where relevant.

Make a sensible recovery action available where possible. Error text should describe what the user can do next, without exposing secrets or treating every failure as an enigmatic “Something went wrong.”

### 5.4 UI action inventory

For each screen under rework, maintain a checkpoint inventory:

| Element | User expectation | Client-core/server operation | Authorization | Error/empty behavior | Verified |
| --- | --- | --- | --- | --- | --- |
| Example: Change banner | Select, crop, publish new banner | Authorized asset upload + metadata revision | Server manage permission | Cancel/invalid file/conflict | Evidence link |

An element is **not finished** if it only emits `clicked()` or opens a dialog with an inert Save button.

---

## 6. Public members, profiles, and account identity

### 6.1 Account-first membership

One public member row represents **one admitted account**. Device contacts are not the source of truth for membership. Public rows may contain verified display name, avatar, visible role color/group, and explicitly public profile information. Do not leak account secrets, raw device identifiers, join requests, restricted channel rosters, or moderation notes.

### 6.2 Member list

- Group once per account by highest *separately displayed* eligible role, then a general Members group; deterministic ties.
- Show counts and role headings without implying a role confers every channel's keys.
- Member row opens the reusable profile popout. Moderation/role assignment context actions appear only when authorized.
- Paginate/window large lists; preserve keyboard selection and clear accessible names.
- No online/offline dots until a genuine presence system is implemented.

### 6.3 Canonical profile popout

- Open consistently from messages, member rows, and administration listings.
- Show verified account display name, avatar, roles, and real supported actions. Details may reveal suitably authorized account identity/device diagnostic information.
- Account presentation comes from one signed **account-level profile**; authorized devices may propose signed updates. Servers cache/sync records but cannot fabricate account-authored profile changes.
- Preserve signed causal revisions and conflicts. Distinct-field concurrent changes may merge through an authorized signed merge; conflicting same-field changes require review. Do not pick winners by timestamps, server arrival, or device ID.
- Previously unseen offline edits from revoked devices require an explicit secure acceptance policy; historical signatures alone do not prove pre-revocation authorship.
- Independently hosted servers need not immediately know the same latest revision; clients converge after receiving the same verified history. No implicit federation.
- Role assignments remain **server-scoped**; the account's canonical profile is not a global role definition.
- No fake DM, activity, or bio controls.

### 6.4 Account versus device administration

The Members page is a searchable, role-filtered account directory. Requests live in a separate review tab. A member detail page has an expandable Devices section for device certification, approval, revocation, generations, and diagnostic IDs. Actions explicitly identify account-wide versus device-specific consequences.

---

## 7. Roles and permissions: Discord-familiar, Cords-authoritative

### 7.1 Role-management surfaces

Use familiar workflow options backed by the **same mutation path**:

- In a member's profile: colored role chips + searchable authorized add/remove picker.
- In Members administration: select an account; inspect/change roles.
- In Roles administration: select a role; manage its assigned accounts.

### 7.2 Two-pane role editor

Role hierarchy on left; on right tabs/sections for **Display**, **Permissions**, and **Members**. Drag reorder where safe and provide keyboard up/down controls. Persist numeric priorities in backend without forcing admins to type them. Show role color/description/member grouping and appropriate live preview. Keep the implicit default role and owner identity visibly distinct.

### 7.3 Security semantics

Do not silently substitute Discord permission resolution for Cords':

- `$default` implicit for admitted accounts.
- Explicit `$admin` → `$moderator` inheritance where documented; hierarchy order alone is **not** inheritance.
- Owner authority is independent of roles.
- Channel/category override resolution follows the actual Cords evaluator, including its specified deny precedence and visibility boundary.
- Assignment and role-definition editing are distinct permissions; enforce hierarchy and prevent self-escalation.
- Server-role grants and channel/category `Inherit / Allow / Deny` controls are represented according to **real policy semantics**, not superficial uniform toggles.
- An effective-capabilities view may explain permissions but must not become a second authoritative evaluator in QML.

### 7.4 Incomplete role states

If RBAC activation is genuinely incomplete, do not claim saved presentation roles actively enforce permissions. Conversely, once validated enforcement ships, remove obsolete preparatory warnings. Visible role color isn't proof of MLS enrollment. The server decides every privileged mutation.

---

## 8. Server administration and settings

### 8.1 Layout

Use a clear settings shell: left section navigation, one primary scroll region, predictable header, Save/Cancel or per-section autosave with honest persistence. Avoid nested full-height scrolling panes and overlapping dialogs. Keep dense technical information in Advanced, not as the opening screen.

Sections MAY include Overview, Members, Requests, Roles, Channels/Categories, Invites, Moderation, Audit, Notifications, Advanced, and Ownership, but ONLY when backed by functioning content. Unsupported sections do not get decorative navigation entries.

### 8.2 Overview and presentation

- **Display name**, **description**, **icon**, **banner**: editable with full picker, crop/preview, upload, validate, persist, remove, and authoritative refresh if implemented and authorized.
- **Server URL/origin, signing key, fingerprint**: stable identity data; read-only, selectable/copyable, with trust verification available.
- Presentation metadata suitable for invitation previews MAY be publicly readable under bounded server policy; this does not expose membership/private policies.
- Asset upload storage is independent from encrypted chat-message attachments; restrict sizes/frames/dimensions and handle replacement cleanup.
- Show join policy as intelligible human text; only enable editing when the server actually supports changing it.
- After Save, update server rail/header and other relevant surfaces. No silent reversion on restart.

### 8.3 Channel management

Separate ordinary edits (name, category, supported description, policy) from membership/MLS state and irreversible lifecycle operations. A channel's confidentiality mode is immutable for its identity. Replacement or authenticated succession to a new channel is NOT a normal Save on a dropdown.

An encrypted channel pending removal commits is accurately shown as temporarily unable to accept new application messages; unrelated channels keep functioning. If no eligible committer remains, a permissioned, explicit succession workflow can create a *new* group identity without pretending to recover its old cryptographic state/history. Local archives may differ by device.

### 8.4 Moderation and deletion

Kick, ban/unban, timeout/removal, and own-message/moderator deletion require the right authority and precise targets. Show sanctions and pending MLS consequences separately. Durable tombstones make removed messages stay removed after reconnect, but the UI must never promise remote erasure of screenshots, exports, or previously retained plaintext.

### 8.5 Advanced/operator details

Security details are available but progressively disclosed: account identity, device approval, server pin, MLS enrollment, retention, event revisions, diagnostics, and migration/recovery state. User-facing language describes practical consequences first, exact protocol terms second.

---

## 9. Synchronization, unread state, and local notifications

### 9.1 Client operation

All eligible joined servers of an **unlocked** account continue synchronizing while the app runs. Selected server/channel changes only the visible workspace. Use independent session/cursor/retry state, bounded fairness/concurrency, WebSocket wakeups and incremental fetch. Offline or blocked servers must not freeze healthy servers or the Qt event loop.

### 9.2 Native notifications

Notifications are decided in the Rust client core **after authorized retrieval and, for encrypted content, authenticated decryption**. The native Qt/platform layer only presents approved candidates and handles safe clicks/navigation. Do not build a plaintext server-side notification router.

Offer **All Messages** (default), **Mentions Only**, **Nothing**, server/channel overrides, mute, app Do Not Disturb, and hidden previews by default. Suppress own-account messages across devices, currently visible focused chat, historical first-sync, duplicates, deleted/inaccessible messages, and locked content. Read/unread state is independent of notification deduplication state. Don't promise alerts when the app is fully stopped unless a separate supported background process exists.

### 9.3 Role mentions

Structured direct-account, stable role ID, and everyone mentions are authenticated with message content; in encrypted channels they are decrypted locally. Role membership helps decide whether a **role mention targets this account**. Roles do NOT create a separate permission to receive ordinary notifications. Don't imply server enforcement of encrypted mention contents without a protocol that actually provides it.

---

## 10. Native Qt interaction, accessibility, and performance

### 10.1 Focus, keyboard, and assistive technology

- Use proper Qt Quick Controls or explicit `Accessible.role`, `Accessible.name`, `Accessible.description`, and accessible actions for custom items. Mouse click handlers alone do not make a control accessible.
- Full keyboard reachability, visible focus ring, coherent Tab/Shift-Tab order, and correct Enter/Space activation are required.
- `Escape` dismisses topmost cancellable popout/dialog; destructive confirmations are not implicitly accepted. Restore focus to the invoking control.
- Context menus appear within the current window and offer keyboard access. Avoid modal stacks and invisible offscreen controls.
- Text scales appropriately; screen reader labels must identify both action and target where necessary.
- Respect reduced motion and offer an animation disable preference; don't use animation or color as the only signal.

### 10.2 Qt-specific list and rendering discipline

- Use `ListView` and model-backed delegates for long messages/members/channels; avoid constructing thousands of permanent QML objects.
- `reuseItems` can recycle delegates. Always reset local ephemeral state when reused and pause animated frames while pooled/offscreen.
- Handle variable-height messages carefully; history prepend, scrolling to an event, and reply/menu anchors must not jump unpredictably.
- Decode/process media off-thread with bounded memory and cancellation. Qt's scene graph and model updates obey their thread requirements.
- Test the actual image renderer with static and animated JXL; static thumbnails should not force every offscreen item into animation playback.
- Do not block the UI waiting for an unresponsive server, slow database, animation decode, or key operation.

### 10.3 Responsive rules

- **Wide desktop:** four areas by default; member rail optional.
- **Narrow desktop:** member/Info rail overlays; channel rail resizable or collapsible while conversation stays usable.
- **Smallest supported window:** no clipping of composer, settings Save/Cancel, role menus, or destructive-confirmation buttons. A measured minimum window size is acceptable if justified and documented.
- **High-DPI and large text:** layout adapts; icons and click targets remain usable. Treat user font scaling separately from physical-pixel density.
- **Mobile future:** stacked screens/drawers, not compressed desktop columns; preserve concepts rather than fixed coordinates.

### 10.4 Validation toolkit

- Run Qt/QML static analysis (including `qmllint`/`qmlformat` where applicable), compile/resource checks, and **Qt Quick Test** for controls, focus, popouts, and state changes.
- Test typed Rust/Qt bridge commands, data model lifecycle, threading, error propagation, cancellation, and account switching.
- Use actual server integration fixtures for persisted edits, RBAC denial, MLS state, and background synchronization; mock-only tests are not release proof.
- Capture screenshots and interaction recordings from packaged native **Windows, Linux, and macOS** builds at normal/narrow/high-DPI/text-scale settings.
- Validate native desktop notifications, image animation, file dialogs, OS integration, and accessibility separately per OS. A successful cross-compile isn't a runtime result.
- Review custom Qt Quick style upgrade compatibility, platform graphics backends, packaging dependencies, and Qt licensing at release gates.

---

## 11. Acceptance scenarios

A screen or checkpoint is not complete until the applicable scenario works with real services (or a clearly documented local-only setting) and has evidence:

1. **Open and navigate:** launch native Qt client, choose server and channel, read/send permitted messages, and maintain expected scroll/focus.
2. **Familiar shell:** wide layout shows server rail, channel tree, chat, and public member list; toggling Members/Info works; narrow layout keeps composer accessible.
3. **Account identity:** one account with multiple devices is shown once; profile popout is consistent everywhere; verified avatar/name updates propagate after synchronization.
4. **Roles:** assign via profile, member directory, and role view; role grouping/colors update; unauthorized API calls still fail; ordering is keyboard-accessible.
5. **Member requests:** review/approve/reject without leaking pending accounts into public sidebar.
6. **Moderation:** kick, ban/unban, timeout, own/moderator deletion show proper feedback; tombstones survive reconnect; MLS consequences are not misrepresented.
7. **Encrypted revocation:** affected channel fences when needed, unrelated channel stays usable; recovery/succession clearly distinguishes new cryptographic identity.
8. **Server presentation:** rename, description, icon, and banner edits work (when supported) via real validation and durable storage; cosmetic changes do not change trust identity.
9. **Native media:** static and animated profile/server imagery decodes, crops, plays, pauses, and respects reduced motion on each supported OS.
10. **Background sync:** server B receives new content while server A is selected; an unavailable server C doesn't stall either.
11. **Notifications:** app running/unlocked receives a new message and classifies it correctly; no historical, duplicate, own-account, visible-chat, or forbidden previews.
12. **Keyboard and accessibility:** ordinary chat, member profile, role picker, menu, settings Save/Cancel, and error recovery are operable without a pointer.
13. **Window variants:** normal desktop, narrow window, and large-font/high-DPI screenshots have no clipped composer, inaccessible menus, or inconsistent rail alignment.
14. **No false affordances:** every apparent control either works, clearly communicates an actionable limitation, is read-only, or has been removed.
15. **Architecture:** native Qt scene renders the UI without an embedded browser; Rust client core remains the source of protocol/security state.

---

## 12. Workflow for Codex and contributors

1. **Read this file, relevant ADRs/security contracts, and actual code.** Do not assume planned milestones are implemented.
2. **Audit every visible action** in the targeted screen. Mark working, broken-but-supported, read-only, or out of scope.
3. **Confirm target architecture:** Qt 6 Quick/QML, Cords custom style, typed Rust bridge. Do not implement new frontend features in Tauri/Svelte when building the replacement native client.
4. **Deliver vertical slices**, not screenshots: QML control → typed client operation → authoritative result → refreshed shared model → failure handling → tests.
5. **Preserve existing frontend while migrating** until the native implementation has a validated replacement. Do not remove working features simply to claim WebView elimination early.
6. **Keep each checkpoint independently testable:** design tokens/shell; core bridge; chat; public members/profiles; administration/RBAC; media; notifications; visual/accessibility refinement. Dependencies may alter order.
7. **Record evidence** from actual native runs. Separate completed, partial, and unverified behavior.
8. **Update this design contract** when a real product decision changes, not to rationalize an accidental regression.

### 12.1 Running implementation ledger

| Surface | Contract | Actual implementation | Evidence / next blocker |
| --- | --- | --- | --- |
| Qt shell and Cords style | Specified | **Partial native implementation** | MSVC/Qt 6.10.3 executable; real-service Windows captures at 1180x760, 680x520 and 150% scaling |
| Server/channel rail | Specified | **Partial** | Core trust/selection, channel lifecycle and persisted notification mute wired; categories and unread remain incomplete |
| Conversation/composer | Specified | **Partial** | Independent TLS/PostgreSQL send/receive/restart passes; bounded history loading/anchors remain |
| Public member sidebar | Specified | **Partial** | Real admitted directory aggregates accounts; automatic refresh, avatars and role grouping remain |
| Canonical profile/popouts | Specified | **Partial** | Signed name edit and propagation, native animated JXL verified; image editor and conflict resolution remain |
| Members/requests/devices | Specified | **Partial** | Typed core commands and QML views; complete interaction/denial acceptance remains |
| Roles and channel policy | Specified | **Partial** | Preparatory signed catalog/assignment API ported; active RBAC stays separately gated at schema 14 |
| Moderation and lifecycle | Specified | **Partial** | Supported departure/archive/burn and channel succession wired; moderation/deletion APIs remain gaps |
| Server presentation/media | Specified | **Incomplete** | Native JXL renderer tested independently; server asset publication/editing remains unavailable |
| Background sync/notifications | Specified | **Partial** | Real TLS classification, hidden previews, mute persistence and Rust click targets pass; OS presentation unavailable/suppressed in this run, unavailable-third-server and other OS adapters remain |
| Accessibility and platform packaging | Specified | **Partial Windows evidence** | Installed-tree startup without SDK PATH/source migrations; native priority lock controller test passes; Linux/macOS runtime pending |

Evidence is dated 2026-10-16; the detailed ledger is
`docs/plans/qt-desktop-migration.md`. “Partial” does not satisfy the completion
gate. Windows event tests inject a native session-lock event against a stalled
TLS peer; actual workstation lock/suspend and clean-machine deployment remain
separate checks. Tauri remains temporarily for parity comparison and has not
passed its removal gate.

### 12.2 Design decision log

Record each change as `date — decision — rationale — affected surfaces`.

- **2026-10-15 — Initial Cords design contract:** Familiar Discord-inspired navigation; account-centric members and roles; technical conversation info on demand; honest UI controls; client-local notifications; server-authoritative RBAC/MLS.
- **2026-10-15 — Native Qt direction:** Qt 6 Quick/QML with a custom Cords style replaces Tauri/Svelte as the **target** interface framework; Rust remains authoritative for core behavior. Existing frontend can remain during validated transition.
- **2026-10-15 — Discord feedback synthesis:** Preserve the familiar four-region desktop model and contextual interactions; prioritize readable density, distinct server icons, optional panel sizing, stable control positions, and avoidance of visual clutter. Feedback is qualitative and includes both criticism and praise of redesigns.

---

## 13. Reference implementation documentation

Use these as **technical references**, not as instructions to make Cords visually imitate Qt's stock controls:

- [Qt Quick](https://doc.qt.io/qt-6/qtquick-index.html)
- [Customizing Qt Quick Controls](https://doc.qt.io/qt-6/qtquickcontrols-customize.html)
- [Styling Qt Quick Controls](https://doc.qt.io/qt-6/qtquickcontrols-styles.html)
- [Qt Quick best practices](https://doc.qt.io/qt-6/qtquick-bestpractices.html)
- [Qt Quick ListView delegate reuse](https://doc.qt.io/qt-6/qml-qtquick-listview.html)
- [Accessibility for Qt Quick](https://doc.qt.io/qt-6/accessible-qtquick.html)
- [Qt Quick Test](https://doc.qt.io/qt-6/qtquicktest-index.html)
- [Asynchronous image providers](https://doc.qt.io/qt-6/qquickasyncimageprovider.html)

**Final principle:** Make Cords' ordinary chat interactions boringly predictable. The engineering behind it can be sophisticated; the interface should not make people pay for that sophistication in clicks, clutter, or guesswork.
