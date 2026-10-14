# ADR 0008: Account roles and server policy

- Status: Implementation contract under the explicit 2026-10-12 product request;
  activation requires the integration gates in the implementation plan.
- Supersedes ADR 0006's deferral of public-channel role overrides only when the
  role-policy capability is activated. Confidentiality and lifecycle remain immutable.
- Extends AGENTS.md section 10 with persistent account roles. Existing capability
  strings retain meaning; owner-only transitions remain owner-only.

## Deterministic authority

Roles use stable opaque identifiers independent of mutable names. `$` is presentation
and mention syntax. Built-ins use reserved IDs `builtin:default`, `builtin:moderator`
and `builtin:admin`; custom IDs are UUIDs. The default role is implicitly held by every
accepted account. Admin explicitly inherits moderator grants; priority alone grants
no capabilities and does not inherit every lower custom role. All other inheritance
is excluded to avoid cycles and surprising grant propagation.

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
