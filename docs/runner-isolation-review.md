# Existing runner isolation: review proposal

> **Current direction: one Cords monorepo.** The protected-project alternative
> below is superseded and must not be implemented. Keep both physical runners;
> validate an enforced host-controlled macOS guest executor or independently
> approved native execution before enabling native jobs. Cords repository scope
> alone cannot separate PRs from tags. The external signer requires a separate
> protected OS boundary on the Linux host, not merely another UID/directory whose
> state the current job-controlled Docker daemon can reach. No host changes in this
> unit. See [the monorepo plan](ci-monorepo-plan.md).

**Prepared only. No live changes applied.** The user authorized preparation for
review on 2026-10-11. Existing registrations, deployed services, credentials and
unrelated projects remain untouched. This proposal does not claim the complete
CI/CD activation gate is satisfied.

The user subsequently selected the protected-controller option and conditionally
approved transitioning the same physical Mac-mini to repository registration.
Initial private repository preparation is complete; the workflow/dependency,
backup, drain and hosted acceptance prerequisites are not. The Linux configuration
patch remains unapplied. See [transition evidence](controller-transition-review.md).

## Observed environment

opensuse-server: registration ID 5, Forgejo Runner **v12.13.2**, capacity 1,
Docker executor, TLS-enabled dedicated DinD. Inspected configuration SHA-256:
`f8ef30f179d5479aef72ec9b24f40260e84cf785879688e7174ffea4ea19e1f7`.
Docker client certificates are currently mounted into every job, control variables
are supplied globally, network is host and cache is persistent. The production
Docker daemon is separate, but jobs can control the CI daemon and its state.

Mac-mini: registration ID 6, arm64 macOS 26.6.2. Installed runner reports **dev**;
this is not evidence of an identified maintained release. `macos`, `mac-mini`, and
`macos-latest` are host executors. `arm-ubuntu-22.04` is a Linux Docker executor.
Native jobs execute under william, whose home contains runner and SSH state.

## Linux configuration delta for review

[`../.forgejo/runner-proposals/opensuse-ci.patch`](../.forgejo/runner-proposals/opensuse-ci.patch)
is an exact, locally rehearsed delta against the inspected configuration:

- Remove globally injected Docker control variables and the `.env` job import.
- Remove Docker client-certificate mounts and permitted job bind mounts.
- Use a per-job bridge network; keep `privileged: false` and `docker_host: "-"`.
- Bound job containers to 8 GiB, 2 CPUs and 1024 processes; retain capacity 1.
- Disable persistent mutable workflow cache during the initial isolation gate.
  Existing cache data remains on disk; it is not purged.
- Keep the four authorized Linux labels and their Docker executors unchanged.

The runner controller still needs its existing TLS daemon credentials outside job
containers to schedule containers/services. Removing access from jobs must not
remove controller access or weaken TLS. Check inherited environments and the actual
created mounts, not just YAML. No signing/registry credential belongs in the runner
job defaults or reusable build images.

**Compatibility boundary:** existing jobs that call Docker directly will fail
without another reviewed trusted build path. Do not apply this patch while assuming
OCI builds and Compose tests still work. Before activation, replace those job paths
with isolated, disposable builders controlled outside untrusted workflows, and run
the existing Compose/independent-client regressions against disposable services.
No production Docker socket/certificates may be reintroduced as a shortcut.

## macOS boundary: host settings cannot enforce this alone

Changing `host.workdir_parent`, HOME, a shell wrapper, or a workflow condition cannot
prevent arbitrary host jobs running as william from reading personal/runner state.
The inspected backend has no verified per-job macOS VM executor or separate job UID.
No purported secure Mac configuration file is supplied for a capability that has
not been established.

Keep native builds on the **same Mac-mini** physical machine and registered runner,
but choose and validate an enforceable execution/authorization boundary before
untrusted jobs can target its host labels. The concrete alternatives requiring
review are:

1. **Protected build/publication control project.** Restrict the existing Mac-mini
   runner's server-side job scope to a controller project editable only by release
   maintainers. Its pinned workflows fetch exact approved Cords commits. Untrusted
   PRs receive mandatory isolated Linux checks; native Mac builds/tests require an
   explicitly reviewed commit, and all release artifacts still require native Mac
   validation. Move private publication secrets out of the branch-writable Cords
   workflow scope. Keep native build jobs free of signing/registry secrets.
   The controller project now exists with Actions disabled; registration transition
   is conditionally approved. Credential migration and isolated builder activation
   remain prepared/review gates. There are still only two physical workers.
2. **Runner-level disposable macOS guest execution.** Establish a maintained,
   enforced executor that runs *all* native job payloads inside disposable macOS
   guests, with the registration/control plane outside the guest. No host-home mounts,
   SSH forwarding, release credentials, shared writable caches or personal keychain.
   Merely launching a VM in a branch-controlled `run` step does not enforce this.
   The currently inspected runner has no demonstrated support; implementation,
   storage/memory budgeting and additional tool approval are separate unmet gates.

Do not register a replacement/new runner, silently switch macOS artifacts to Linux,
or send GPG/Cosign private keys to the Mac under either alternative. No native
artifact is removed from the required matrix to conceal the unresolved gate.

## Release authorization

Protected tags must cover `v*` and `V*`, restrict creators to release maintainers,
and reject tag replacement/deletion. Protect the controller workflows and public
pin updates through review. Before secrets are supplied, independently verify the
event/tag/commit and approved validation record. A matching tag name alone is not
authorization; branch-controlled scripts cannot grant themselves authority.

Forgejo v14 repository secrets are not shown to have an enforced tag-only scope.
Keeping private keys in an arbitrary branch-writable workflow scope cannot fulfill
that invariant merely by adding an `if` condition. The protected-control alternative
addresses this separately from container isolation; its ACLs must be tested.

## Plan → implement → validate for approved activation

1. Drain active jobs. Back up exact controller configuration, launch definitions,
   registration state and relevant ACLs privately. Do not put registration tokens
   in Git, logs or artifacts. Record running production services without restarting
   them. Verify inspected configuration hash before applying a delta; stop on drift.
2. Rehearse on isolated copies. Apply the reviewed Linux delta, and the selected
   Mac/secret control boundary, during an approved maintenance window. Keep existing
   runner identities; do not concurrently run a cloned registration.
3. Test a deliberately hostile *disposable* job: no Docker control variables/certs/
   sockets; forbidden host/cert volumes rejected; privileged requests rejected;
   no personal home, controller `.runner`, signing/registry credentials or reusable
   credential-bearing files; no access to other jobs/production administration.
   Validate network restrictions against LAN services separately. Prove that arbitrary
   PR workflows cannot route onto the native host or request release secrets.
4. Test healthy disposable PostgreSQL/services, all Rust/frontend/security and
   independent-client coverage, packaging/OCI builders and caches. Test failures
   remain failures and unavailable native gates cannot become green through skips.
5. Execute native Mac packages and independent GUI/vault/TLS/message/restart/offline
   acceptance. Then rehearse staged publishing in an isolated namespace, including
   key mismatches, artifact substitution, moved tags, failed mirrors and retries.
6. Record hosted evidence before activation. Neither this patch's local parsing nor
   independent SSH package builds are hosted runner/isolation acceptance.

Rollback: drain jobs and restore matching reviewed controller settings/ACLs only;
never restore stale live cryptographic heads or replace published release bytes.
If a security gate fails, keep affected dispatch/publication disabled rather than
restoring the known credential exposure. Retain existing artifacts and deployments.

Authority: [Forgejo v14 runner security](https://forgejo.org/docs/v14.0/admin/actions/security/)
describes Docker and host execution boundaries and arbitrary branch workflow risks;
[Forgejo v14 workflow security](https://forgejo.org/docs/v14.0/user/actions/security/)
describes event/secret restrictions. These sources do not assert that the proposed
configuration is a tested complete solution for these hosts.
