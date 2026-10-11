# Publication preparation: repository-only units

> **Superseded architecture.** The user now requires one Cords monorepo and an
> external signing execution/credential boundary. Do not activate the separate
> publication repository or migrate credentials using this former plan. Follow
> [the monorepo plan](ci-monorepo-plan.md). Preserve this document as history.

Authorized scope, 2026-10-11: prepare a separate protected publication repository,
complete owner-secret consumer inventory, release handoff, workflows and migration
review. No Actions activation, secret relocation/deletion, live credential changes
or runner registration changes are authorized in this unit. C0 remains open and
C1 inactive. Existing source/runtime and unrelated services remain unchanged.

## Plan

1. **Inventory:** inspect the authenticated owner's complete repository list and
   current secret names; read every advertised branch/tag's workflow/script source
   without checkout or executing repository code. Record immutable Git refs, direct
   secret names, dynamic access/reusable workflow uncertainty and owner inheritance.
   Validate pagination, scanner negative fixtures and complete scan accounting.
   Unreadable refs, external consumers and unknown names block migration.
2. **Publication repository:** prepare private `firebadnofire/cords-publication`,
   Actions disabled, no collaborators, owner-only protected branches/tags, no private
   credentials and no native build jobs. Copy only validated trusted public pins,
   signer/verification helpers and documentation. Verify saved settings, exact copy
   tests, GPG-signed commits and remote commit identity before recording completion.
3. **Release handoff:** define strict approved manifest and signed validation
   receipt contracts binding protected Cords tag/commit, controller run/job/workflow
   identity, complete package matrix and immutable OCI digests in both registries.
   Prepare a read-only verifier with independent pinned authorization and validation
   evidence; reject wrong identities, missing gates, substitutions and malformed
   input. Do not implement a privileged download/sign/publish path until provenance,
   token and host gates are independently validated.
4. **Migration review:** map each owner input to every observed consumer and proposed
   restricted destination. Preserve shared key bytes and other projects. Define
   consumer tests, secure value-entry/recovery prerequisites, explicit approvals,
   sequencing and rollback fences. Prepare dry-run-only migration checks; no secret
   values accepted, exported or written by the review tooling.
5. **Record validation:** inventory coverage and limitations, repository ACLs,
   manifests/tests/helper versions and commands. Keep publication blocked on Linux
   isolation, owner inheritance, private/public correspondence, actual adversarial
   dispatch/provenance tests and native GUI acceptance.

## Environment and security constraints

Windows/PowerShell source work uses the existing Git HTTPS credential helper and
GPG commit-signing setup. Inventory clones are bare, isolated and non-executing;
never run another repository's scripts. Native Linux fixture tools provide real
GnuPG/ShellCheck validation. The existing Mac stays build-only and receives no
release credentials. The two existing physical workers are retained.

Forgejo v14 distributes owner plus repository secrets in every ordinary task;
repository separation alone does not protect the Mac or subsequent Linux jobs.
The Linux runner's global Docker certificates, host networking, persistent state
and cache remain an unmet boundary. No branch-controlled condition or ignored
`permissions` setting substitutes for server scope or executor isolation.

All units follow plan → implement → validate. Failed checks stop their unit for
diagnosis. Changes are repeatable and reviewable; no production tag/release/registry
mutation is part of preparation. Keep prior published bytes and live authority
state; recover forward rather than restoring insecure owner-wide exposure.
