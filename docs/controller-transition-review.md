# Protected controller: registration transition for review

> **Superseded; do not execute.** The latest direction requires one canonical
> Cords monorepo and no registration changes in this unit. Prior conditional approval
> for a controller-scoped Mac transition is retired. Repository-scoping Mac-mini to
> Cords alone cannot separate PR jobs from approved releases. Follow
> [the monorepo plan](ci-monorepo-plan.md). No transition was applied.

Status: **blocked before activation**. The user selected a protected release-control
project and approved the repository-scoped Mac registration transition, conditional
on validated controller workflows/dependencies, backups and drained jobs. The private
controller repository has been created, with Actions disabled and owner-only branch
and tag rules. No registration, daemon or credential has been changed.

## Verified boundary

The public instance is Forgejo **14.0.5+gitea-1.22.0**. On 2026-10-11 its admin
runner page shows Mac-mini ID 6 and opensuse-server ID 5 both **Global**. Mac-mini's
recent tasks include Cords and `actions/setup-rust`: cross-repository execution is
observed, not merely hypothetical. Its edit page offers description and deletion,
not a repository-scope change.

Forgejo v14.0.5 `models/actions/task.go:284` restricts eligibility by `repo_id` for
a repository runner, or by repository owner for an owner runner. A Global runner
has neither restriction. Labels are evaluated within that eligible set; they are
not an access-control list. Creating a private controller alone does not remove
the current Global runner's eligibility for other repositories.

Versioned primary source, fetched over verified TLS, retained in the isolated
`/home/william/cords-ci-review-20261011` fixture:

| v14.0.5 file | SHA-256 |
|---|---|
| `models/actions/runner.go` | `1bdfe9bc5ed996946d3c4fa25d1319e52ab24eb7743a13a006c0e8fda82c77f8` |
| `models/actions/task.go` | `b1962d92068d3a7eb05d0cb4c74927c5f9f8650a1564a99f6f0ece2c047b605e` |
| `models/actions/forgejo.go` | `cccc443f2bb71c75e02ee39a034b51f5abc84906212d0c5f756fd04b90ce3841` |
| `cmd/forgejo/actions.go` | `1781f573b972e04fd11e3a54333a4a664ade3771e7f6f94d6d33a80b22ab3bf5` |

The CLI `register` can update a record whose UUID derives from the first 16 token
characters. A private, local boolean check on both existing registrations found
their UUIDs **do not match** that derivation. Reusing either token with this CLI
would not safely rescope the existing registration. Do not run that command as
a purported in-place transition or write the live database directly.

Three Mac `forgejo-runner daemon` processes currently use
`/Users/william/forgejo-runner` as working directory. They were inspected, not
stopped. Any approved transition must reconcile/drain all launch definitions and
pollers before replacing registration state; never run old and new identities
concurrently.

The `.54` Docker container named `forgejo` reports **13.0.3**, unlike the public
instance. It is not an established administration target for the v14 public
service. Locate and verify the actual service before any registration operation.

## Approved transition with unmet prerequisites

1. **Repository preparation completed:** private
   [firebadnofire/cords-release-control](https://pubcode.archuser.org/firebadnofire/cords-release-control),
   initial README-only commit `a426842bb8160ee35ac8d29a0fbac91ecd68945a`.
   Validated public helpers were subsequently pushed at GPG-verified commit
   `01235ee48174a051879b96ca4461309b942a62b9`; Actions remains disabled.
   Collaborator list is empty. Actions is disabled (saved setting verified).
   Branch pattern `**` restricts push and merge to `firebadnofire`, disables
   deploy-key push bypass, disallows force push, enforces rules for admins, dismisses
   stale approvals and blocks rejected/outdated merges. Tag pattern `**` allows
   only `firebadnofire`. No workflows, build approvals or credentials installed.
   These are inspected ACL/settings, not adversarial hosted proof. No required
   status contexts or independent-review quorum exists yet. Additional maintainers
   and inherited user/organization secret/token exposure require explicit review.
   That inspection now identifies a **blocking inherited-secret boundary**: v14
   sends all owner/repository secrets to every ordinary job. The owner's shared
   release secrets are present, so this controller's Mac jobs would receive them.
   Repository-scoping the runner does not fix secret distribution. See the
   [required migration review](controller-secrets-review.md); no credentials moved.
2. **Authorized, not executed:** during a controlled maintenance window, drain Mac-mini, privately back up its
   existing launch/registration state, disable the Global ID 6, and register the
   **same Mac-mini daemon and physical host** at controller repository scope.
   This replaces the registration record/ID, not the machine or worker count.
   No source-repository/organization/global Mac registration may remain eligible.
   Verify this through server-side eligibility and hostile disposable jobs.
3. Keep opensuse-server as the ordinary isolated Linux worker. Before removing its
   job-level Docker certificates, establish a controller-managed disposable
   OCI/Compose builder outside untrusted jobs. No publication keys, persistent
   caches or production daemon access in that builder. Its job routing and lifetime
   must be enforced outside branch-controlled workflow steps. A shared TLS daemon
   exposed to an arbitrary job does not satisfy this requirement.
4. Move publication secrets out of Cords' branch-writable workflow scope only after
   a separately reviewed migration. Native Mac build jobs receive none of GPG,
   Cosign, GH_KEY, registry credentials or publication-capable tokens. Verify actual
   token permissions and inherited secrets, not just named step environments.
5. Controller-owned approvals bind an exact Cords commit and protected `v*`/`V*`
   tag. Verify the tag still resolves to that commit and mandatory tests succeeded
   before builds. Do not execute a source-repository workflow as controller code.
   Fetch build outputs by exact controller run/job identity, verify full matrix,
   format/architecture/digests and provenance before privileged signing.
6. Preserve Linux, native macOS, Windows cross-build and both OCI registries.
   Signing, draft staging, mirror verification and retries remain separate protected
   jobs. No release is successful without every mandatory destination and gate.

Approved Mac source still executes as william on a persistent host and can access
that user's files and registration state. Protected dispatch reduces who may run
code; it provides **no OS sandbox**. Review dependencies, disable shared trusted
caches and retain this compromise boundary explicitly.

## Acceptance and rollback

Before activation, prove an arbitrary repository and PR cannot schedule a Mac job
or obtain publication credentials. Use harmless disposable sentinel jobs, never
credential-exfiltration payloads on the live Global Mac. Verify an approved exact
commit can run native packages, and wrong commits/refs/artifacts/cache substitutions
fail closed. Local source review is not adversarial hosted acceptance.

If validation fails, keep affected dispatch/publication disabled. Do not restore
Global native eligibility as an automatic rollback. Preserve original deployments,
queued-job evidence and private backups; restore only a reviewed compatible scope.
Do not delete valid published releases, assets, tags or signatures.

References: [Forgejo v14 runner security](https://forgejo.org/docs/v14.0/admin/actions/security/),
[v14.0.5 eligibility source](https://codeberg.org/forgejo/forgejo/src/tag/v14.0.5/models/actions/task.go),
[v14.0.5 registration source](https://codeberg.org/forgejo/forgejo/src/tag/v14.0.5/models/actions/forgejo.go),
[v14.0.5 CLI](https://codeberg.org/forgejo/forgejo/src/tag/v14.0.5/cmd/forgejo/actions.go).
