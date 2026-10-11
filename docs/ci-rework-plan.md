# CI/CD rework: implementation and activation gates

> **Superseded architecture, retained for historical evidence.** The user's
> 2026-10-11 correction requires the single `firebadnofire/cords` monorepo.
> Follow [the current monorepo plan](ci-monorepo-plan.md). The controller/publication
> repository and registration proposals below must not be activated.

Status: in progress, not activated. C1 and later product checkpoints remain inactive.
This is the CI/CD scope explicitly requested on 2026-10-11, alongside C0 acceptance.
Unit 1 passes local contract, real GnuPG fixture, shell syntax and ShellCheck gates;
see [recorded commands/results](reports/ci-rework.md). Hosted evidence is pending.

## Plan

Preserve the messenger, protocol/storage formats, deployment, and existing packaging
scripts. Work in the following independently validated units. No production release,
tag mutation, runner reconfiguration, or credential relocation is part of a local test.

1. **Release contracts:** strict `v`/`V` SemVer validation; expected public signing
   identity; armored signatures for every input asset and `SHA256SUMS`; read-only
   verification; negative/retry fixtures. Local fixture keys are not release keys.
2. **CI execution boundary:** establish isolation for untrusted work on the two
   existing runners before enabling all-branch/PR execution. Test immutable event
   checkout, Rust/frontend/security checks, disposable PostgreSQL, and existing
   independent-client fixtures. Required aggregate checks fail on unavailable jobs.
3. **Build dependency graph:** all release builds depend on the complete mandatory
   validation gate. Reuse Linux client/server, Windows cross-build, Mac universal
   DMG/native PKG/server, and OCI scripts. Validate complete inventory, package
   contents/architecture, lockfiles, event commit, and collected provenance before
   signing. Linux ARM builds must execute on opensuse-server; its existing QEMU
   capability needs an actual build gate, not an assumed native ARM claim.
4. **Privileged authority:** verify trusted tag authorization outside arbitrary
   branch-controlled scripts before exposing secrets. Pin public GPG/Cosign
   identities independently of imported private keys. Review branch/tag protection,
   workflow protection, secret availability, daemon access, caches, and job tokens.
5. **Publication:** draft both file releases; publish/verify immutable OCI digests
   independently in both registries; reconcile existing identical assets without
   replacement; reject moved tags or different bytes. Reuse pinned gh-sync with
   the smallest necessary scoped extension/integration. Verify all downloaded
   files, signatures, metadata and tag commits before finalization. Document
   non-atomic partial finalization and repair/retry without automatic deletion.
6. **Hosted acceptance:** main/feature/PR, lowercase/uppercase tags, negative
   checks, failed mandatory jobs, completeness, duplicate execution, and publication
   failure in an isolated namespace. Native Mac-mini runner evidence and independent
   macOS package/runtime acceptance remain separate required gates.

Validate each unit before proceeding. Shell syntax, ShellCheck, contract tests,
YAML and extracted shell validation supplement hosted runs; none replaces them.
No activation on an unmet isolation, authority, build, or acceptance gate.

## Inspected implementation and environment

- Forgejo API reports **14.0.5+gitea-1.22.0**. Inspect v14 behavior, not assumptions
  borrowed from GitHub Actions.
- opensuse-server runner registration ID 5; daemon configuration labels
  `ubuntu-22.04`, `ubuntu-latest`, `debian`, `alpine` use Docker executors.
  Registration labels differ from the daemon configuration; hosted routing must
  still be validated. Capacity 1, host networking, TLS Docker client certificates
  mounted into every job, shared daemon access and persistent cache are present.
  `container.docker_host: "-"` alone does not isolate jobs because the runner also
  supplies `DOCKER_HOST` and client certificates. Untrusted jobs can control that
  daemon. No runner setting has been changed.
- Mac-mini ID 6: `macos`, `mac-mini`, `macos-latest` are **host** executors;
  `arm-ubuntu-22.04` is a Docker Linux executor, not native macOS. Native host jobs
  execute as william alongside persistent SSH/runner state. No runner change made.
- Native Mac inspection and SSH trust evidence are in `reports/rework-c0.md`.
  Disk capacity is now sufficient. The repaired native PKG passes strict bundle
  verification and independent startup smoke tests. Graphical acceptance remains
  blocked by unavailable Accessibility/display access; no TCC settings changed.
- Existing release workflow allows manual branch releases, does not require tests
  before builds, publishes before mirror verification, and automatically replaces
  assets. Existing signing selects the first imported key and produces only `.sig`.
  These are current shortcomings, not accepted design.
- Read-only gh-sync checkout at
  `d879ea006b4cf0ed91659727d76fc6f1551d4fbe`: supports drafts and overwrite=false,
  but lacks exact target-commit binding, metadata updates, downloaded digest
  reconciliation, and matching-asset retry. No shared project modification/push.

## Decisions required before affected units

The user selected the **protected release-control project** direction, with
repository/controller implementation first and host/registration/credential changes
prepared for review. This is an authorization boundary, not an OS sandbox for
approved native code executing as william on the persistent Mac.
Live registration inspection and Forgejo v14.0.5 source establish that both runners
are Global. Current registrations cannot restrict Mac jobs to the controller.
Stop affected activation and prepare the registration transition; do not approximate
scope with labels or workflow conditions. The user subsequently approved executing
that transition only after controller workflows/dependencies, secure backups and
job draining pass. Those prerequisites remain incomplete; no registration changed.
See [the concrete transition review](controller-transition-review.md).

The user authorized **preparing isolation changes for review**, not applying them.
Prepare concrete configuration deltas, resource/credential boundaries, migration and
rollback, and acceptance gates before requesting activation. Preserve live settings.
Workflow conditions cannot secure a compromised persistent runner or prevent another
branch workflow requesting repository secrets. Do not substitute a branch-controlled
boolean for an execution/secret boundary.

The user supplied authoritative shared public identities: `https://archuser.org/gpg.key`
and the bazzite-firebadnofire project's published `cosign.pub`. This unit proceeds:
retrieve with verified HTTPS, validate certificate/key formats and self-signatures,
pin the full GPG primary fingerprint and exact public files/hash/source URLs in
version control, verify imported private identities before any signing or publication,
and exercise production public pins plus supplementary mismatch fixtures. No runtime
key retrieval or automatically accepted URL replacement. Rotation requires review.

macOS disk inspection now reports 23 GiB free; begin isolated native packaging with
bounded build parallelism. Graphical access remains a distinct pending runtime gate.

Native packaging exposed a C0 integrity defect: the ARM linker-created executable
signature does not seal the `.app` resources, and strict bundle verification fails.
Remediate this unit using Tauri's supported explicit ad hoc bundle signing default,
preserving any explicit Developer ID environment override. No certificate/private
key or quarantine/TCC override is involved. Require strict codesign verification
of the bundled app and extracted PKG payload before package acceptance. Rebuild
native PKG/DMG, verify architectures/resources/hashes, and run startup separately.
Ad hoc sealing is neither Developer ID distribution signing nor notarization.

The next repository-only unit stages the already validated public-pin, signing,
read-only verification and release-contract helpers in the protected controller.
Keep Actions disabled, install no credentials and copy no unfinished publisher.
Validate the exact controller copy with real disposable GnuPG fixtures before
committing/pushing it. Controller signing jobs must eventually execute these
trusted helpers, never substitute scripts or public pins from the Cords build
commit. This unit does not activate a release workflow or satisfy runner isolation.

That staging unit is now published at GPG-verified controller commit
`01235ee48174a051879b96ca4461309b942a62b9`, with Actions still disabled.
The subsequent secret-scope audit blocks the proposed combined build/signing
repository: Forgejo v14 sends all owner/repository secrets in every ordinary job's
task payload, and this owner has shared GPG/Cosign/GitHub credentials. Do not
activate or migrate Mac-mini while those inputs would reach it. Prepare a
secret-free build controller, separately protected publication scope and complete
consumer-preserving owner-secret migration for explicit review. No new publication
repository, credential movement or server policy is assumed authorized by this
finding. See [concrete prerequisites](controller-secrets-review.md).

Sources: [Forgejo v14 runner security](https://forgejo.org/docs/v14.0/admin/actions/security/),
[Forgejo v14 workflow security](https://forgejo.org/docs/v14.0/user/actions/security/),
[SemVer 2.0](https://semver.org/),
[inspected gh-sync revision](https://pubcode.archuser.org/actions/gh-sync/src/commit/d879ea006b4cf0ed91659727d76fc6f1551d4fbe).

## Rollback

Local source changes are reviewable and reversible before activation. Retain existing
packages and deployments. Once an external tag, release or OCI digest is published,
never rewind or replace its identity/bytes; fix forward or explicitly resume staging.
Keep public verification identities available across code rollback. An older signing
workflow that silently chooses an imported key is not a compatible rollback path.
