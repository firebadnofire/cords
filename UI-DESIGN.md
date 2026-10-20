# Cords Human Interface Guidelines

> **Canonical file:** `UI-DESIGN.md` at the repository root  
> **Status:** Adopted authoritative HIG revision, 2026-10-19  
> **Product:** Cords, *Right on the wire*  
> **Target:** Native Qt 6 Quick / QML with a custom Cords design system  
> **Scope:** The visual, interaction, accessibility, and user-facing behavior contract for Cords

## 0. Authority and how to use this HIG

**This is the master document for how Cords looks, feels, reads, and responds.** Every user-facing feature, screen, reusable control, and design change MUST follow it. It is not a sprint plan, a migration ledger, or a substitute for backend specifications.

Read it before planning or implementing UI changes. If the implementation disagrees with the HIG, correct the implementation unless an intentional product decision has been approved and recorded here. Update this document when the *design decision* changes, not because the current implementation happens to be deficient.

The terms are normative:

- **MUST:** required for any applicable implemented feature.
- **SHOULD:** default; deviations require a documented reason.
- **MAY:** optional and deliberate.
- **NOT YET:** an intended capability that MUST NOT be shown as operational before implementation and verification.

**Order of authority:** Protocol, cryptographic, identity, authentication, MLS, and RBAC specifications determine which actions are possible and authorized. This HIG determines how those actions are presented and performed. `AGENTS.md`, relevant ADRs (including ADR 0008), and current role-policy contracts remain binding in their domains. No design decision can waive a security boundary.

**A feature is not complete when it renders.** Completion requires an appropriate view, a working accessible interaction, an authorized core/server operation (or clearly designated local action), truthful feedback, persistence when applicable, and verification in a running native app.

This HIG deliberately does **not** carry dated implementation-progress tables, build machine inventories, or milestone estimates. Keep those in `docs/plans/qt-desktop-migration.md` or the current implementation ledger. Plans must cite this HIG, not rewrite it per session.

---

## 1. Product identity and design direction

### 1.1 The target feeling

**Discord-familiar, unmistakably Cords.** A Discord user should understand the interface immediately, while recognizing Cords as an independent, self-hosted communications product with its own identity and conventions.

The desired first impression is a mature, dense-but-readable desktop messenger, **not an application assembled from stock Qt controls**. It has strong navigation, recognizable people, restrained chrome, rich contextual interaction, and purpose-built administration screens.

The basic visual grammar is:

1. Server identities in a narrow, dark rail on the far left.
2. A channel and server-navigation column immediately to its right.
3. The dominant conversation area with a compact header and anchored composer.
4. An optional right rail for actual admitted server members, or a mutually exclusive Info surface.
5. Consistent profile popouts, contextual menus, hover tools, and management workspaces.

**Borrow Discord's interaction quality and information architecture, not its trademarks, proprietary art, upsells, or protocol semantics.** Do not copy every redesign merely because it is newer. The supplied reference screenshots are concrete visual/interaction targets; they are not authorization to invent unsupported product features.

### 1.2 Intentional Cords differences are not bugs

- Users are referenced with `@`; **roles are referenced with `$`**. Preserve `$role`, `$default`, `$admin`, and the documented role model rather than rewriting role references into Discord-like `@role` mentions. Autocomplete, token display, chips, previews, notifications, and documentation MUST respect the distinction. Resolve mentions to stable IDs when the underlying feature is implemented; do not derive authority from display text.
- Cords servers are **independently hosted servers**. The rail is not a list of rooms on one central service.
- Account identity and device identity are distinct. A public member is one verified account, not an individual device connection.
- Server roles, channel access, MLS enrollment, and encryption state are distinct. Visual grouping cannot imply cryptographic access.
- Server/community administration is not machine/SSH administration. Keep both the permissions and navigation boundaries explicit.
- Account switching changes the selected identity and its own remembered server bar, credentials, drafts, and applicable local preferences. Never merge state across vaults.

### 1.3 Product-wide non-goals

Do not introduce fake friends, direct messages, voice calls, presence, activity games, attachment controls, integrations, Nitro, subscriptions, boosts, or monetization merely to fill space. Do not show server-administration items whose backend behavior is absent. Do not expose MLS epochs, key packages, device fingerprints, UUIDs, or raw HTTP errors as ordinary chat chrome.

No WebView, Electron, Tauri, browser DOM, or CSS renderer is used for the target native application. The intended UI is Qt Quick/QML.

---

## 2. Visual design system

### 2.1 Principle: quiet structure, strong content

Conversation and people receive visual emphasis; infrastructure is available but not constantly demanding attention. Structure should emerge from surface relationships, typography, alignment, and transient interaction states, not permanent thick borders around every rectangle.

**Avoid:** a uniform navy sheet with slightly different rectangles, raw default Qt buttons, large empty gutters, bright outlines on every surface, random control heights, naked device identifiers, and permanent debugging/status labels in high-value navigation areas.

**Prefer:** layered charcoal surfaces, narrow separators, clear active states, compact rows, restrained accent usage, and controls revealed at the moment they are relevant.

### 2.2 Dark appearance baseline

The following palette is a **proposed concrete baseline** for the default charcoal appearance. Validate colors in real Qt renders against the approved Cords branding and reference captures before treating the values as final. Colors MUST be defined centrally as semantic QML theme tokens, never scattered across individual screens.

| Semantic token | Starting value | Use |
| --- | --- | --- |
| `window` | `#232429` | Outer/background frame |
| `serverRail` | `#202127` | Deepest left rail |
| `navigation` | `#2B2D33` | Channel and settings sidebars |
| `conversation` | `#32343B` | Message surface and primary content |
| `memberRail` | `#2B2D33` | Right context/member rail |
| `elevated` | `#3B3D46` | Popovers, menus, selected settings surfaces |
| `input` | `#262830` | Text entry and search |
| `hover` | `#3E414A` | Transient hover background |
| `selected` | `#454954` | Persistent selection background |
| `divider` | `#42454D` | Subtle structural lines |
| `textPrimary` | `#F2F3F5` | Main labels, messages |
| `textSecondary` | `#C6C9D0` | Secondary details |
| `textMuted` | `#9DA2AC` | Hints and timestamps |
| `accent` | `#83BDD9` | Proposed Cords accent; selected/primary actions |
| `danger` | `#EE777D` | Destructive controls and genuine errors |
| `success` | `#78C99A` | Confirmed success, not generic links |
| `warning` | `#E8BC70` | Warnings and pending attention |

Check actual text/background pairs for WCAG AA contrast (4.5:1 for ordinary text, 3:1 for large text and discernible control boundaries); adjust token values before shipping. Role-selected colors MUST be transformed or supplemented as needed for readable text on the active surface. Do not use color alone to communicate permissions, connectivity, or failure.

Dark, midnight, and light variants share **semantic token names and layout geometry**. A different theme must not rearrange common navigation. Theme choices and user density settings are separate.

### 2.3 Typography

Use a legible, properly licensed local/system sans-serif font; never require network font downloads. Adopt one coherent scale, tuned by platform font metrics rather than arbitrarily mixing point and pixel sizes:

| Purpose | Starting Qt logical size | Treatment |
| --- | --- | --- |
| App/section title | 20–22 px | Semibold |
| Page title | 18–20 px | Semibold |
| Section header | 15–16 px | Semibold |
| Body/message | 14 px | Regular; user-scalable |
| Navigation/member name | 13–14 px | Medium where needed |
| Buttons/fields | 13–14 px | Medium or regular |
| Timestamp/helper text | 11–12 px | Muted, still readable |
| Technical ID/code | 12–13 px | Monospace only when meaningful |

Avoid all-caps body labels. Short all-caps category headings are acceptable in navigation when they improve scanning. Do not squeeze legibility merely to match a screenshot. Support user text scaling independently from OS device-pixel ratio.

### 2.4 Spacing, shape, and density

Use a documented spacing scale: **4, 8, 12, 16, 24, 32 logical px**, with 2 or 6 px optical adjustment only in shared components. Consistent alignment is more important than an exact token everywhere.

| Element | Starting geometry |
| --- | --- |
| Primary/secondary control height | 32–36 px |
| Compact icon-only action | 28–32 px with usable hit area |
| Search field | 32–36 px |
| Channel row | 32–36 px |
| Member row | 38–44 px |
| Message avatar | 36–40 px |
| Member avatar | 30–32 px |
| Server icon | 44–48 px in a 64–72 px rail |
| Compact window/channel header | 48–52 px |
| Composer minimum / maximum | ~40 px / bounded 5–6 lines |
| Settings form content width | ~680–760 px, not stretched to fill a huge monitor |
| Common control corner radius | 6–8 px |
| Popover/dialog corner radius | 8–12 px |

These are **design targets, not unbreakable fixed widths**. Prefer intrinsic sizing and adaptive constraints. Compact and comfortable density variants SHOULD be supported. An explicit server-icon size preference MUST be independent of global text scale.

### 2.5 Hierarchy and iconography

Use a consistent, legally redistributable icon family with matching stroke, size, baseline, and optical weight. Do not mix emoji, platform glyphs, ASCII symbols, and multiple unrelated icon sets to represent adjacent actions. Every icon-only action needs an accessible name and a tooltip.

The accent indicates primary action or selection; danger marks destructive action; role colors identify people/roles. Never use a single bright status color for everything clickable. An active element must be visibly different from merely hovered or focused elements.

Dividers should be quiet and purposeful. Avoid bordered-card nesting for basic settings rows. Dense admin tables should not be styled as a stack of enormous cards.

### 2.6 Motion

Use short, unobtrusive hover/focus transitions (roughly 80–150 ms) and popover/panel transitions (roughly 120–200 ms), tuned in native captures. Motion SHOULD explain state, not delay tasks. Obey reduced-motion preference and provide effectively immediate alternatives. Animated JXL avatars follow the specific per-instance playback contract in §6.

---

## 3. The desktop shell

### 3.1 Canonical desktop anatomy

```text
┌────────┬────────────────────────┬──────────────────────────────────────┬───────────────────────┐
│ SERVER │ SERVER / CHANNEL TREE  │ #channel        Search Members Info │ MEMBERS               │
│ RAIL   │ ────────────────────── │──────────────────────────────────────│                       │
│        │ Category               │ avatar  Name            timestamp   │ ROLE GROUP            │
│ icons  │   # channel            │         Message text                 │   avatar Name         │
│        │   # another            │                                      │   avatar Name         │
│        │                        │ More conversation                    │                       │
│        │ Current account strip  │ Message composer                     │                       │
└────────┴────────────────────────┴──────────────────────────────────────┴───────────────────────┘
```

Desktop default: **four conceptual areas**, with conversation owning flexible remaining width. Suggested rail widths: **64–72 / ~240 / flexible / ~240 logical px**. Header, account strip, and composer align; side panels scroll independently. Do not add a redundant full-width app toolbar or permanently display technical diagnostic panels where people should appear.

At narrower widths, collapse the member rail first; expose Members and Info as mutually exclusive drawers when needed. Next permit collapsing/reducing channel navigation. Never make the composer or conversation unusably narrow to retain four simultaneous columns. Save panel visibility/size choices where appropriate.

### 3.2 Server rail

- Server identity appears as a real icon (preferably recognizably large) or a deterministic, polished letter fallback. No mock servers.
- Use a clear active indicator at the rail edge; unread and selected states are distinguishable. A hover state MAY smoothly modify icon shape or highlight, but does not masquerade as selection.
- Server tooltips show display name. Host URL, verified signing identity, and trust pin belong in details, not as the dominant list label.
- Clicking selects the server and restores the last eligible channel. Do not suspend eligible background sync on other unlocked joined servers.
- Provide supported Add/Join, ordering, contextual actions, and a sensible disabled reason where necessary. Destructive Disconnect/Drop controls are clearly distinguished from normal selection.
- A selected account shows **only that account's remembered servers**. Switching vaults must immediately clear stale avatars, members, channels, drafts, and delayed results from the former account.

### 3.3 Channel and server navigation

- A compact server-name header opens a real menu with relevant supported actions (server settings, invite, connection/details, etc.).
- Below it: searchable ordered text channels, expandable/collapsible categories where supported, role/permission-aware creation controls, and stable `#` channel cues.
- Users join servers and open readable channels; there is no user-facing channel-join operation. Hide channels without read access. Initialize local history and authorized device encryption state automatically, without treating encryption setup as a permission grant.
- Indicate selected, unread, muted, inaccessible (when visibility policy permits), and temporarily read-only channels differently. Do not infer unread from arbitrary cached list order.
- Selected rows use a persistent surface treatment; hover actions appear contextually. Align icons and labels across categories.
- Keep the channel tree scrollable independent of chat, and preserve expanded state and last selection appropriately.
- Bottom strip contains the selected account avatar/name and compact actionable settings, profile, lock, and account-switch entries. Routine sync completion is not a permanent navigation headline.

### 3.4 Channel header and conversation

- Keep a **48–52 px** compact header identifying the selected channel with stable search, Members toggle, and Info affordances where supported. Avoid shuffling controls on each channel change.
- Read messages in chronological order with thoughtful bottom anchoring. Scrolling upward to history MUST NOT cause unwanted snap-to-bottom when new events arrive.
- Each message retains its unique ID and available timestamp. Group consecutive messages by **same verified author within approximately five minutes**, breaking at date changes and system events. Do not merge away independently selectable messages.
- A grouped first row shows avatar, author display name, and timestamp. Continuations are visually tighter; their individual timestamp can appear on hover/focus. Preserve accessibility of each message.
- Role color may affect readable display-name styling, not permission claims. Do not present raw device UUIDs as preferred sender names.
- Hover/focus reveals a compact action toolbar without reserving permanent blank space for it. Right-click opens a context menu with **only supported** copy/reply/edit/delete/moderation actions, based on actual capability data.
- Sender avatar or name opens the same account profile popout used by the member rail and admin directory.
- Treat loading, no local history, empty confirmed history, unjoined, insufficient access, pending approval, failed fetch, and archived channel as separate states with accurate next actions.

### 3.5 Message composer

- Anchor a single cohesive multiline input to the bottom of conversation; do not render a bare text box alongside an unrelated-looking default Send button.
- Enter sends; Shift+Enter inserts a line break (unless a deliberate user preference changes this). Composer grows only to a bounded height, after which its own content scrolls.
- Preserve unsent input per account/server/channel, including after recoverable send failure; protect drafts on navigation and lock.
- Send state communicates in-progress, failure/retry, and inability to post. A disabled composer MUST explain whether the cause is permission, connection, lock, pending membership, or pending cryptographic state.
- No visible attachment/emoji/reply/formatting affordance unless it performs a real operation. Existing supported actions should be readily discoverable.

### 3.6 Member / Info rail

- Default right rail on a sufficiently wide window shows **public admitted server members**, not device contacts, diagnostics, or pending applicants.
- Present one row per account, with verified avatar/display name and readable role-group headings/counts. Group each member once under its highest eligible *separately displayed* role, then a general member section. Do not infer presence from historic connections or show fake green status dots.
- A row opens the shared profile popout. Authorized contextual actions can offer inspect roles, copy identity, moderate, etc., but never expose hidden restricted rosters or secrets.
- A header Members control toggles the rail. **Info** replaces it with relevant technical conversation information and a return path. Do not stack multiple right panels simultaneously.
- At narrow widths, these surfaces become dismissible overlays. Closing them does not reset channel scroll or composer state.

---

## 4. Reusable controls and interaction grammar

Create a coherent `Cords.UI` component library rather than repeating ad-hoc `Rectangle + MouseArea` implementations. Expected primitives include `CordsButton`, `CordsIconButton`, `CordsTextField`, `CordsSearchField`, `CordsCheckbox`, `CordsToggle`, `CordsSelect`, `CordsMenu`, `CordsPopover`, `CordsDialog`, `CordsToast`, `CordsTabs`, `CordsAvatar`, `CordsRoleChip`, `CordsMemberRow`, `CordsSettingsRow`, `CordsEmptyState`, and progress/error views.

### 4.1 Standard control states

Every applicable component MUST expose and style **resting, hover, focused, pressed, selected/checked, disabled, busy, error, and success** states. Do not let keyboard focus use a less visible style than mouse hover. Pending submission disables duplicate commands without swallowing cancellation when cancellation is safe.

| Element | Expected behavior |
| --- | --- |
| Button | Clear primary/secondary/quiet/destructive variants; state and action match label; no inert labels posing as buttons |
| Icon button | Consistent glyph/size/hit target; tooltip and accessible name identify the action and, when needed, target |
| Text field | Label, clear value, validation, edit state, selection/clipboard support, focus ring, recoverable submission error |
| Search | Search scope explicit, results/count or useful empty state, fast updates without blocking UI thread |
| Checkbox/toggle | Click label and control; checked state truthfully changes policy; keyboard Space toggles |
| Menu | Context-aware entries, separators only when meaningful, nested items sparingly, never leaves window bounds |
| Popover | Anchored to invoking element; closes on Escape/outside click; focus restores to invoker |
| Dialog | Clear title and consequence, primary action, Cancel, focus trap, Escape behavior where cancellation is valid |
| Toast/inline status | Communicates completed result without replacing error detail needed for recovery |
| Empty state | States *why* content is missing and offers the correct available action, not a generic blank page |
| Role chip | Color and role name, accessible; role reference and membership operate using stable role IDs |

### 4.2 Hover is not permission

Show contextual tools at hover/focus where helpful, but expose the same operations via keyboard-accessible menus. A hidden button is not server-side authorization. Derive availability from typed capability data; every privileged operation is enforced by the authoritative service.

### 4.3 Mutations and forms

- Save is enabled only for changed, valid data. Changing a field does not imply remote persistence.
- Use explicit Save/Cancel or clearly indicated per-section autosave; do not mix both unpredictably on the same form.
- Preserve drafts on recoverable failure and warn before navigating away from dirty forms.
- Show optimistic feedback only when it can be reconciled with authoritative results. A spinner vanishing does not prove success.
- Handle stale revisions, conflicts, retries, and duplicate submissions; use stable IDs, not QML delegate indices.
- Confirm impactful actions with target and consequences. Ordinary reversible actions should not require exhausting confirmation rituals.

### 4.4 Profile and action popouts

Use one shared presentation for an account, regardless of whether opened from a message, member row, or administrator listing. Content may include verified account avatar, signed display name, visible server roles, supported actions, and an explicit Details section for identity/device metadata. No invented bio, online presence, DM, or activity button.

---

## 5. Settings and administrative information architecture

**Do not force every type of work into one generic page template.** The provided Discord examples show three distinct layouts that Cords SHOULD adopt deliberately:

1. **Account settings:** persistent left navigation, readable sections and setting rows, contextual edit controls.
2. **Server settings:** persistent navigation, bounded forms with descriptions and visual previews.
3. **Administrative workspace:** broad searchable/sortable tables or two-pane editors, using available width.

Use a dedicated app-level settings workspace (rather than a forest of floating unstyled windows), with a predictable close/return control and Escape handling. Sidebar selection persists while switching sections; content scrolls within a single clear primary region. Normal forms target approximately **680–760 px** content width, while member directories and other tables may expand much wider.

### 5.1 Account settings

Group related operations; only show categories that contain useful implemented content. Likely areas: **Profile & Identity**, **Password & Security**, **Privacy & Notifications**, **Appearance & Accessibility**, and **Local Accounts / Saved Servers**.

- Profile identity comes from the verified signed account-level profile. Do not treat devices as separate public people.
- Change avatar via actual file selection, preview/crop, bounded processing, upload/publication, and actionable errors.
- Mask sensitive values appropriately. Reveal operations must accurately reflect the supported security policy.
- Password changes and vault lock/unlock are not interchangeable. Label each operation according to what it affects.
- Account switch follows the existing account-picker-first flow and vault lifecycle. Clear old context immediately.
- Read-only identity values should look selectable/copyable, not like broken Edit fields.

**Specific local vault-policy control:** label the supported override **“Ignore weak passwords”** without the previous misleading 12-character explanatory paragraph. Unchecked applies the normal local vault password policy. When checked, bypass local password length/strength checks; password/confirmation matching remains the only password-validation condition for that opted-in workflow. Do not quietly enforce a hidden client minimum or misrepresent the setting as a server-admission password policy. Respect any independent mandatory cryptographic format constraints, and state them as a separate concept rather than secretly treating them as password-strength checks.

### 5.2 Server profile and appearance

Show an understandable editor for **name, description, icon, and banner** when implemented. Mirror the screenshot's idea of a live server preview. File selection, crop/position, preview, upload, removal, cancellation, save, failure, authoritative refresh, and restart persistence are parts of the same feature.

Host URL, signing key, and trust fingerprint are immutable identity information with appropriate copy/verification actions, not cosmetic profile fields. A presentation change must not modify the trust identity.

Optional tags, games, traits, engagement metrics, and promotional pages appear **only** if Cords has real data and intentional product requirements; the screenshot does not authorize them automatically.

### 5.3 Member administration

Use a genuinely wide **directory table**, with verified account avatar/name, membership facts the server actually knows, role chips, search, sorting, selections, and context actions. Do not invent joined-Discord dates, online status, or recruitment signals. A selected row opens member details and supported administrative operations.

Show loading, no results, genuinely zero admitted members, insufficient permission, and fetch failure differently. Bulk moderation actions require understandable eligibility and precise confirmation.

### 5.4 Roles and permission editor

Prefer a hierarchy/list on the left and a contextual editor on the right with **Display**, **Permissions**, and **Members**. Include Create Role, search, role color, count, edit, member assignment, reorder where authorized, and delete where supported. Support keyboard reordering when drag-and-drop exists.

**Cords semantics outrank visual convention:** `$default` is implicit for admitted accounts; documented `$admin` → `$moderator` inheritance is explicit, while visual priority alone does not create inheritance. Owner authority remains independent. Channel/category policies follow the actual **Inherit / Allow / Deny** resolver and precedence rules. Assigning a role, editing a role, and managing encryption membership require distinct authorization where specified.

A role editor must not visually advertise preparatory/display roles as enforced RBAC until verified activation. Conversely, remove obsolete warnings after enforced behavior is proven.

### 5.5 Access, invites, applications, and rules

Separate **how an account joins** from **what it must acknowledge before participating**.

- Join choices may include **Invite only**, **Apply to join**, or **Discoverable**, but each is rendered as an actual choice only if its backing infrastructure and policy exist.
- Invitations need real creation, copy/share, expiry/limit where supported, listing, and revocation.
- Application-required admission needs a real submit → persist → admin-review → approve/reject → client-state transition workflow.
- Rules acknowledgement is a distinct control. An editable rules list includes Add, Edit, Remove, Reorder, Preview, Cancel, and Save only when all are operational; enforcement is server-authoritative.
- Age gating is NOT YET unless a genuine policy and enforcement model exists; do not copy a meaningless Discord toggle.

**Pending admission:** never show `CORDS_APPROVAL_PENDING: HTTP 403 Forbidden` as the primary UI. Show **“Waiting for administrator approval”**, explain which server and what happens next, allow a real refresh/status check, and transition after approval or denial. Pending applicants are not public admitted members.

**Administrator Requests:** display pending applications with verified requesting identity, submission time, meaningful details where supported, Approve/Reject, refresh, permissions, loading/empty/error states. An empty list cannot silently conceal a failed fetch.

### 5.6 Moderation and advanced/operator tasks

When supported, expose kick, ban/unban, timeout, device revocation, own/moderator deletion, and audits in context-specific menus/pages. Distinguish public account consequences from device-specific ones and from MLS-related pending changes. Confirm destructive actions; do not promise remote erasure of content users may have retained.

Technical diagnostics, server signing identity, MLS state, device approvals, event revisions, retention, and recovery/succession belong in clearly labeled **Advanced / Info** surfaces. Explain practical consequences first, jargon second. Hosting/machine management is an entirely different authority domain and should never be silently included in a community moderator's tools.

---

## 6. People, roles, mentions, and media

### 6.1 Verified public identity

One public row and one profile popout represent **one admitted account**. The displayed name and avatar derive from the canonical signed account-level profile. Server-scoped roles do not mutate that identity. Keep device details expandable and suitably authorized, not part of normal social identity presentation.

A server might not immediately possess the newest signed revision from another independently hosted server. Show the latest *verified available* record; do not fabricate updates, resolve conflicts by arrival time, or imply implicit federation. The Rust core implements validation/merge rules; QML renders the resulting verified snapshot.

### 6.2 `$` roles and `@` users

`@name` and `$role` are deliberately distinct user-facing concepts. Keep distinct autocomplete headings, symbols, entity chips, and announcement/notification descriptions. Resolve mentions to verified account/role IDs and the real structured-message semantics where supported. Literal plain text containing `@` or `$` must not silently grant permissions, trigger role actions, or claim structured mentions.

Role-color styling identifies membership or grouping; it is not evidence of permission to view or decrypt a channel. Mention notification eligibility is determined by the core after authorized content processing.

### 6.3 JPEG XL account and server assets

- Support the existing bounded static and animated JXL pipeline using native decoding and Qt rendering. Never assume that QML `Image` automatically supports all animated JXL semantics.
- Use shared cached decoded *still* data for repeated instances of the same account/image revision. **Playback clocks are independent per visible instance.**
- A message-associated avatar animates while its **whole message row** is hovered; a member avatar animates while its **member row** is hovered. Leaving the region restores its still frame. Expanded profile avatars animate while displayed if permitted by reduced-motion preferences and encoded loops.
- Preserve frame timing, compositing, alpha, loops, crop/position, cancellation, and memory limits. Offscreen/recycled delegates release playback resources.
- Server icon/banner publication is separate from signed account-profile publication and never alters server identity/trust pins.

---

## 7. Copywriting and state communication

**Use plain language for normal states and technical detail on demand.** Do not teach users the internal API simply because it was easier to expose an error string.

| Condition | Primary language | Appropriate action |
| --- | --- | --- |
| Application awaiting approval | **Waiting for administrator approval** | Check status, view relevant server details |
| Request denied | **Your request to join was denied** | Return to server selection / contact staff where applicable |
| Offline | **Can't reach this server** | Retry, inspect connection details |
| Locked vault | **Account locked** | Unlock |
| History not yet fetched | **Loading messages…** | Wait / Retry if failed |
| No messages confirmed | **No messages yet** | Start a conversation if authorized |
| Local cache empty | **Messages haven't been loaded on this device** | Synchronize/load when possible |
| Not authorized to post | **You can't send messages here** | Explain relevant permission/state |
| Save conflict | **This changed elsewhere** | Review/reload/reconcile |
| No pending applications | **No membership requests awaiting review** | No artificial action necessary |
| Failed admin request fetch | **Couldn't load membership requests** | Retry; diagnostics if needed |

The exact copy may vary with context, but it MUST correctly distinguish `Loading`, `Empty`, `Disconnected`, `Locked`, `Pending approval`, `Denied`, `No permission`, `Pending cryptographic transition`, `Unsupported server version`, `Conflict`, `Failed`, and `Succeeded` where relevant.

Status belongs near its object or as a temporary, actionable notification. Do not permanently occupy prime sidebar space with “Membership approved” or “Synchronize” when normal operation can happen silently. A tooltip or Info surface can show more detail.

A control that cannot operate must either disappear, become honestly read-only, or be disabled **with a useful reason** when discovering its existence is valuable. Never show success until the authoritative operation succeeded.

---

## 8. Keyboard, accessibility, and desktop interaction

- All ordinary chat, navigation, profile, menus, settings, and administrative actions MUST be operable by keyboard. Provide visible focus rings and sensible Tab/Shift+Tab order.
- Enter/Space activate the focused control according to convention. Escape closes the topmost cancellable menu, popout, or dialog and returns focus to its trigger. Escape never implicitly confirms destructive operations.
- Menu placement is constrained to the usable window/screen; do not spawn popouts where they cannot be reached.
- Custom QML controls define the proper `Accessible.role`, `Accessible.name`, `Accessible.description`, and actions. Mouse-only handlers do not meet accessibility acceptance.
- Search and large lists support keyboard selection, announcement of useful counts, and reliable focus after updates.
- Do not communicate hover-only actions solely through animation or color. Expose equivalent focus/context-menu actions.
- Respect reduced motion, text scaling, contrast requirements, platform high-DPI behavior, and installed screen-reader backends where available.
- Persist sensible user preferences without rearranging common controls unexpectedly. Users should not have to relearn channel navigation after changing themes.

---

## 9. QML and Rust implementation boundary

This HIG is primarily a **user interface specification**. Technical implementation follows existing architecture and ADRs, with these constraints relevant to UI correctness:

- **Qt 6 Quick/QML** implements scene/layout, custom styling, animations, local presentation and input. Use Qt Quick Controls/Templates or a coherent Cords style; do not rely on raw OS-native default visual themes.
- The typed **CXX-Qt / QObject bridge** exposes immutable identifiers, typed snapshots, authorized commands, async progress/results, and cancellation. Rust owns protocol, synchronization, authentication, key material, encrypted persistence, policy decisions, and profile validation.
- Never put authentication secrets, MLS state, or server-authoritative authorization in QML properties. QML cannot become a second permission evaluator.
- Do not block the GUI thread on networking, database access, encryption, image decoding, or remote services.
- Use `ListView`/proper Qt models and recycled delegates for substantial lists; reset ephemeral delegate state on reuse. History preprends, scroll anchors, profile popouts, and animation state must remain stable.
- Scope drafts, search, selections, focus restore, delayed requests, and results by account/server/channel. Locking or switching vaults immediately invalidates former presentation data.
- All eligible joined servers of an unlocked account continue background synchronization even when another server is selected. Errors on one server cannot freeze the rest.
- Native notifications are classified by the Rust core only after authorized fetch/decryption. Suppress historical, duplicate, own-account, visible-chat, deleted, inaccessible, and locked-content notifications. No fake presence from connected devices.
- Notification settings should support supported All Messages / Mentions Only / Nothing choices, server/channel overrides, and mute without conflating unread state with OS notifications.
- Desktop OS-specific window controls, file pickers, system notifications, and packaging may vary, but the Cords interaction model must remain recognizable on Windows, Linux, and macOS. Android is a future form factor, not proof of a current mobile client.

---

## 10. Visual and behavioral acceptance

### 10.1 Screenshot reference suite

The provided references establish canonical *types of screens*, not an obligation to copy their copyrighted visual assets. Maintain repository-approved captures or equivalent accessible design references for:

| Reference | What a reviewer compares |
| --- | --- |
| Main chat (current Cords baseline + desired Discord-level target) | Four-region layout, density, rail hierarchy, message treatment, composer, member identities, contextual actions |
| Discord account settings | Persistent navigation, group headings, settings rows, readable content width, edit buttons, status semantics |
| Discord server profile | Bounded editor, live preview, icon/banner controls, balanced column layout |
| Discord member management | Full-width table, search/sort, compact cells, role chips, contextual row actions |
| Discord roles | Searchable hierarchy, member counts, create/edit operations, role colors, contextual editor |
| Discord access and rules | Join-policy chooser, separate acknowledgement, clear cards, editable rule list |

**Do not claim reference comparison was performed without rendering Cords and actually inspecting the images.** Comparing screenshot proportions, spacing, and behavior is a design test, not a license to copy Discord trademarks.

### 10.2 Required capture states

For each touched major surface, capture both normal and interaction states as applicable: normal, hovered/focused, selected, menu/popover open, dialog open, validation error, loading, permission denied, network error, and empty-but-successful data. Test account/server/channel switching and state restoration.

Use at least **1440×900**, **1024×768**, and **150% display scaling** for current Windows acceptance, and add narrow/minimum-window and large-font variants. A separately packaged Windows smoke test is needed; headless/offscreen QML rendering does not replace native mouse/keyboard verification. Linux and macOS support remain unverified until their actual native runtime checks are performed.

### 10.3 Required behavioral scenarios

1. Launch, select an account/server/channel, send/read permitted messages, preserve scroll anchors and focus.
2. Verify server, channel, member, and conversation panels at wide/narrow sizes; Members/Info replacement works.
3. Account/profile avatar and name updates reach every appropriate visible surface after verified synchronization.
4. Message grouping, hover tools, profile clicks, context actions, search scope, and composer shortcuts work.
5. Account switching never leaks old server lists, drafts, identity, or stale asynchronous results.
6. Admin can see and approve/reject a real pending application using an independent second vault; requester sees corresponding state.
7. Role identity/color/grouping and supported assignments update through the same server operation, with forbidden mutations rejected by the server.
8. Server appearance edits preview and persist without changing server signing identity.
9. Animations use correct per-instance JXL playback and stop when hidden/offscreen or reduced motion applies.
10. Background sync on server B works while server A is selected; unavailable server C does not stall either.
11. Keyboard and accessibility controls reach every important operation; Escape/focus restoration and dialog guards work.
12. No apparent action is inert, no pending state is described as an unexplained crash, and no technical ID replaces a verified user name by default.

### 10.4 Definition of done for each screen

Maintain a short action inventory in the implementation ledger (not here): **element → user expectation → real core/server/local action → required authority → states/errors → validation evidence**. A screen passes only when its visual composition, behavior, correctness, and accessibility have all been validated. A working backend without an accessible UI and a pretty QML mockup without a working backend are both incomplete.

---

## 11. HIG governance

- A change that affects Cords' appearance, layout, input conventions, shared component behavior, or user-facing copy **must be reconciled against this HIG**.
- Small bug fixes that restore the written design do not need a policy rewrite.
- Intentional changes require: date, decision, motivation, alternatives considered when meaningful, affected surfaces, and updated reference captures/tests.
- Keep implementation status, evidence, blockers, and migration gates in `docs/plans/qt-desktop-migration.md` or its successor, not mixed into these enduring rules.
- Changes to `$` role semantics, account/device identity, security policy, or MLS are protocol/product decisions first. This HIG may reflect them only after the governing specification changes.

### Design decisions

- **2026-10-19 — HIG-first presentation:** Make `UI-DESIGN.md` the permanent visual and behavioral master, with explicit charcoal reference palette, component geometry, screen-specific UI patterns, Cords `$` role identity, and visual acceptance gates. Move ephemeral milestone status out of the master design rules.
- **2026-10-16 — Full-window settings pattern:** Account/server settings use left navigation, bounded forms, close/Escape return to chat; administration tables may expand to use width.
- **2026-10-16 — JXL instance animation:** Share still-image decode by verified account/image revision while keeping hover/profile animation playback clocks independent per visible instance.
- **2026-10-15 — Native interface direction:** Qt 6 Quick/QML custom Cords style; typed Rust bridge; no embedded browser shell.
- **2026-10-15 — Familiar interaction structure:** Discord-like four-region desktop mental model with Cords identity, server independence, and protocol-specific semantics.

### Technical references

- [Qt Quick](https://doc.qt.io/qt-6/qtquick-index.html)
- [Customizing Qt Quick Controls](https://doc.qt.io/qt-6/qtquickcontrols-customize.html)
- [Qt Quick ListView delegate reuse](https://doc.qt.io/qt-6/qml-qtquick-listview.html)
- [Accessibility for Qt Quick](https://doc.qt.io/qt-6/accessible-qtquick.html)
- [Qt Quick Test](https://doc.qt.io/qt-6/qtquicktest-index.html)
- [Asynchronous image providers](https://doc.qt.io/qt-6/qquickasyncimageprovider.html)

**Final rule:** Cords should feel so familiar that people can just chat. The complexity of self-hosting, permissions, cryptography, and synchronization must not be paid for in unnecessary clicks, strange layout, misleading labels, or dead buttons.
