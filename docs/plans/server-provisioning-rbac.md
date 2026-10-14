# Server creation, presets, RBAC and role mentions

## Scope and current state

The 2026-10-12 user request authorizes all ten implementation areas below. This
plan extends the focused role-access work; it does not activate unrelated rework
features or release publication. The canonical repository remains `firebadnofire/cords`.
Existing staged picker-crop changes belong to the preceding request and are preserved.

Current production uses device-certified memberships, fixed capability lists,
owner-only channel creation, PostgreSQL schema 12, explicit MLS roster requests,
and immutable signed public/encrypted channel identities. Roles/category permissions,
role mentions, per-account admission policy and SSH provisioning are not implemented.
No VISION.md is present. AGENTS.md, KEY-SYSTEM.md and accepted ADRs remain normative.

## Implementation sequence and acceptance gates

Every numbered unit follows plan → implement → validate. Failed validation blocks
the next dependent unit. Missing runtime evidence is recorded as incomplete.

1. **Authorization contract and engine.** Define stable role IDs, capability mapping,
   explicit built-in inheritance, hierarchy limits, overrides, device admission and
   preset defaults. Implement the shared deterministic evaluator and adversarial
   tests. Add versioned, transactional persistence with schema constraints and
   migration tests against real PostgreSQL. Runtime activation remains off until
   all request/delivery paths and independent MLS clients pass integration.
2. **Role administration.** Current-state authorized CRUD, assignment, list and audit
   endpoints, client-core and native commands, functioning editor/member UI. Test
   durable restart, idempotency, concurrent edits, stale credentials, self-elevation,
   owner protection and stable IDs across rename. Unsupported capabilities stay out
   of actionable controls. Use `$` for display; ownership is never a role.
3. **Channel/category access and MLS.** Apply one evaluator to listing, direct lookup,
   history, live notices, welcome, KeyPackages, roster operations and commits. Persist
   desired access separately from the actual MLS roster. Reconcile with the existing
   single pending roster operation constraint. Test denied metadata, every alternate
   API route, immediate delivery denial, missing packages, concurrent flips, independent
   clients, offline catch-up and restart. Introduce authenticated policy proofs through
   a negotiated new operation contract, never alter old signing bytes.
4. **Account/device admission.** Persist separate decisions. Community/Private Group
   admit additional root-authorized devices of admitted accounts; Lockdown requires
   explicit device approval. Test pending/rejected/stale/revoked devices and valid
   independent devices. Reuse owner succession/operator recovery; recovery never
   gives the server client secrets or bypasses device certificates.
5. **Role mentions.** Structured stable-ID tokens and accessible composer suggestions;
   dedicated authenticated notification metadata bound to the exact accepted event.
   Server checks current sender/recipient permissions, target mentionability, dedup
   and rate budgets. Recipient compares metadata with decrypted structured mentions.
   Preserve ordinary `$text`, escapes/code/currency, legacy message decoding, previews
   off by default and channel confidentiality. Protocol vectors precede activation.
6. **Preset initialization.** Persist Community/Private Group/Lockdown policies and
   create their real channel/category templates. Owner client creates MLS groups;
   server never participates. Test all presets with independently initialized clients.
7. **Native SSH/Docker provider.** Native transport with strict host-key verification,
   agent-first authentication, explicit unsupported auth modes, no credential storage
   in server records. Read-only host checks precede an exact deployment manifest.
   Enforce pinned images and trusted signature policy, unique install/container/volume
   identities, resumable journal, safe failures, and TLS readiness. No destructive
   rollback. Actual deployment requires an explicitly authorized verified target.
8. **Create wizard.** `+` opens Join/Create; existing join is reused. Wire all steps to
   the native provider, expose readiness/progress/failures, then securely consume the
   existing single-use owner claim, verify pin and connect. Test packaged UI against
   a disposable Linux deployment; source mocks alone do not pass.
9. **Preset changes.** Review and confirm durable transition plans. Changing channel
   mode creates new identities using existing succession; no history re-encryption.
   Enabling Lockdown fences existing devices until explicit review. Loosening device
   policy requires confirmation. Test interruption, restart, duplicate approval and
   irreversible-side-effect boundaries.
10. **Integrated acceptance/documentation.** Run all 24 requested cases, real PostgreSQL,
    independent TLS clients, packaged UI, migration from schema 12 and repeated startup.
    Record commands, artifact hashes, environment and explicit limitations. Update
    operator/user guides and recovery procedures before calling the task complete.

## Environment and operational limits

Development is Windows/PowerShell. The verified existing SSH path to Linux `.54`
works with BatchMode and StrictHostKeyChecking; it reports Linux x86_64 and Docker
29.7.2-ce. This is inspection evidence, not approval to replace its production server.
Use an isolated database/container for persistence acceptance. C: has roughly 4 GiB
free; existing target artifacts may be reused and new large output belongs on D:.
Do not copy signing/private identity keys to deployment targets. Privileged release
publication remains disabled under its independently incomplete security gates.

## Compatibility and rollback

Keep legacy routes and signed records intact. Store policy changes with expected
revision, authenticated actor, idempotency key and atomic audit entry. Stale writes
return conflict, never last-writer-wins. After policy activation, old binaries that
ignore role/device restrictions must refuse the schema; rollback uses compatible
code or repair forward. Never rewind live MLS/identity/owner state. Pre-migration
backups and matching server keys are recovery exercises on isolated copies only.
No data resets, production deployments, repository deletion or signing-policy changes
are implied by implementing this plan.
