# Controller secret boundary: migration review

> **Split-repository proposal superseded.** Preserve the verified Forgejo findings
> and consumer audit. Follow [the monorepo plan](ci-monorepo-plan.md) and external
> execution/credential boundary. No Cords publication keys may be stored in Actions
> owner/repository scopes. Existing owner secrets remain untouched pending complete
> consumer review and explicit migration approval.

Current read-only result: [71-repository consumer inventory](reports/owner-secret-consumers.md).
Its dynamic/external/override limitations must be resolved before any owner migration.

Status: prepared for review; no credentials or registrations changed. The private
controller's helpers are at signed commit `01235ee48174a051879b96ca4461309b942a62b9`,
with the activation warning added in signed HEAD
`381b2205d856be015ef3b6be8dfb2a06398fc237`. Actions is disabled. The approved Mac
registration transition cannot proceed yet.

## Verified behavior and blocking condition

Forgejo v14.0.5 sends all applicable owner and repository secrets to every ordinary
job's runner. This is a server task-payload boundary, not merely a shell environment
or workflow-expression boundary. `services/actions/task.go:42,66` calls
`GetSecretsOfTask` and includes its result in `runnerv1.Task.Secrets`.
`models/secret/secret.go:123-166` decrypts all owner and repository secrets except
for the documented fork-PR case. Push/manual controller jobs inherit them.

The logged-in owner's Actions settings currently contain these names (values
were masked and never accessed):

| Owner secret names | Observed role / limitation |
|---|---|
| `CI_KEY_B64`, `CI_KEY_PASSPHRASE` | Requested Cords release signer inputs |
| `COSIGN_B64`, `COSIGN_PASSWORD` | Requested OCI signer inputs |
| `GH_KEY` | GitHub publication/mirroring input |
| `CI_KEY` | Existing shared input; do not assume its encoding or consumers |
| `KEY_ALIAS`, `KEY_PASSWORD`, `KEYSTORE_BASE64`, `KEYSTORE_PASSWORD` | Other existing signing inputs; consumers not inventoried |

Repository-scoping Mac-mini would restrict job eligibility, but would not prevent
these owner secrets reaching its controller jobs. Putting publication keys in that
controller's repository secrets would also send them to Mac jobs. No omission
from `env`, runner label, workflow condition, or `permissions: {}` solves this.
Forgejo's v14 compatibility documentation identifies job `permissions` as ignored.

**Do not enable controller Actions or migrate Mac-mini until this is resolved.**
No secret compromise is established by this inspection. No hostile live job was
dispatched, and no production credential was retrieved or tested.

## Proposed boundary for explicit review

Keep `cords-release-control` a **secret-free build controller**, including inherited
owner scope. Its repository-scoped Mac-mini runs only explicitly reviewed exact
Cords commits. Keep the same two physical workers and all required release targets.

Use a **separately protected publication repository** for trusted Linux signing and
publishing jobs. Mac-mini must have no eligible registration for that repository.
Store release inputs only in that publication repository's secret scope, after
verified Linux isolation. Its workflows execute protected signing helpers, never
source-provided executable code or build-provided public keys. Builds hand off
artifacts by exact controller run/job and commit identity, with a complete matrix,
digests and independent authorization checks. Built-in token permissions and
cross-repository denial remain mandatory hosted tests, not assumed guarantees.

That split alone is insufficient while owner-wide secrets remain. Inventory every
existing consumer of all ten owner secrets, then prepare a destination mapping into
approved repository-specific scopes. Preserve every currently functional consumer
and the intentionally shared signing identities. Do not generate replacement keys,
change the user's Mac account, create runners, or silently reconfigure other projects.
Moving shared owner secrets affects those other projects and requires an explicit
review of the complete mapping before application.

An alternative is a server-enforced per-job secret policy or separately isolated
external signer. Neither is established on the current service. Do not claim v14
supports such filtering or implement a speculative broker to avoid this review.
If the proposed split/migration is not authorized, keep activation blocked.

## Plan → implement → validate before any credential change

1. **Inventory, read-only:** enumerate active/default and release workflows that
   consume the owner inputs, repository access rules, reusable workflows, schedules,
   dependent services and runner scopes. Include dynamically referenced secrets and
   unresolved consumers as blockers. Record names, not values. Locate and verify the
   actual public v14 service; `.54`'s v13 container is not an administration target.
2. **Concrete mapping for approval:** list each owner input, all consumers, proposed
   protected destination, compatible workflows, recovery owner and maintenance order.
   No deletion until all consumers and secure recovery paths are established. Keep
   values out of Git, logs, test artifacts, shell arguments and Mac jobs. Secret
   copying/backup needs a verified trusted administration mechanism; this inspection
   did not establish one or retrieve existing encrypted values.
3. **Repository-only preparation:** prepare publication ACLs and pinned workflows
   with Actions disabled and no secrets. Validate exact-commit/tag authorization,
   complete artifact inventory, authenticity/provenance, byte-preserving handoff,
   wrong-run/wrong-commit/substitution rejection and retry rules before activation.
   Existing native build jobs and ordinary branch tests must never enter this scope.
4. **Infrastructure gates:** validate isolated Linux OCI/Compose and ordinary jobs
   before removing Docker certificates; verify all mounts, network/token access,
   hostile jobs and cache separation. Back up/drain the approved Mac registration
   transition and validate its complete prerequisites. No live restoration of Global
   Mac eligibility as an automatic rollback.
5. **Approved maintenance only:** privately establish the reviewed per-repository
   copies and validate consumers before removing owner entries. Removing owner scope
   must not remove the only recoverable credential copy. Secret duplication alone
   does not satisfy isolation: activation remains disabled while owner entries exist.
   Shared private key bytes and authoritative public pins stay the same.
6. **Acceptance:** use harmless sentinel secrets first to prove that native Mac jobs
   receive no owner/publication secrets and cannot request publication jobs, refs,
   artifacts or credentials. Prove ordinary Cords and unrelated jobs cannot dispatch
   Mac-mini or access publication scope. Validate all former owner-secret consumers
   independently. Only then install/use real release inputs in protected jobs and
   verify private/public correspondence without logging them. No production release
   is part of this test.

Missing evidence fails the gate. If migration fails, preserve diagnostics, disable
affected publication, and recover through the reviewed per-repository mapping.
Do not restore owner-wide exposure automatically or delete valid releases. No
credential movement, deletion, new publication repository or host service has been
applied by this review.

## Primary sources and evidence

- [v14.0.5 task payload](https://codeberg.org/forgejo/forgejo/src/tag/v14.0.5/services/actions/task.go)
- [v14.0.5 secret loading](https://codeberg.org/forgejo/forgejo/src/tag/v14.0.5/models/secret/secret.go)
- [v14 workflow security](https://forgejo.org/docs/v14.0/user/actions/security/)
- [v14 ignored job permissions](https://forgejo.org/docs/v14.0/user/actions/github-actions/)
- [Exact commands, source hashes and live settings evidence](reports/ci-rework.md)
