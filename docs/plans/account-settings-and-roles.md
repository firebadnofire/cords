# Focused account settings and role access

The user authorizes these focused functional changes after the completed C0 baseline.
This is not activation of the entire C1–C8 milestone or privileged publication.

## Plan

1. Verify the existing account isolation, password wrapping, local archive and roster
   contracts. Read the full FireBADnoFire error from the running client before choosing
   a recovery behavior. Do not alter personal vault data or discard ambiguous outboxes.
2. Add Settings → Security password change with current-password authentication,
   confirmation, fresh Argon2id salt and atomic replacement of the existing wrapped
   master key. Preserve account/device identity, MLS, history, archives and server trust.
   An older backup still has its own password wrapper; changing a password is not key
   revocation or forensic erasure. Legacy OS-protected migration remains separate.
3. Move retained channel archive navigation into Settings → Local history. Show archived
   server bookmarks separately and accurately; they are not complete server histories.
   Viewing remains account-scoped, read-only and offline. Make errors fully readable.
4. Diagnose and reproduce the roster failure. Change retry handling only for explicitly
   unambiguous responses; accepted/uncertain roster operations must remain durable.
5. Record a scoped role-access ADR before introducing server role contracts. Implement
   roles and account assignments, then channel role access, then automatic verified-device
   reconciliation and UI, in individually tested vertical units. Role assignment queues
   encrypted access automatically, as approved by the user. Missing KeyPackages or an
   unavailable authorized committer remain visibly pending. No ACL-only MLS bypass.

## Validation and rollback

Each unit passes targeted tests and appropriate existing regressions before the next
unit. Password tests cover wrong/short passwords, SQL failure, repeated changes,
restart, preserved identity/key/history/MLS/outbox and a separate unaffected account.
UI tests cover Settings navigation, confirmation, native callbacks and error handling.
Run frontend diagnostics/build and Rust checks, then the affected packaged desktop
path. Record commands, artifact hashes and limitations in a dedicated report.

Role/storage changes additionally require disposable PostgreSQL tests, stale-credential
denial, owner protection, independent two-client TLS messaging and restart tests.
Do not claim packaged or cross-platform evidence from source tests. Stop a failed gate
and diagnose it; no activation on missing integration evidence.

Password rewrap uses the existing storage format. Do not rewind an operational vault
to undo a password change; authenticate and rewrap forward. Navigation/CSS can roll
back independently. Define role migration/activation fences in the scoped ADR before
emitting any new authorization records. Preserve functional predecessors until each
replacement passes acceptance. Signing/publication remains disabled.
