# External signing/publishing executor contract

## Plan

One canonical `firebadnofire/cords` monorepo; no permanent controller/publication
repository. This is a preparation contract, **not an installed service**. The
[policy](../.forgejo/release-policy.json) has activation disabled. The executor
must fail closed until each gate below is independently verified. Product C0/C1
maturity is unchanged.

## Implementation contract

Use an independently protected execution environment on the existing Linux physical
host only if ordinary jobs cannot reach its credentials, disks, control interfaces,
inputs or durable log. A guest boundary with no job-controlled Docker/host mounts,
shared mutable caches or credentials is the candidate for review; no guest/backend
has been installed or validated. A separate repo, directory, job label or UID alone
does not defeat the current daemon access. The service is not an Actions job.

Keep service source, fixed release policy, packaging/build definitions and verifier
tests in Cords. Install only an explicitly reviewed commit/digest through an
independent administrative step into protected service storage. Repository writes,
tag pushes and job tokens cannot update that installation. The service never runs
scripts, actions, commands, plugins or executable paths supplied by candidate commits,
manifests or artifacts. Keep downloaded packages inert; inspect them in a separately
isolated verifier. Do not import or execute their provenance files.

Store existing user-authorized GPG/Cosign private keys, passphrases and minimal
Forgejo/registry/GitHub publication tokens outside owner/repository Actions scopes.
Do not copy private keys to Mac-mini or generate replacements. Verify private-key
correspondence to installed public pins before signing. Pin rotation and service
updates require explicit review. Never trust build-supplied keys or URL changes.
Existing owner-secret migration requires a complete consumer-preserving approved
mapping; it is not performed by this contract.

### Candidate intake and independent authorization

Candidate requests contain only exact source identity, file digests/sizes, OCI
digests, policy/pin hashes and evidence references. `validate-release-request.py`
checks shape/completeness/bytes **only**; a success is never permission to sign.
Requests have no `authorized`, `success`, shell command, executable or arbitrary
download URL fields. Queue admission grants no privileged operation.

The schema-1 request fields are `schema`, `origin`, `repository`, `tag`, `commit`,
`policy_sha256`, `signing_identity_sha256`, `tests`, `builds`, `files` and `oci`.
Each evidence reference has `id`, positive `run_id`, `job_id`, `attempt`, matching
`commit` and `evidence_sha256`. These are locators to verify independently, not
client-certified test results. File entries bind `name`, `size` and `sha256`;
OCI entries bind `image`, immutable `digest` and exact `platforms`. Require the nine
policy packages, nine checksum files, pinned `cosign.pub` and inert
`release-provenance.json`. Incoming `SHA256SUMS`/signatures are not accepted as inputs:
the protected signer generates/verifies them after approval. Intake JSON is bounded
to 1 MiB, files to 8 GiB each/32 GiB total. Duplicate/unknown fields and inventory
entries fail. These service preparation bounds are independent of messenger limits.

```sh
python3 scripts/validate-release-request.py --request candidate.json --artifacts immutable-staging
python3 scripts/test-release-request.py
```

The offline validator assumes privately owned immutable staging; it does not enforce
concurrent-writer exclusion, race-safe intake, provenance signatures, archive member
validation or network authorization. Real executor acceptance must cover those gates.

The executor independently obtains these facts over certificate-validated TLS or
strictly verified SSH, using its own read-only identity:

1. Canonical protected v*/V* tag rule, authorized creator/event and exact peeled
   commit; commit's workspace version equals strict tag SemVer. Check current tag
   again before finalization and reject moved/deleted tags. Tag syntax or a Git
   signature alone is not proof the tag was authorized. Forgejo endpoint coverage
   for creator/protection/immutability remains to be validated. Missing evidence
   requires manual authorization, never an inferred `authorized: true`.
2. Full required test results for that exact commit, correct event/ref and expected
   independently installed workflow/policy revision. Never accept arbitrary
   branch-selected check names or a green aggregate that omitted/cancelled jobs.
   Verify **tests finished successfully before build start**, all required matrix
   entries, run attempts and immutable artifact association. Job labels, names and
   a user-uploaded provenance JSON cannot authenticate a builder.
3. Artifact provenance authenticated independently by a protected builder/control
   plane outside the untrusted job, or explicit independent maintainer acceptance
   of the exact candidate manifest/digests. Builders must not attest arbitrary bytes
   on a job's request. Provenance binds source, build recipe/toolchain, platform,
   tests, run/job/attempt, artifact digest and policy. The exact authenticated
   mechanism/key identity is an unresolved activation gate, not an invented key.
4. Copy artifact bytes into a bounded private staging area through a trusted intake
   mechanism, validate exact package inventory/architecture, hashes, checksums and
   pinned public key bytes. Never follow symlinks/path escapes or use shared writable
   workspaces/caches after verification. Race-safe ownership and intake quotas require
   integration tests; the offline validator assumes immutable staging.
5. Approve the exact normalized manifest, including both OCI destinations/platforms,
   through protected service policy and explicit maintainer approval until fully
   automated authorization/provenance gates pass. Record the approval identity and
   manifest digest independently; no approval is supplied by branch code.

### Signing, publication and recovery

Append an immutable transaction identity binding protected tag, peeled commit,
policy revision and accepted manifest digest. Persist state before acknowledgement.
Repeated identical requests reuse verified signatures/results; conflicting bytes,
retargeted tags or policy revisions freeze the release for review. No unchecked
overwrite, delete, downgrade or automatic live-history rewind.

Sign each release input, each `.sha256`, provenance and `SHA256SUMS` using GPG
ASCII-armored detached `.asc`; verify every signature with the installed public
identity. Sign OCI manifest digests using Cosign, separately verify Forgejo and
GHCR with the exact installed Cosign key. Builds export OCI layouts/digests without
registry publication credentials; registry pushes happen only in the protected service.

Stage Forgejo and GitHub drafts, mirror the exact source tag/commit and asset bytes,
download/reverify complete files/signatures and both OCI signatures before finalization.
Record partial remote progress durably. Cross-registry/release finalization is not
atomic: retain verified partial publication, repair forward, never silently replace
conflicting assets. The old `publish-release-assets.sh` auto-replacement path is
not approved for this executor. No production signing/publication is run here.

## Validation and manual fallback

Adversarial hosted acceptance must prove PR/unrelated-repo jobs cannot reach Mac
host personal/runner state, signer credentials/control plane, installed policy,
staging/logs or trusted caches; cannot forge tests/provenance, substitute files,
select commands or authenticate an unauthorized tag. Test crash/retry, timeouts,
quotas, invalid signatures, moved tags and failure in each publication destination.
Do not dispatch a hostile payload against the unsafe existing live host as a test.

Forgejo v14 lacks a demonstrated protected-ref-only runner/secret boundary in this
installation. With Global host execution and inherited owner keys, fully automated
secure release activation is **blocked**. Until a real executor/provenance boundary
is validated, use independently approved native builds and a maintainer-controlled
isolated signing session with the same policy, digest review, pin verification and
durable release checks. This is a proposed manual procedure, not evidence of a
completed release or authorization to install/change credentials.

Approved native execution as william retains access to a persistent personal host.
Approval controls which source executes; it is not an OS sandbox. A compromised
approved commit, toolchain or retained host state can compromise future builds.
No claim of complete host isolation applies to the manual/approved native fallback.

Required approvals: executor/backend and host changes; Mac enforced execution/
registration changes if needed; complete owner-consumer credential mapping; installed
policy/provenance authority; protected-tag rules and hosted acceptance; final activation.
No additional physical runner or permanent Cords repository is proposed.
