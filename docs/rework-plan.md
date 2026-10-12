# Cords rework: checkpoint implementation and evidence plan

Approved scope: visual/interactions, native identity lifecycle, hardware/OpenPGP,
administration, message actions/DMs, attachments, encrypted reports, local settings
and integration. [ADR 0007](adr/0007-rework-security-contracts.md) establishes the
security contracts, including the three final compromise/deletion clarifications.
The general [IMS checklist](../CHECKPOINT.md) remains separate and is not checked
off by this milestone plan.

## Plan

Each lettered unit follows plan → implement → validate before the next dependent
unit. Keep the existing messenger runnable; add replacements alongside functioning
paths and activate only after UI/native/server/persistence acceptance. Preserve
unlock, trust/authentication, public/encrypted messaging, synchronization, channel
lifecycle, archives and ownership recovery. No unsupported events enter legacy
decoders. Missing evidence leaves a gate incomplete; an architecture conflict stops
the affected unit for an explicit decision, not an invented resolution.

Defaults remain 14 px comfortable UI, Steel + Blue semantic tokens, Requests DM
policy, notification previews/sounds off, configurable 100 MiB attachments with
bounded streaming, bounded selective reports, least-privilege invites and no automatic
historical-key recovery. Calls, presence/typing, integrations/bots and full conversation
export remain deferred. No additional feature scope is selected.

## Implementation and acceptance

| Unit | Smallest coherent implementation and acceptance | Rollback boundary |
| --- | --- | --- |
| C0 | Security ADR/contracts; current wire/storage audit; semantic concurrency/finality/archive/compatibility scenarios; executable existing-signature/migration regressions; recorded packaged two-client baseline | No new runtime formats; runtime unchanged |
| C1 | Steel + Blue tokens, accessible themes/accents, consistent controls, focus/menu/dialog handling, member search/sort, readable IDs, responsive layouts; exercise all currently functional controls and archive paths | UI rollback with unchanged native contracts |
| C2a | Scoped protocol decision and byte vectors first; fork-preserving root storage, terminal revocation/burn evidence, reconciliation and genuine legacy migration; branches, withheld ancestry, omitted branches, duplicates, crashes/restart | Emitted/accepted new security records fence downgrade |
| C2b | Device-only enrollment, verified request/response transfer, remote authority operations, device lists, revocation propagation/MLS removal; independent vaults/device keys | Preserve certificates, revocations, heads, MLS; disable new operations rather than downgrade |
| C2c | Separately confirmed root move/duplicate and encrypted root-only recovery; inject faults before/after recipient acknowledgement and source custody removal; prove enrollment never transfers roots | Acknowledged custody changes cannot be undone with snapshots |
| C3a | Native hardware provider; independently identify hardware/firmware/middleware/platform/PIN/touch; real compatible-token lifecycle/absence/lock/restart tests | Software accounts work; hardware accounts never fall back |
| C3b | Optional embedded OpenPGP attestation verification; tampering/context binding/burn rejection independently tested | Preserve native authority and separate keys |
| C4a | Server name/description/icon; mutable channel name/topic/category/order separate from immutable identity; restart/sync/succession notices/public acknowledgement/offline archives | Reverse presentation with a new authenticated update; never older lifecycle heads |
| C4b | Roles/constraints/channel overrides integrated with current endpoint authorization; stale credentials, owner protection, immediate delivery denial/pending MLS exclusion | Preserve current permissions and generations |
| C4c | Least-privilege invite lifecycle; proof-bound concurrent consumption, expiry/revocation/retry | Preserve consumed/revoked capabilities and membership generations |
| C4d | Kick/ban/timeout/audit; enforce current authority at each endpoint and immediate delivery denial/pending MLS exclusion | Preserve bans/audit/current membership; no rollback restores access |
| C5a | Immutable replies/edits/deletes/reactions/mentions, deterministic materialization, retry and selective evidence; author/route binding/replay/order/restart/legacy history | Explicit route-format activation fence; retain accepted events/current MLS |
| C5b | Metadata-only Requests/Open/Closed and blocking; no unauthenticated identity or policy leakage | Preserve accepted consent/block state |
| C5c | Real two-account MLS DM delivery; independent devices/offline catch-up/crash recovery/malicious courier | Preserve DM identity and cryptographic state; no rollback recreation |
| C5d | Authenticated courier migration; stable DM identity across two servers, consent and crash recovery | Preserve accepted courier/control state; no implicit file/history transfer |
| C6a | Encrypted bounded streaming/resumption; authorization, hashes/association/corruption/truncation/quotas/cancel/restart/offline files | Preserve object modes/manifests/keys/resumable state |
| C6b | Explicit public plaintext attachment mode with permission-gated retrieval/deletion and signed association | Never reinterpret encrypted objects as public |
| C7a | Report authority commits/revocation/handoff/log replay first; partitions, equivocation, backup continuity, owner succession/burn/rotation | Authority commits and issued approvals fence rollback; conflicting recovery pending |
| C7b | Recipient encryption and selective evidence; substitution/replay/hidden revocation/disclosure bounds/no server plaintext | Preserve report keys/state; no automatic historical moderator access |
| C7c | Selection/preview/submission/inbox UI through verified native paths; confirm exact disclosure and recipients | Preserve pending encrypted reports and authorization state |
| C8 | Persist accessibility/density/font/accents/notifications/unread/block/save-discard; remove deferred placeholders only after replacement acceptance; complete desktop workflows | Preserve old preference values; remove functional predecessors only after gate |

C4–C7 are vertical slices. Each subcheckpoint integrates real UI/native/server paths;
there is no backend batch followed by a final UI-only validation pass.

## Validation required at every gate

- Targeted protocol/crypto/storage tests, frontend diagnostics/interactions, Rust
  checks and relevant existing regressions.
- Real disposable PostgreSQL for server persistence changes; skipped/ignored tests
  are not passes. Do not run destructive restart scripts against a live community.
- Migration from the preceding checkpoint; repeated startup/interrupted migration;
  preserved IDs/history and incompatible-version rejection. Fixture-only tests do
  not prove live sealed-vault or protocol continuity.
- Packaged desktop execution of the affected path. Messaging gates require independent
  clients over real TLS; mocked component and CLI results supplement that evidence.
- Hardware and Windows/Linux/macOS results recorded separately. Missing evidence
  is incomplete. Production activation stays disabled until acceptance passes.

## Migration and recovery record required per unit

Record source revision/artifact hashes, starting and resulting schema/protocol versions,
activation fence, backup set (matching database, server signing key, account vault and
external token metadata where relevant), compatible binaries, repeated/interrupted
restart behavior, and isolated restore/forward-repair instructions. Retain externally
observed heads, tombstones, counters, keys, outboxes and cursors through code rollback.
Never restore stale live security state. C0 changes no runtime storage or signing bytes.

## C0 implementation plan

1. Audit normative decisions and current contracts; record ADR 0005's scoped burn
   supersession and all final clarifications without changing behavior.
2. Add semantic acceptance scenarios with explicit future-gate status. Execute current
   public signature vectors against typed decoders/verification and domain/tamper rejection.
3. Exercise schema 2 → 3 SQLite migration with known opaque bytes/IDs, interruption,
   transactional rollback and repeated restart; rerun native sealed-vault regressions.
4. Run frontend checks/build/interactions, Rust checks/tests, dependency policy, real
   disposable PostgreSQL and independent TLS-client regressions. Record failures honestly.
5. Record current packaged two-client/migration evidence and artifact identity. Advance
   C1 only after C0's gate is met. Old smoke reports remain historical evidence.

Evidence/status: [C0 record](reports/rework-c0.md). No future gate is marked complete
by the acceptance manifest or by this plan.

C0 acceptance completed on 2026-10-11 with separately recorded native packaged
Windows, Linux and macOS evidence. C1 and later checkpoints remain inactive under
the user's C0-only instruction; release publication has separate unresolved gates.
