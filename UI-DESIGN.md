# Cords UI Design Contract

> **Status:** Living design specification  
> **Scope:** Desktop Cords client (Tauri/Svelte), shared client-core presentation contracts, and user-facing server administration  
> **Last reviewed:** 2026-10-15  
> **Location:** Repository root, `UI-DESIGN.md`

## How to use this document

This file defines **how Cords should look, behave, and explain itself**. It is a standing reference for every UI change, not a one-time redesign brief. Read it before planning UI work, use it when reviewing implementations, and update it when a deliberate product decision changes.

- **MUST**: required for a feature to be considered complete.
- **SHOULD**: expected by default; departures need a documented reason.
- **MAY**: optional, provided it does not conflict with required behavior.
- **NOT YET**: a designed capability that must not be misrepresented as implemented.

The repository's security specifications, protocol contracts, and architecture decision records govern what a client **may actually do**. This file governs how those capabilities are exposed. If a proposed UI interaction conflicts with server authority, MLS, identity, or privacy rules, **change the UI rather than weakening the security model**. In particular, keep this document aligned with the RBAC design and ADR 0008.

**Definition of done:** A screen is not complete because it renders or resembles Discord. It is complete when every visible action has an intentional, authorized, testable behavior; every displayed state comes from a trustworthy source; and the interface remains understandable in loading, failure, restricted, empty, and offline states.

---

## 1. Product direction

**Cords: Right on the wire.** A self-hosted communication client that feels familiar to a Discord user without hiding the fact that servers are independently operated and encrypted participation has real security boundaries.

### Goals

1. **Recognizable chat-first navigation.** Server rail, channel list, conversation, and an optional member list should require almost no explanation.
2. **People before implementation details.** Show accounts, display names, avatars, roles, and interactions by default. Reveal device IDs, fingerprints, epochs, and diagnostics only where useful.
3. **Progressive disclosure.** Ordinary members see chat and people. Moderators see contextual management. Operators can intentionally reach advanced security and deployment controls.
4. **Complete interaction contracts.** A control that looks actionable MUST work, explain why it is unavailable, or not be presented as a control.
5. **Native-feeling, consistent desktop UI.** Clear hierarchy, compact information density, keyboard support, predictable windows/dialogs, and restrained motion.
6. **Truthful security language.** Never imply permission enforcement, presence, message recall, MLS membership, or notification delivery that has not actually happened.
7. **One coherent UI system.** Shared components, tokens, state handling, and vocabulary rather than individually designed administration pages.

### Non-goals

- Pixel-perfect copying of Discord, its branding, its commercial features, or its decorative clutter.
- Exposing every backend capability on the main chat screen.
- Inventing client-side authorization or treating hidden buttons as a security boundary.
- Requiring ordinary users to understand MLS, KeyPackages, role policy revisions, or device generations to send a message.
- Displaying fake presence indicators, placeholder data as real data, or future features as working controls.

### The governing question

> **What would a regular member reasonably expect to happen when they click this?**
>
> If the answer is different from what the application can actually do, the design is wrong.

---

## 2. Problems this redesign must eliminate

The current UI has useful underlying functionality, but its visual and interaction model does not consistently distinguish working actions from placeholders. The October 2026 screenshots illustrate these failure modes:

- Bright green text appears across buttons, menus, and settings without a consistent semantic meaning. Some prominent controls appear unresponsive. Users cannot tell *action*, *highlight*, *status*, and *unavailable* apart.
- The **Members** screen describes public *device contacts* rather than a real admitted-account directory. Device generations and identifiers are more prominent than the people they represent.
- **Conversation details** permanently occupies the right sidebar while ordinary users have no familiar public member list.
- Server settings combine actual data, read-only backend omissions (for example, “not exposed by the server API”), and apparently editable fields without a clear distinction.
- **Change icon**, **Change banner**, **Save changes**, **Invite member**, and other prominent actions must be validated end to end, not merely displayed in attractive styling.
- A large settings dialog contains competing scroll regions and dense technical forms. It feels like a configuration inspector rather than a navigable administration experience.
- Channel settings mix everyday edits with irreversible cryptographic channel replacement. These need visibly separate workflows.
- Some pages communicate implementation gaps more prominently than the user task. Limitations should be honest and contextual, not the dominant design language.

These are **observed UX issues**, not claims that every named backend operation is missing. Audit the real code and runtime behavior before deciding whether to connect, replace, or remove each control.

---

## 3. Information architecture

### 3.1 Chat shell

On a comfortably wide desktop window, use four conceptual regions:

```text
┌──────────┬──────────────────────┬────────────────────────────────┬─────────────────────┐
│ Servers  │ Server / channels    │ Conversation                   │ Members (toggle)    │
│          │                      │ Channel header                 │                     │
│          │ Categories           │                                │ Role groups         │
│          │ Channels             │ Messages                       │ Account rows        │
│          │                      │                                │                     │
│          │ Account / settings   │ Composer                       │                     │
└──────────┴──────────────────────┴────────────────────────────────┴─────────────────────┘
```

- **Server rail:** compact server icons, an obvious active state, add/connect server entry, and accessible labels.
- **Channel navigation:** current server name and menu, categories, channels, selected-channel state, and the current account/device entry in a consistent location.
- **Conversation:** channel header, content/history, message actions, and composer. This is the primary workspace and MUST retain enough width to be usable.
- **Right rail by default:** the **member list**, not permanent technical conversation details.

Suggested initial sizing, to be tuned against real application windows: server rail ~56–72 px; channel rail ~220–280 px; member rail ~220–260 px; conversation takes remaining space. Do not make these hard-coded minimums that break small windows.

### 3.2 Right-side surfaces

The channel header MUST distinguish:

- **Members:** show/hide the admitted-account sidebar. Its toggle state should be predictable across channel changes.
- **Info / Conversation details:** inspect confidentiality, server identity, key enrollment, retention, and advanced channel information on demand.
- **Channel settings:** edit channel configuration if authorized. This is distinct from informational details.

Info may replace the member rail or open an appropriate panel/dialog, but MUST provide a clear way back. Do not stack confusing simultaneous right panels. On narrow windows, use a dismissible overlay/drawer instead of permanently squeezing the conversation.

### 3.3 Server administration

Organize by the user's job, not backend endpoint names:

| Area | Purpose |
| --- | --- |
| Overview | Server identity, presentation, join policy, supported defaults |
| Members | Account directory, member roles, moderation, and requests |
| Roles | Role hierarchy, appearance, permissions, and assigned members |
| Channels | Categories, channels, access policies, and channel lifecycle |
| Invites | Generate/manage supported invitation flows |
| Moderation | Moderation tools and active sanctions |
| Audit log | Authorized view of meaningful security and management changes |
| Notifications | Server-level notification defaults/settings **only where applicable**; do not conflate with role permissions |
| Integrations | Only implemented integrations |
| Advanced | Technical configuration and diagnostics |
| Ownership | Independently verified owner authority and supported succession workflows |

**Important:** a navigation item MUST open a real, usable view. If an area does not exist yet, omit it from the production navigation or present an explicitly informational, noninteractive roadmap entry in a development-only context. Do not ship a convincing empty settings page.

A sizable settings workspace is appropriate. Prefer one stable navigation column and **one primary scrolling content region**; avoid nested scrollbars where possible. Preserve tab/section selection, focus, and unsaved edits predictably.

---

## 4. Visual system

### 4.1 Personality and hierarchy

Cords should be understated, technical where necessary, but welcoming in everyday use. Use Discord's **interaction familiarity**, not its brand palette or monetization surfaces.

- Dark theme may remain the default; define tokens that can support light theme later.
- Use neutral backgrounds with distinct but subtle surface elevation for sidebar, conversation, popovers, and dialogs.
- Text should be readable without relying on fluorescent accents.
- Reserve strong emphasis for selected navigation, the primary action of the current view, important alerts, and destructive confirmations.
- Role colors express **member identity**, not button affordances or authorization success.
- Prefer legible labels such as **Join policy: Moderator approval** over raw values like `moderator_approval` except in an advanced/debug view.

### 4.2 Color semantics

| Meaning | Styling rule |
| --- | --- |
| Primary actionable control | Consistent Cords primary accent, filled or clearly outlined |
| Secondary action | Neutral surface/border with evident hover and focus |
| Link | Consistent link appearance and underline/focus treatment where needed |
| Success/connected | Reserved success color plus text/icon; not a universal button color |
| Warning/pending | Reserved caution color with explanatory wording |
| Destructive | Clear danger treatment and confirmation when needed |
| Disabled/unavailable | Muted, *not* deceptively clickable; add a reason if relevant |
| Role identity | Per-role display color, constrained for readability |

**Stop using bright green as a generic signifier of interactivity.** A green label is not an interaction contract. Do not use color alone to communicate meaning.

### 4.3 Type, spacing, and components

- Use a consistent type scale for page heading, section heading, body, metadata, and technical monospace IDs.
- Follow a restrained spacing scale (for example 4/8/12/16/24 px); consistent row density matters more than oversized panels.
- Use reusable buttons, icon buttons, inputs, selects, search fields, tabs, dialogs, menus, status indicators, member rows, role chips, and empty-state patterns.
- Keep technical identifiers selectable/copyable and appropriately truncated with a full-value reveal.
- Provide hover, active, selected, focus, disabled, loading, error, and success visual states across shared components.
- Avoid decorative gradients on ordinary settings surfaces. A server banner is presentation content, not an excuse to make the entire form compete for attention.
- Animations SHOULD be subtle, brief, and reduced-motion aware. Do not animate for the sake of looking modern.

### 4.4 Empty-state discipline

Every list and workspace needs a useful state when there is no data: no channels, no admitted members, no pending requests, no matches, no messages, no assigned roles, no notification permission, etc. Explain the situation and provide **one valid next action** where appropriate. An unavailable action is not an acceptable empty-state CTA.

---

## 5. Universal interaction contract

This is the most important section of this document.

### 5.1 No false affordances

Every visibly interactive element MUST have all of the following:

1. **An actual handler and supported operation.** No `onClick` that does nothing, fake links, or purely cosmetic buttons.
2. **Correct authorization.** A UI hint never substitutes for server enforcement.
3. **Predictable result.** Navigate, open a menu/dialog, change a local setting, or perform a confirmed mutation.
4. **Feedback.** Loading, success, error, and retry behavior appropriate to the operation.
5. **Keyboard and pointer support.** Appropriate semantics, focus, and accessible name.
6. **Test coverage** for the advertised behavior, ideally including one meaningful user-level path.

When backend support is absent, choose deliberately among:

- **Implement and connect** the feature if it belongs in the current task.
- **Display read-only** information using text, not an editable input or fake button.
- **Omit** the action until it exists.
- **Disable with an explicit reason** only when discovering the capability is itself useful, such as a permission restriction or temporarily pending security state.

**Never silently swallow errors or show success before the authoritative mutation succeeds.** Avoid vague “Coming soon” controls throughout production settings.

### 5.2 Action audit

Before changing a screen, inventory every apparent action:

| Control | Present state | Intended result | Required capability/API | Loading/error outcome | Verified test |
| --- | --- | --- | --- | --- | --- |
| Example: Save changes | Audit first | Persist dirty fields | Authoritative update | Conflict/error or success | End-to-end |

Complete this table in the implementation PR/checkpoint, not necessarily in this permanent file. Do not count an interaction as complete until it works through the real service or a documented local-only setting.

### 5.3 Forms and mutations

- Distinguish **view mode** from **edit mode**. A read-only server identity is text with Copy, not a faux input.
- Save only when fields are dirty and valid. Keep entered values after a recoverable error.
- Show validation near the affected field. Confirm destructive changes with clear target and consequences.
- Support optimistic display only when rollback and conflict handling are reliable; otherwise wait for authoritative confirmation.
- Handle stale policy revisions and concurrent edits explicitly: refresh, compare, retry, or resolve, never silently overwrite.
- For uncertain mutation outcomes, preserve an idempotent request identity and reconcile server state before resubmitting.
- Warn about unsaved changes before closing or navigating away.

### 5.4 Loading, unavailable, and error states

Use distinct language for **Loading**, **No data**, **No permission**, **Offline**, **Pending security transition**, **Unsupported by server version**, and **Failed**. A blank table or inert button cannot communicate all of these. Show useful recovery actions such as Retry when genuinely supported.

---

## 6. Members: server-wide directory and public sidebar

### 6.1 One member means one account

Member rows represent **admitted accounts**, not individual devices. An account with three devices still appears once. An account may have multiple separately managed authenticated devices.

**Public member presentation** MAY include avatar, display name, applicable visible role badges/colors, and authorized public profile information. Do not expose device UUIDs, account fingerprints, authorization generations, secret/key material, pending admissions, or hidden channel rosters in the public list.

The directory must derive from authoritative admitted-account information, **not a convenience API returning device contacts**. If only device contacts are available, do not label them a complete membership directory.

### 6.2 Right member sidebar

- Visible by default on wide windows, user-toggleable; compact overlay/drawer at narrow widths.
- Group each member once under their highest **separately displayed** eligible role. Use the defined deterministic hierarchy/tie rules.
- Provide a general **Members** group for accounts without a displayed grouping role.
- Show group counts and readable names. Role colors may affect names, but contrast must remain adequate.
- Clicking a member opens a profile popout; a context menu exposes actions actually authorized for the current user.
- Preserve privacy across channels. This sidebar is server-admitted membership, **not an implied list of members able to decrypt the current channel**.
- Do not display fabricated online/offline status. Presence remains omitted until a real model is implemented.
- Support sensible loading, empty, and large-directory behavior; paginate/window as needed.

### 6.3 Member administration

Use an account-first, searchable table or list with role filters, useful status, and actions. Separate **Members** and **Requests** using tabs or similarly obvious navigation.

- **Requests:** pending/rejected admission flows with functional Approve/Reject actions and appropriate confirmation/feedback.
- **Account details:** current roles, admission/moderation status, and an expandable **Devices** section.
- **Devices:** advanced identifiers, approval/revocation, and authorization diagnostics are explicitly device-scoped.
- **Moderation:** Kick, Ban/Unban, Timeout/Remove timeout as allowed; clear consequences, duration, reason where appropriate, and audit feedback.
- **Owner protections and role hierarchy** must be reflected in available actions, with the server making the final decision.

Never present a server kick as if it were an automatic cryptographic MLS removal; show pending MLS consequences where needed.

---

## 7. Profiles and role assignment

### 7.1 Profile popout

The public profile belongs to the account, not a representative device. Authorized
devices publish signed causal revisions under ADR 0010. Independent field changes
merge through an authorized signed merge; conflicting field values need review
and a preview. Show the last agreed profile until resolution. Missing ancestry,
replay, conflicts and per-server publication failures must remain distinguishable.
Legacy device cards are migration candidates, never automatic account authority.

Opening a member from a member row or message author should produce the **same** reusable profile UI:

- Avatar and account display name.
- Server role chips (clear color and label).
- Existing implemented interactions only.
- Moderation / role management actions only when authorized.
- Optional advanced details for account identity and device information, appropriately scoped.

No fake “Send DM,” presence, bio, or activity widgets before those systems exist. In particular, a role chip is an identity/presentation label, not proof that the viewer can enter every channel the role might authorize.

### 7.2 Assignment workflow

- Authorized users can add a role from a searchable chooser and remove manageable existing assignments.
- The chooser shows only roles assignable by the acting account, as reported by authoritative effective-action metadata.
- Clearly distinguish implicit `$default`, explicitly assigned roles, inherited capabilities, and owner identity.
- Changes propagate consistently to the profile, public member sidebar, member directory, and role editor after confirmed persistence.
- Do not implement separate inconsistent role pickers for each surface.

---

## 8. Roles and permissions administration

### 8.1 Roles editor layout

Prefer a two-pane layout: ordered role hierarchy on the left; selected role settings on the right. Provide Create Role and applicable Delete Role actions. Ordering SHOULD support pointer drag and keyboard move controls.

Suggested sections:

- **Display:** name, color, description, separately display members, and preview; mentionability only when the feature has defined behavior.
- **Permissions:** grouped capabilities with clear descriptions and warnings for powerful actions.
- **Members:** searchable accounts currently assigned to this role, with authorized add/remove operations.

Keep priorities as a backend representation. Do not make a normal administrator type numeric priority values.

### 8.2 Cords semantics override Discord assumptions

The UI MUST accurately reflect Cords' documented semantics:

- Implicit `$default` for admitted accounts.
- Explicit `$admin` inheritance of `$moderator` where defined; hierarchy position alone does **not** imply permission inheritance.
- Owner authority is separate from any role.
- Actual category/channel override evaluation (including deny precedence) follows the existing Cords evaluator, not whichever Discord behavior seems familiar.
- Role assignment authority and role-definition editing are distinct operations.
- Role ordering changes must be authorized, revisioned, and atomic.

If a permission is not yet enforceable, don't expose it as a working security control. Once full RBAC is active, remove preparatory-role warnings only after end-to-end enforcement gates pass.

### 8.3 Effective permissions

Provide understandable explanations of **why** a user may or may not perform an action, where feasible. An eventual effective-permissions inspector is valuable; it must query or reproduce authoritative semantics without becoming a second, divergent evaluator.

For server roles, use grant-style controls where appropriate. Reserve **Allow / Deny / Inherit** for actual channel/category overrides that support those states. Distinguish “no explicit grant” from an explicit deny.

---

## 9. Channels and conversations

### 9.1 Ordinary use

- Selecting a channel opens its conversation.
- Composer actions are available only when the account is authorized and the local device can actually participate.
- Explain inability to send with a precise cause: read-only permission, timeout, disconnected state, missing MLS enrollment, pending removal commit, or locked client.
- Keep search and message actions functional. Otherwise omit or clearly scope unavailable search behavior.
- Conversation details remain accessible through the header **Info** control; they are not the default public right sidebar.

### 9.2 Channel administration

The channel editor should distinguish:

1. **Everyday configuration:** display name, supported topic/description, category, and access policy.
2. **Membership and cryptographic state:** who is authorized versus which devices are actually participating.
3. **Lifecycle operations:** retiring, replacing, or succeeding an encrypted channel.

**Confidentiality mode is immutable for a channel identity.** Do not style replacing an encrypted channel as if it were a harmless dropdown save. Use a separate named workflow with a consequences summary and explicit confirmation. Never imply old history or encryption secrets transfer to a new identity.

For encrypted channel revocation:

- Show pending removal and which affected channel is fenced.
- State that application sends are paused until the cryptographic transition completes.
- Offer only legitimate authorized completion/recovery actions.
- If no eligible committer exists, explain succession as creating a **new** channel identity, not magically recovering the old MLS group.
- Keep unaffected channels usable; do not imply a global outage.
- Clarify that local archives may differ across devices.

### 9.3 Message moderation

Contextual message menus must distinguish **Delete my message** from **Remove message** for an authorized moderator. Show the author/target, request confirmation where appropriate, and reflect durable deletion tombstones consistently across clients. A deleted message should not reappear after refresh or replay. Never promise deletion of copies recipients already retained.

---

## 10. Server overview and presentation

Server presentation editing is in scope: name, description, animated icon and
banner. It uses separately authorized, revision-aware server APIs and durable
assets. Name edits never change the hostname, origin, server ID or signing pin.
Icons use a square crop and at most 1024 pixels/5,000,000 JXL bytes; banners use a
3:1 crop and at most 1536 by 512 pixels/10,000,000 JXL bytes. Share ADR 0009/0010's
lossless incremental codec and player with account avatars, preserving their
independent signing and authorization boundaries. Show progress, allow cancellation,
preserve drafts on failure and surface concurrent-edit conflicts before overwriting.

The server overview should feel like a normal settings page, not a mock form:

- Server name: editable only when the backend and actor's capability actually support updating it.
- Server identity/fingerprint: read-only text, with Copy and optional advanced trust details.
- Icon/banner: either a complete supported picker/upload/crop/save flow **or no clickable Change control**.
- Description: if not supported by the API, show a plain informational omission or omit the field. Do not render a dead textarea labeled “Not exposed by the server API.”
- Join policy: show human-readable choices and explain admission implications; only persist supported values.
- Save: enabled only for valid unsaved changes, with reliable pending/success/error/conflict states.

Separate server presentation from ownership and cryptographic identity. Cosmetic edits must not give the impression that the server signing identity changed.

---

## 11. Notifications and mentions

### 11.1 Source of truth

While the account is unlocked and Cords is running, all active joined servers
synchronize independently of selection. An offline server must not block others.
Show per-server connection state; maintain bounded/fair work, retry backoff and
durable account/server/channel cursors. Locking, switching or shutdown cancels work
and never implicitly unlocks another account.

Notifications are generated **locally after the client successfully synchronizes and, for encrypted channels, authenticates/decrypts new message content**. WebSocket notices may trigger fetches but do not themselves authorize or constitute a notification.

RBAC governs read/write/management permissions and therefore whether a client may receive channel data. **Roles do not grant “notification access.”** Their only direct connection to notification targeting is explicit structured role mentions.

### 11.2 User settings

Provide readable settings for:

- **All Messages** (default for ordinary channels, as currently decided).
- **Mentions Only**.
- **Nothing**.
- Per-server and per-channel overrides, mute, and application Do Not Disturb.
- Message preview privacy (default **hidden**), clearly distinguished from whether a notification occurs.

Suppress own-account messages from any of the account's devices, the focused visible conversation, historical first synchronization, duplicate events, deleted messages, and unauthorized/unreadable content.

The native notification opens the relevant account/server/channel/message when safe. When the app is locked, never expose hidden plaintext through OS notifications. Do not imply notifications are delivered when the app is completely stopped unless background execution exists.

### 11.3 Mentions

Use stable structured identifiers inside authenticated/decrypted message content for direct-account, role, and everyone mentions. The client resolves its own targeting from verified account roles after decryption. Do not invent a server-side notification-recipient list, plaintext scanning, or unsupported claims of server-enforced mention restrictions.

Unread state and “already notified” state are separate. A reconnect must not replay a wall of old notifications.

---

## 12. Security and privacy copy

Prefer human-facing explanations with an **Advanced details** path:

| Avoid leading with | Lead with |
| --- | --- |
| “Policy revision 3” | “Your permissions changed” (revision in details) |
| “Device generation 1” | “This device is approved” (generation in details) |
| “Publish a KeyPackage” | “Prepare this device for encrypted channels” (technical label in details) |
| “MLS removal pending” | “This channel is temporarily read-only while access is removed” |
| “Account fingerprint …” | Display name and roles, with identity verification details available |
| “Server retention not exposed” | “Retention information is unavailable from this server” |

Do not oversimplify away material security facts. A user must be able to inspect their server identity, device authorization, confidentiality guarantees, and pending cryptographic transitions. The design should explain consequences before performing an irreversible action.

Make permission-denied, revoked, banned, device-not-approved, and missing-key states distinguishable. Security-critical confirmations and audit messages should identify the action and target without exposing plaintext or secret material.

---

## 13. Accessibility, responsiveness, and performance

- All actionable items MUST be keyboard reachable, correctly labeled, and operable using Enter/Space as appropriate.
- Provide a visible focus ring, predictable tab order, and focus restoration after closing dialogs/popovers.
- Escape closes the topmost dismissible surface; destructive confirmations cannot be inadvertently accepted by Escape.
- Menus, dialogs, role chips, list rows, and icon controls need appropriate semantics and screen-reader names.
- Meet WCAG AA contrast targets for essential text and control boundaries where practical; never rely solely on color or hover.
- Respect reduced motion and text scaling.
- Animated avatars and server imagery play when visible by default; suspend them
  offscreen or when hidden. Honor OS/application reduced motion and a persisted
  animation-disable option. Expanded media views provide explicit playback.
- Do not trap scrolling across multiple nested panes. Constrain menus/popovers to the viewport.
- Server/member/channel lists must remain responsive with realistic large datasets; use pagination or virtualization where needed.
- Handle slow networks, service restarts, client lock/unlock, account switching, and offline/reconnect behavior without losing user context.
- Verify layouts at minimum typical desktop, narrower laptop/windowed desktop, and high-DPI/text-scaled configurations.

---

## 14. Acceptance scenarios

A design pass is not accepted until tests or a recorded manual validation demonstrate the following scenarios with real services when applicable:

1. **Ordinary member:** Open Cords, select server/channel, read and send permitted messages, toggle the public member list, inspect a profile, and open conversation details without seeing administrator-only diagnostics.
2. **Multiple devices:** One account with multiple devices appears **once** in public member lists and role assignment UI; device management remains separate.
3. **Permissions:** An owner assigns a role; another authorized administrator sees updated role chips; direct unauthorized API attempts still fail. A revoked account with an existing connection cannot continue accessing protected content.
4. **Role administration:** Reorder roles with pointer and keyboard, edit appearance, manage members, and view effective authorization without accidental hierarchy escalation.
5. **Admission:** Review a pending request, approve/reject, display accurate status, and avoid leaking pending accounts into public membership.
6. **Moderation:** Kick, ban/unban, timeout, and message removal show authoritative results and correct failure states; tombstones survive reconnect.
7. **Encrypted-channel revocation:** Clearly show pending cryptographic removal and channel-scoped send pause; authorized recovery/succession distinguishes a new identity from the old group.
8. **Notifications:** Local fetch generates an appropriate native notification, not during first-sync history, not for own messages, not for focused visible chat, and not twice on reconnect.
9. **Server overview:** Every visible Save/Change/Invite action works end to end or is absent/read-only. An API-unsupported field is not presented as editable.
10. **Failure and access:** Offline, timeout, invalid input, permission denied, stale revision, and missing feature/version produce distinct understandable feedback.
11. **Keyboard-only:** Complete ordinary navigation, member profile actions, role selection, and relevant administrative workflows without a mouse.
12. **Visual review:** The primary chat layout and administration pages use the same tokens and interaction patterns. No generic neon-green pseudo-actions or unexplained fake controls remain.

Keep screenshots (or other UI evidence), reproduction steps, relevant tests, and known limitations with each implementation checkpoint. Mock-only tests are insufficient proof of backend-dependent functionality.

---

## 15. Implementation workflow for AI agents and contributors

For each UI rework task:

1. **Read this document, relevant security contracts, and the real components/APIs.** Do not assume a screenshot proves implementation state.
2. **Audit current affordances** in the targeted surface. List what works, what fails, what is read-only, and what is genuinely unsupported.
3. **Identify the canonical state and authorization source.** Use shared account/role state rather than duplicate per-component caches where avoidable.
4. **Propose the smallest coherent UI change** that fulfills this contract; prefer reusable primitives and existing patterns.
5. **Implement complete vertical slices:** visible control → client operation → server or local persistence → feedback → refreshed state → tests.
6. **Validate behavior before polish**, then do a visual/keyboard/responsive pass. No decorative action without a working path.
7. **Record evidence and update this file** if the design contract changed, if a section is now implemented, or if a discovered limitation materially alters the intended UX.

Do not treat planned backend milestones as already implemented. Do not mark security-critical UI complete until real authorization and MLS behavior pass their own checks. Avoid unrelated rewriting of encryption, identity, or persistence code solely to simplify a screen.

### Status tracking

Maintain a small implementation ledger in the relevant PR/checkpoint or in this file as work progresses:

| Surface | Design status | Functional status | Evidence / outstanding issue |
| --- | --- | --- | --- |
| Chat shell / right sidebar | Shell refinement implemented | Frontend and browser reviewed | Info is opt-in; native modal drawers at narrow widths; account directory still blocks default members |
| Shell type / themes / empty states | Implemented | Frontend and browser reviewed | Relative type, 125% text, dark/light surfaces, valid connection action, no inactive composer |
| Public member directory | Specified here | Needs runtime audit | Must be account-centric, not device contacts |
| Profile popouts | Specified here | Needs runtime audit | Shared account presentation and actions |
| Member administration | Specified here | Needs runtime audit | Requests, devices, moderation, roles |
| Role editor | Specified here | Needs runtime audit | Hierarchy, display, permissions, members |
| Channel editor / lifecycle | Specified here | Needs runtime audit | Separate metadata from MLS replacement |
| Server overview | Read-only foundation implemented | Presentation API outstanding | Copyable identity/origin/policy; false Save/media controls omitted |
| Notification settings | Specified here | Needs runtime audit | Client-local decision and native delivery |
| Shared clipboard / read-only fields | Implemented | Frontend validated | Clipboard failures and success are announced |
| Local settings drafts | Implemented | Frontend validated | Dirty/discard, pending save and failed-save retention; packaged review outstanding |

Checkpoint evidence and the source action inventory are maintained in
`docs/reports/ui-rework-20261015.md`. The shell continuation passes frontend check,
lint, build, formatting and 91 tests. Browser fixtures cover desktop, laptop,
minimum window, light theme, larger text, and keyboard behavior. The preceding
backend unit passed 52 isolated PostgreSQL server tests, including five focused
current-policy tests. These remain separate from RBAC activation, media, and
packaged Windows/Linux/macOS acceptance.

“Needs runtime audit” is intentional. Do not mark a feature complete solely because a component exists or the design document describes it.

---

## 16. Design decision log

Record changes as short dated entries describing **decision**, **reason**, and **affected surfaces**. Do not silently rewrite established behavior.

- **2026-10-15 — Initial contract:** Use Discord-familiar information architecture without copying the product; default right rail to public members; keep conversation security details on demand; make accounts the primary member unit; retain Cords RBAC/MLS semantics; require every visible action to be functional or honestly noninteractive; generate notifications locally after message synchronization.
- **2026-10-15 — Consolidated implementation:** Complete existing backend milestones
  alongside independent UI work. Canonical profiles use account-bound signed causal
  history with field merging and explicit conflict review (ADR 0010). Share bounded
  animated JXL processing/playback with separately authorized server imagery. Sync
  all active joined servers for the unlocked account. Track backend, UI and actual
  packaged visual evidence as independent gates.
- **2026-10-15 — Visible shell refinement:** Use Cords blue with distinct neutral
  surfaces, relative typography, quieter conversation intros, and useful local
  empty states. Info starts closed and requires a channel. Until the admitted-account
  directory passes its backend gate, leave that right surface closed rather than
  presenting device contacts as public members. At narrow widths, native modal
  drawers preserve channel navigation and Info with Escape, bounded scrolling,
  keyboard focus containment, and restoration.

**This file is a maintained product contract, not a substitute for functional tests or the security architecture.**
