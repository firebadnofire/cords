# Cords monorepo CI/CD plan

Status: repository-side preparation; privileged publication disabled. C0 remains
incomplete and C1 remains inactive. The user's 2026-10-11 single-repository correction
supersedes the earlier controller/publication repository and Mac registration plans.

## Plan

The sole canonical Forgejo repository is **firebadnofire/cords**. Source, Actions
workflows, build scripts, tests, release requests/manifests, public signing pins and
documentation live here. An independently protected executor is an execution
environment, not another Cords repository. No host, registration, owner-secret,
deployment or release-history mutation is authorized in this implementation unit.

Proceed through these units, validating each before proceeding:

1. **Consolidation and boundaries.** Inventory both unwanted repositories at exact
   commits, compare public helper bytes with Cords, preserve unique review evidence,
   and record removal prerequisites. Keep Actions disabled; delete neither repository.
   Acceptance: every tracked file accounted for, no missing useful configuration,
   current plan/references consistent. Rollback: documentation only.
2. **Publication stop and inactive workflow preparation.** Preserve the old release
   definition outside Actions discovery; replace the active publication definition
   with an explicitly skipped gate. Prevent this checkout's existing CI definition
   from scheduling direct native Mac jobs. Prepare all-branch/PR testing and exact
   event-commit checkout alongside it; do not activate expanded execution on unsafe
   runners. Preserve Linux, native Mac, Windows cross-build and OCI requirements.
   Acceptance: workflow checks prove no active release job, release secret expression
   or native Mac dispatch; proposals preserve triggers/commands and reject moved
   event commits. Skipped jobs are not accepted tests. Rollback cannot restore
   privileged publication/direct untrusted host execution before isolation passes.
3. **Release handoff contract.** Implement a read-only, bounded candidate-request
   validator for strict v/V SemVer, canonical origin/repository, exact commit,
   complete package/checksum inventory, both OCI digest destinations, mandatory
   test/build evidence references and public-pin hashes. Test missing, duplicate,
   substituted and malicious inputs. These are untrusted candidates, not proof of
   authorization. No request boolean can authorize signing. Acceptance: negative
   fixtures/repeat validation pass without mutation, secrets or publication.
   Rollback: no external side effects or emitted authority.
4. **External executor review.** Keep its specification/policy in Cords, activation
   off. Specify independently installed reviewed policy, protected credentials,
   trusted evidence acquisition, immutable intake, provenance, durable retries and
   manual signing fallback. Review Linux OS isolation before installation.
   Acceptance: a malicious branch cannot select signer executables, modify installed
   policy, authenticate evidence or obtain credentials. Real service/runner/hostile
   dispatch evidence is required; documents/fixtures cannot pass this gate.
5. **Consumer-preserving credential and runner migration review.** Finish read-only
   owner-secret inventory, including unresolved dynamic/external consumers. Propose
   mapping and approvals; change no secrets. Validate an isolated OCI/Compose path
   before applying the Linux Docker-certificate removal patch. Validate an enforced
   native macOS executor on the same Mac-mini before enabling native jobs.
   Repository-scoping it to Cords alone does not separate PRs from releases.
   No registration transition is performed now.
6. **Activation and hosted acceptance, separately approved.** All branch pushes and
   PRs run mandatory isolated tests. Tag builds depend on successful tests for the
   exact commit. Build Linux AMD64/ARM64 client/server, Windows AMD64/ARM64 cross
   packages, native Mac universal DMG/ARM64 PKG/server and OCI AMD64/ARM64.
   The executor independently verifies authorization/provenance, signs every file
   with GPG ASCII-armored detached signatures, signs exact OCI digests with Cosign
   in Forgejo and GHCR, stages Forgejo/GitHub drafts, verifies downloaded bytes and
   mirror identity, then finalizes. Test missing/failed checks, moved tags, forged
   evidence, substitutions, retries, unauthorized PRs, cache poisoning and partial
   publication recovery. Never auto-delete/overwrite conflicting assets.
   Cross-registry finalization is non-atomic: retain evidence and repair forward.

## Implementation boundaries

Forgejo **14.0.5+gitea-1.22.0** is the inspected public service. Ordinary task
payloads include applicable owner/repository secrets irrespective of `env` use.
Job `permissions` is ignored. Global runners admit unrelated repositories;
repository registration limits repositories, not trusted ref classes within one
repository. Labels, conditions and working-directory changes are not isolation.

The existing opensuse-server runner gives jobs Docker TLS credentials and shared
daemon/cache access. Its removal patch stays unapplied until disposable OCI/Compose
replacement tests pass. Native Mac jobs run as william on a persistent host. An
enforced host-controlled disposable macOS guest executor must be validated, or
native builds remain independently approved/manual. A shell wrapper or VM started
by an untrusted workflow is insufficient. Preserve both physical runners and services.

No new publication credentials enter Actions. Existing owner secrets cannot be
moved/deleted until other consumers are accounted for and migration is approved.
Their inherited exposure blocks expanded ordinary CI. The signer must be outside
job UID, daemon, mounts, caches and network-control boundaries; another directory
on the same host is insufficient.

Public pins remain the user-authorized shared keys. Rotation requires review, not
a changed URL or artifact. The executor uses its independently installed pin/policy
revision; repository commits cannot automatically update privileged service code.

## Validation

Record commands, revisions, tools, results and limitations in
[the CI report](reports/ci-rework.md). Local contract/security fixtures and YAML/shell
checks supplement hosted runs. Tests before builds, native package execution,
credential denial and authenticated provenance require integration proof. Missing
evidence blocks activation; product runtime/contracts remain unchanged.

See [external executor specification](external-release-executor.md),
[scaffolding inventory](reports/ci-monorepo-consolidation.md),
[runner review](runner-isolation-review.md) and
[owner-secret review](controller-secrets-review.md).
