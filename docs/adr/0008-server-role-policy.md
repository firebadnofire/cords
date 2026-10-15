# ADR 0008: Account roles and server policy

- Status: Implementation contract under the explicit 2026-10-12 product request;
  activation requires the integration gates in the implementation plan.
- Supersedes ADR 0006's deferral of public-channel role overrides only when the
  role-policy capability is activated. Confidentiality and lifecycle remain immutable.
- Extends AGENTS.md section 10 with persistent account roles. Existing capability
  strings retain meaning; owner-only transitions remain owner-only.

## Approved enforced-RBAC milestone (2026-10-15)

The approved [seven-checkpoint contract](../plans/member-rbac-milestone.md)
supersedes this ADR's preparatory-runtime end state and server-distributed role
notification design. The following requirements are normative for that milestone;
the remaining sections describe the baseline until the integration gate passes.

New installations must enforce RBAC immediately once the integrated implementation
is delivered. There is no permanent legacy/preparatory mode or activation wizard.
Development assignments are archived, never silently promoted to authority. Native
signature validation constructs restricted encrypted-channel account boundaries
from existing active, delivery-enabled device memberships only. Account access
cannot reactivate excluded/revoked devices, change joined-sequence history bounds,
or create MLS membership. Invalid/expired identity evidence supplies no grant.

Pending cryptographic removals fence application sends only in affected encrypted
channels. Authorized control processing remains possible. An unavailable committer
leaves the original fenced; it never justifies use of revoked devices or old keys.
Recovery uses a current eligible committer or explicitly confirmed authenticated
succession with fresh channel identity and MLS state. The existing owner/create
and scoped management authority remains required. The initiating device signs both
the pinned predecessor identity and proposed successor identity; clients verify
the initiator chain and signed server acceptance. Pending-removal succession needs
a dedicated, validated recovery path, not general removal of the existing guard.
Replacement is not continuity of the original group, and partial local archives
may differ between devices.

Moderation must include account kick, ban/unban, timeouts and authenticated durable
message deletion. Redaction never removes required MLS control messages. Exact
OpenMLS omission limits and safe rejoin must pass independent-client tests before
deletion is activated. Server-side own-message deletion binds authenticated event
authorship without requiring encrypted plaintext.

Notifications originate in client-core after authenticated retrieval/decryption.
Direct, role and everyone mentions are structured message content; encrypted routes
do not disclose them for recipient distribution. Local mention filtering is not
server-enforced mention authorization. Notification preferences, initial-sync
boundaries and persistent deduplication are independent of unread/read state.
Defaults are All Messages with hidden previews; roles never determine notification
access. Actual packaged tests are required per supported desktop platform.

Checkpoint candidates stay outside automatic migrations until every dependent
security gate passes. Schema 15 is rejected by the current schema-14 service. Native
boundary preparation does not itself activate RBAC or establish endpoint coverage.

## Deterministic authority

Roles use stable opaque identifiers independent of mutable names. `$` is presentation
and mention syntax. Built-ins use reserved IDs `builtin:default`, `builtin:moderator`
and `builtin:admin`; custom IDs are UUIDs. The default role is implicitly held by every
accepted account. Admin explicitly inherits moderator grants; priority alone grants
no capabilities and does not inherit every lower custom role. All other inheritance
is excluded to avoid cycles and surprising grant propagation.

The server pre-initializes the three built-in role definitions during database
migration. Opening Roles requires no setup operation. Schema 14 seeds only an
untouched empty catalog and preserves all existing definitions and assignments.
The installed defaults are revision zero, with no invented actor or user audit
event. An older preparatory client's explicit `initialize` request may acknowledge
that untouched baseline once, but cannot reset an edited catalog. Default role
definitions do not activate RBAC or grant explicit account assignments.

Effective server grants are the union of assigned roles, default, and explicit admin
inheritance. Revocation/burn/inactive account/unapproved device gates apply first.
The authenticated owner has management authority independently of roles; this never
supplies an MLS leaf, historical key or exemption from device admission or revocation.
Built-ins cannot be deleted or renamed away from their reserved identities. Positions
are nonnegative, default is zero, and admin must rank above moderator.

For each category/channel layer: apply the default-role override first, then aggregate
matching non-default-role overrides (denies win over allows within that aggregate).
The category result is the input to the channel layer. A category's denied view is a
hard visibility boundary: channel overrides cannot reveal its contents. A denied
channel view removes all read/write/manage/mention access on that channel. Resource
overrides can change only channel capabilities, never global management permissions.
Owner management bypasses role grants but never grants cryptographic participation.

Role administration needs explicit manage/assign capabilities. A non-owner may only
edit/assign/revoke roles strictly below their highest assigned priority; both old and
new positions are checked. They cannot grant capabilities they do not currently hold,
change their own authority, modify an equal/higher account, or manage the owner.
An unprivileged user cannot obtain authority by choosing a display name or role ID.
New custom roles begin empty and non-mentionable. Safety constraints prevent making
default a management role; default role-wide mentions are off initially.

Granting a role requires a currently admitted, non-burned target. Removing a
stored assignment remains possible after the target leaves or loses its valid
devices, including after root burning. Removal does not readmit the target.
Current actor admission, hierarchy, self-modification and owner protections
still apply; a target's loss of admission does not lower its stored role rank.

## Policy storage, concurrency and admission

Policy updates compare an expected revision and serialize with request authorization.
Roles, assignments, overrides, desired MLS access, audit and idempotent response commit
atomically. Authorization is rechecked inside that transaction. Old membership
credentials are not proof of current permissions; every endpoint rechecks durable
policy. Revocation denies newly authorized delivery immediately while MLS removal is
pending. Request ordering across that commit is the server's local serialization
boundary, not a claim that already delivered content can be recalled.

Account admission and device approval are independent from account-root certification.
Private Group defaults to moderator-approved accounts and allows their additional
valid devices. Lockdown requires account acceptance and explicit approval of every
device. Imported presets never silently modify existing channels/devices. Last-admin
recovery uses the established owner designation/acceptance maturity or operator
recovery flow and still requires a valid client account/device chain.

## Wire and activation boundary

The shared evaluator is not an activation flag. New policy proofs, mention metadata
and MLS policy operations require separate versioned deterministic signing objects,
fixed vectors and capability negotiation before runtime use. No V1 signature is
reinterpreted. Older clients retain legacy messaging only while the server remains
in legacy authorization mode. A policy-controlled server must reject unsupported
clients explicitly; it cannot leave a legacy endpoint as a permission bypass.

Role mentions disclose stable role IDs, route/event association and delivery timing
to the server. They do not disclose encrypted bodies. The server authorizes a dedicated
device-authenticated request bound to the accepted event; recipients validate against
decrypted structured tokens. Raw `$text` never causes rejection or notification by
itself. Mention authorization does not bypass recipient channel/device policy.

See [the implementation and acceptance plan](../plans/server-provisioning-rbac.md).
