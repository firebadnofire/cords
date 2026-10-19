# ADR 0012: Server-owned presentation

Status: Accepted under the explicit October 2026 server-profile implementation request.

Server names, descriptions and profile background colors are server-owned presentation, independent of the pinned
server identity, account profile history, admission and conversation encryption. The
authenticated `server-profile-v1` API returns a domain-separated signed profile. Updates
require current administrative authority inside the same transaction as the write, a
reviewed revision and a device-scoped idempotency key. Retries cannot overwrite a newer
revision or reuse a key for different content. Revision, actor and commit time are audited.

The initial revision has no override and discovery retains the operator-configured name.
Saved names propagate to freshly signed discovery without altering the configured join
policy. Clients cache verified profiles inside their account vault and reject rollback or
conflicting equal revisions. Background presentation work remains lower priority than
message synchronization. Older servers explicitly report unsupported editing.

The normal schema advances from 15 to 17. Schema 16 remains the previously reserved,
unsupported staged RBAC schema. This change does not activate that policy or broaden
MLS membership. The staged migration chain advances to 18 and remains rejected by the
normal server's startup check. Existing applied migration files are not rewritten.

Names are limited to 100 Unicode scalar values, descriptions to 2000. Backgrounds use
opaque six-digit RGB colors and render separately from theme-controlled text. These
fields contain no credentials.

The server icon is optional canonical JXL, at most 5 MB and 1024 by 1024 pixels.
Its digest, dimensions, byte count and presentation shape are signed in the same
profile revision. Icon upload/removal shares the profile's revision and audit rules;
text edits preserve the icon. The server stores only the current image. Its bytes
are presentation data visible to the server and admitted members, not encrypted
conversation content. Removing an icon cannot erase copies already delivered.

Upload authority is checked before reading the bounded request and again after
codec validation, inside the commit transaction. A shared codec permit bounds the
whole upload, including cancellation; expensive decoding holds no identity/policy
locks. Retrieval requires current ViewServer authority and the current signed hash.
Clients verify that hash and reuse the sealed per-account media cache. Qt snapshot
icon data is limited to 16 MiB with the selected server prioritized; other rail
entries fall back to initials when the full animated media exceeds that budget.
