# CI/CD rework evidence — 2026-10-11

Status: **incomplete, not activated**. No hosted workflow, production release, tag,
registry object or shared gh-sync repository was modified by these local tests.
The Cords remediation tree is now committed as
`c9cfa658c428f449ce60b20f61f1567260affb9a` during the signing wait. No source
Cords push or workflow activation was performed by this agent.
The new private controller repository and its restrictive ACL/settings were created;
Actions remains disabled. No runner settings, credentials, deployments or unrelated
files were changed.

## Plan

The ordered implementation and unresolved security decisions are recorded in
[`../ci-rework-plan.md`](../ci-rework-plan.md). Unit 1 (offline release contracts)
passes its local gate. Public identity pinning also passes its local gate. The user
selected the protected-controller direction; current Global registrations block
activation. Workflow/isolation/publication units remain pending.

## Implementation

Changed:

- `scripts/sign-release-artifacts.sh`: require an independently configured primary
  checked-in public fingerprint, exactly one matching imported primary secret key,
  per-asset armored signatures,
  signed exact manifest, verification before output, unchanged valid bytes on retry.
- `scripts/test-release-contract.sh`: disposable certification/signing subkeys and
  positive, tamper, wrong-key, missing-configuration and retry fixtures.
- `.forgejo/docs/release-workflow.md`: mark the existing workflow as unrevised and
  document revised signer/consumer commands and the supplied public identities.

Added:

- `scripts/verify-release-artifacts.sh`: public-key-only, read-only inventory/hash/
  signature verification with a separately pinned primary identity.
- `scripts/release-contract.py`: strict v/V SemVer 2.0 and exact byte/inventory/status
  contracts. This is a validator, not proof of a complete package matrix.
- `scripts/test-release-contract.py`: seven supplementary contract tests.
- `docs/ci-rework-plan.md` and this report.

Existing packaging scripts were inspected and retained, including Linux
client/server, matching-architecture Linux-to-Windows cargo-xwin, native macOS
universal DMG, native PKG/server tarball, and OCI builds. No replacement packages
or production signing material were manufactured.

## Validation

Native openSUSE `.54`, isolated `/home/william/cords-ci-review-20261011/scripts`:
Python 3.13.15, GnuPG 2.5.22/libgcrypt 1.12.4, ShellCheck 0.11.0.
Native Windows Python 3.13.14 also runs the seven Python tests successfully.

Exact transfer and Linux commands (strict existing SSH trust):

```powershell
scp.exe -o BatchMode=yes -o StrictHostKeyChecking=yes `
  scripts/release-contract.py scripts/test-release-contract.py `
  scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh `
  scripts/test-release-contract.sh `
  william@192.168.86.54:/home/william/cords-ci-review-20261011/scripts/
ssh.exe -o BatchMode=yes -o StrictHostKeyChecking=yes william@192.168.86.54 `
  'cd /home/william/cords-ci-review-20261011 && shellcheck scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh scripts/test-release-contract.sh && bash -n scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh scripts/test-release-contract.sh && bash scripts/test-release-contract.sh'
python scripts/test-release-contract.py
git diff --check
```

Results: ShellCheck and shell syntax **PASS**; Python **7/7 PASS** on both hosts;
real GnuPG fixture signing/verification and eight expected failure scenarios
**PASS**. Five input assets plus the manifest receive six verified `.asc` files.
An Ed25519 signing subkey verifies under the pinned certification primary key.
Repeat signing preserves all signatures and manifest bytes. Wrong public/private
pin, absent pin, invalid base64, missing signature, altered asset, altered retry,
and altered signature fail closed. No production release keys were used.

Initial ShellCheck found SC2035 in two fixture checksum commands; adding `--`
resolved them and the complete gate was rerun. A hash-inventory command initially
included Python's `__pycache__` directory; explicit file paths corrected the command.
These diagnostics were not ignored or treated as successful checks.

Initial unit's source SHA-256 (historical, before public-pin integration):

```text
632ff84af5989b4beb85aace0e718fc9c3b98ef2df4c593d46ebdab57ec12140  release-contract.py
b2aeae985506a8bd4c7175899d2913782f8014c20eb0739a6e8baeac748da2c5  test-release-contract.py
412e41b8d3c69e6016eae02e37d6e852d7ac68f7d9c534b149af8a816fb5b261  sign-release-artifacts.sh
8c9988f19ed1e263db5a3bfad8dcfcf79aceeb2b0b98a0317205ea9bf11aacb8  verify-release-artifacts.sh
d3a67af1386b443cc5ec44ffd18b21eda46fe0334a6211b6ae026b6e55162df3  test-release-contract.sh
```

## Unmet gates

- All-branch/PR/exact-commit triggers and mandatory aggregate checks are **not yet
  implemented/hosted-tested**. Workflow files remain unchanged pending isolation.
- Inspected Linux runner supplies `DOCKER_HOST` and TLS client credentials to every
  job; Mac-mini native host jobs run as william beside persistent runner/SSH state.
  Removing secrets from individual steps is insufficient. An isolation decision
  has been selected; current Global registrations cannot enforce it. The prepared
  transition requires review. No new physical runner or reconfiguration introduced.
- Public GPG and Cosign identities are now supplied, retrieved and pinned (below).
  Actual CI private-key correspondence must still pass in protected hosted jobs.
- Strict protected-tag authorization before secrets, tests-before-build dependency
  graph, complete verified inventory/provenance, immutable OCI publication to both
  registries, draft staging/finalization, byte-identical mirror, cache isolation and
  hosted negative/rerun tests remain **pending**, not passing.
- Read-only gh-sync inspection at `d879ea006b4cf0ed91659727d76fc6f1551d4fbe`
  established useful draft/overwrite=false support but missing exact commit binding,
  matching-asset retry and metadata/digest verification. No shared library edits or
  publication were performed.
- Mac-mini now has sufficient disk capacity. Native PKG integrity and independent
  startup smoke pass after a bundle-sealing repair. Full GUI acceptance remains
  unavailable; see `rework-c0.md` for individual build/test outcomes.
- Apple Developer signing/notarization and Windows Authenticode have no demonstrated
  credentials/evidence. GPG signatures do not imply either distribution signature.

The old workflow is not a safe revised release path merely because the local
signing tests pass. Its invocation must be updated only with the remaining gates;
the new signer deliberately fails without the checked-in public pin. Keep activation
disabled until all mandatory evidence exists. C0 remains incomplete; C1 stays inactive.

## Authoritative public identities and mismatch gates

The user designated the existing shared keys as authoritative, on 2026-10-11.
No new production key was generated. TLS 1.3 retrieval:

```powershell
curl.exe --proto '=https' --proto-redir '=https' --tlsv1.3 --location --max-redirs 3 --fail --show-error https://archuser.org/gpg.key -o "$env:TEMP/cords-ci-keys-20261011/gpg.key"
curl.exe --proto '=https' --tlsv1.3 --fail --show-error https://raw.githubusercontent.com/firebadnofire/bazzite-firebadnofire/refs/heads/main/cosign.pub -o "$env:TEMP/cords-ci-keys-20261011/cosign.pub"
```

The first GPG request without redirect-following downloaded HTTP 301 HTML; GPG
rejected it. The corrected HTTPS-only redirect resolves to
`https://archuser.org/gpg/william.asc`. Despite that suffix, the 1042-byte certificate
is binary OpenPGP, not ASCII armor. Real GnuPG verifies all four self-signatures.
Full primary fingerprint: **7D6EF134D851C8DA0862D97494F31AF374E2EE3C**; signing
subkey: `F66F1B747BC6EE547BF0ACE442B0D0D2CA34F814`. This is a release identity;
its UID mentioning a YubiKey provides no Cords native-token acceptance evidence.

The exact Cosign PEM passes OpenSSL public-key checking (P-256). Exact public bytes,
SHA-256, full fingerprint, source URLs and redirect provenance are source-configuration
inputs under `.forgejo/release-keys/`. No runtime URL retrieval changes trust.

| Public file | SHA-256 |
|---|---|
| `gpg.pub` | `f1d3b7895d0ddb6f5638c92ebf80434152bf9db797d56473e9d1a793c403e8e9` |
| `cosign.pub` | `fa232091c35997060654dfbcb58c911adc5432756f5234e8846a51dba3ee2d01` |

`scripts/release-identity.py` checks byte pins, certificate fingerprint/liveness/
self-signatures, absence of private packets, and PEM validity. Signing and read-only
verification use the pinned public certificate rather than a private-key export.
`scripts/publish-container.sh` compares the derived private Cosign public key with
the pinned key **before publication**, and uses that pin to verify both registries.
Its remaining publication/staging behavior is still unrevised and inactive.

Repeated native `.54` commands:

```sh
cd /home/william/cords-ci-review-20261011
python3 scripts/release-identity.py
python3 scripts/test-release-identity.py
shellcheck scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh scripts/test-release-contract.sh scripts/publish-container.sh
bash scripts/test-release-contract.sh
```

Results: production **public** identity validation PASS, identity tests **5/5**,
contract tests **7/7**, GnuPG rejection scenarios **10/10**, ShellCheck PASS.
New negatives prove a fixture private key cannot satisfy the real production pin,
and an unpublished signing subkey of the same primary is rejected before promotion.
Positive signing tests use only disposable fixture identities in an isolated copy
of the scripts/configuration; no test override bypass exists in production scripts.
Valid retries preserve all six signatures and manifest bytes.

Validated current source hashes:

```text
5e2b9d76cf2ded96c9f483d91fd6c408e9419dd2650f5234bd3c8f1178a5d630  release-identity.py
3926cb19cd94fc13e7fd346d2af1b2d7b0f037306b899fe7bc41d0be7595471d  test-release-identity.py
d7276ed9f339a2984931a490a491f6bf558555f663ca6a9cf84e44c110304e28  sign-release-artifacts.sh
d3ba202bcf113e41025157c1d87e99e734a5ea4f7deee10b42207605a7e5e284  verify-release-artifacts.sh
60b09316a83bfe8fc85783c0934daa7555d2b8e32aac5733c8cbd6b2dc0e191c  test-release-contract.sh
ae40a9f75f3f5d8f32c17cc03bdeedac9be6ec72314e47e6bf8cfc6f7cd620c1  publish-container.sh
```

Actual `CI_KEY_B64`/`COSIGN_B64` were not retrieved, signed with or copied to the Mac.
The production correspondence gate is implemented, but actual protected hosted
execution remains pending. Isolated Cosign v3.1.2 matches its pinned binary hash
`f7622ed3cf22e55e1ae6377c080979ff77a22da9981c11df222a2e444991e7cf`.
An official upstream encrypted test fixture could not decrypt with an empty
password; that attempted supplemental test failed, and no password guessing or
Cosign positive-signing claim followed. No OCI object was published.

## Controller and runner review evidence

The exact Linux configuration patch passed `patch --dry-run --fuzz=0 -p0` against
a retained copy with the inspected SHA-256. Windows PyYAML 6.0.3 parsed the proposed
configuration and assertions verified removed credentials/mounts, non-privileged
execution, per-job networking, bounded resources and disabled cache. Linux Python
lacks PyYAML; that failed local parsing attempt was diagnosed, not silently skipped.
The patch has **not** been applied; Docker/Compose replacement acceptance is pending.

See [runner proposal](../runner-isolation-review.md) and
[registration transition](../controller-transition-review.md). Live v14 admin UI
confirms both registrations are Global; actual eligibility source has no repository
restriction for them. Existing tokens cannot safely rescope those IDs through the
CLI. Current registrations do **not** satisfy the selected controller boundary.
No hostile job was dispatched onto the Global Mac, and no adversarial hosted
isolation test is claimed. Controller activation stops at this gate.

The user subsequently approved the conditional repository-scoped registration
transition. Its workflow/dependency/backup/drain prerequisites are not yet complete;
authorization is not evidence of execution. The private controller now exists at
`firebadnofire/cords-release-control`, README-only commit
`a426842bb8160ee35ac8d29a0fbac91ecd68945a`, no collaborators. Saved Actions-disabled
state, all-branch owner-only push/merge (admin enforcement, no deploy-key bypass or
force push), and all-tag owner-only protection were inspected after saving. No
workflows/credentials were installed, no source Cords workflow was pushed, and no
job was dispatched. Inherited secret/token exposure and hostile ACL tests remain
unvalidated; this is initial repository preparation, not completed isolation.

The public Forgejo reports v14.0.5; `.54`'s local container reports v13.0.3. The local
container is not an established administration target for the public instance.
Mac-mini's three daemon pollers and unidentified `dev` runner version require
review before registration changes. Approved Mac jobs still execute as william
on a persistent host; protected dispatch is not an OS sandbox.

Additional read-only `.54` builder inspection: QEMU 11.1.2, libvirt/virsh 12.8.0,
systemd 261.3 and readable KVM exist; Podman, Buildah and BuildKit are unavailable.
An isolated OCI/Compose path is **not** thereby established. No tool was installed,
VM launched, host networking changed or Docker access restored to arbitrary jobs.
The prepared certificate-removal patch remains blocked on that replacement path.

Native Mac evidence now includes accepted package-integrity checks for ARM PKG,
universal DMG and ARM server tarball; two independent startup/restart smoke paths;
36 frontend tests and 37 Rust tests, Clippy and formatting. Its 21 database tests
remain ignored and full graphical messaging acceptance is pending. Package hashes,
commands, failures and transfer checks are in [C0 report](rework-c0.md).

The controller's unauthenticated repository API returns HTTP 404 as expected for
its private visibility. This is a privacy check, not a test of runner eligibility.
Saved UI evidence is retained locally under
`C:/Users/william/AppData/Local/Temp/cords-ci-keys-20261011/evidence/`.

## Exact controller helper staging

Cloned the private controller through existing HTTPS Git authentication into
`C:/Users/william/AppData/Local/Temp/cords-release-control-20261011` (no new credential).
Staged the exact validated public keys/configuration, seven release-contract/
identity/signing/verification helpers, README and LF/binary Git attributes. No
unfinished OCI publisher, active workflow, approved source commit or private key
is included. Future privileged jobs must execute these protected-controller
helpers, not a script or key supplied by a Cords build commit.

Exact staged copy was transferred to the isolated `.54` fixture and validated:

```sh
cd /home/william/cords-ci-review-20261011/controller-stage
python3 scripts/release-identity.py
python3 scripts/test-release-identity.py
shellcheck scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh scripts/test-release-contract.sh
bash -n scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh scripts/test-release-contract.sh
bash scripts/test-release-contract.sh
```

PASS: public identity, five identity tests, seven contract tests, ten real GnuPG
rejection scenarios, shell syntax/ShellCheck. Helper and public-key hashes match
the earlier validated source exactly. Disposable fixture secrets are cleaned by
the test; production credentials are not used.

The controller commit currently waits on the user's existing local
`commit.gpgsign=true` GnuPG/pinentry configuration. No signing bypass is applied.
An attempted push was issued before the pending commit completed; it reported
`Everything up-to-date` and remote HEAD remained the initial README commit
`a426842bb8160ee35ac8d29a0fbac91ecd68945a`. This is **not** publication of the
staged helpers. Await local signing authorization, verify commit completion and
exact remote HEAD before recording a successful controller source update.

## Signed controller commit and secret-boundary blocker

After the user confirmed signing was cached, the previous signing attempt was
observed to have timed out (exit 1). The retry succeeded without bypassing GPG:

```powershell
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 diff --cached --check
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 commit -m 'Pin release identities and stage validated signing contracts'
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 verify-commit HEAD
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 log -1 --format='%H %GF %G? %s'
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 push origin main
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 ls-remote origin refs/heads/main
```

PASS: exact signed commit and remote main are
`01235ee48174a051879b96ca4461309b942a62b9`; signature status `G`, primary
`7D6EF134D851C8DA0862D97494F31AF374E2EE3C`. Controller checkout is clean.
Push followed successful signature verification. Forgejo's code page shows the
same commit, signed badge and public helpers. Actions remains disabled; no runnable
workflow or private credential was added. Browser proof:
`C:/Users/william/AppData/Local/Temp/cords-ci-keys-20261011/evidence/controller-pushed.jpg`.

The next read-only security gate **fails for the proposed single-repository
build/signing model**. Forgejo v14.0.5 `services/actions/task.go:42,66` calls
`GetSecretsOfTask` and sends the complete result in each runner task's `Secrets`
field. `models/secret/secret.go:123-166` loads and decrypts all owner secrets plus
all repository secrets for ordinary non-fork jobs. It does not filter by runner,
job, referenced expression, environment or workflow `env`. The fork-PR exception
does not protect a trusted controller's push/dispatch jobs. Omitting a secret from
a Mac step does not keep that secret off the Mac runner.

Live owner settings at `/user/settings/actions/secrets` list `CI_KEY`,
`CI_KEY_B64`, `CI_KEY_PASSPHRASE`, `COSIGN_B64`, `COSIGN_PASSWORD`, `GH_KEY`,
`KEY_ALIAS`, `KEY_PASSWORD`, `KEYSTORE_BASE64`, and `KEYSTORE_PASSWORD`.
Only names and masked values were viewed. No secret was retrieved, revealed,
copied, changed or removed. These owner secrets would be inherited by this
controller even with no repository secrets. Owner-wide consumers are not yet
inventoried. There is no claim that any key was compromised or exfiltrated.

Source retrieval used verified TLS 1.3 and existing strict SSH trust, storing public
files in `/home/william/cords-ci-review-20261011/forgejo-v14-secret-audit`:

```sh
curl --proto '=https' --tlsv1.3 --fail --silent --show-error --location \
  https://codeberg.org/forgejo/forgejo/raw/tag/v14.0.5/services/actions/task.go \
  -o forgejo-v14-secret-audit/service-task.go
curl --proto '=https' --tlsv1.3 --fail --silent --show-error --location \
  https://codeberg.org/forgejo/forgejo/raw/tag/v14.0.5/models/secret/secret.go \
  -o forgejo-v14-secret-audit/secret.go
sed -n '1,100p' forgejo-v14-secret-audit/service-task.go
sed -n '123,205p' forgejo-v14-secret-audit/secret.go
sha256sum forgejo-v14-secret-audit/*.go
```

| Public source file | SHA-256 |
|---|---|
| `routers/api/actions/runner/runner.go` | `b32af658854ef715879d14033f7c7586539f44da3973880fb0b685a5b22daa28` |
| `services/actions/task.go` | `79160206751b62fa5e9ae5433f8abffea6f8ebb1d0c4b3535c3d17984efcec2f` |
| `models/secret/secret.go` | `f1e504119615f8a88e4a57cb1dd14814f5573da0eaaf123354993567d295477f` |

Diagnostics: `.54` has no `rg`; the first search failed explicitly and was repeated
with `grep`. Two guessed source-file paths returned HTTP 404; neither supplied
evidence. The actual `PickTask` call and versioned secret implementation above
establish the boundary. The public source-tree API was subsequently consulted
rather than continuing guessed authentication paths.

Forgejo's [v14 compatibility documentation](https://forgejo.org/docs/v14.0/user/actions/github-actions/)
also identifies job `permissions` as ignored. No YAML `permissions` restriction
is accepted as proof that a native job token is read-only. Actual endpoint and
protected-ref tests are still required; no hosted job was started to inspect or
exfiltrate credentials.

Activation, credential relocation and the Mac registration transition stop at
this gate. The [secret-boundary migration review](../controller-secrets-review.md)
describes the required repository separation and consumer-preserving migration.
This is prepared for review, not executed. C0 remains incomplete pending macOS
graphical acceptance; C1 stays inactive.

The controller README now carries this same activation blocker in signed commit
`381b2205d856be015ef3b6be8dfb2a06398fc237`. Its documentation-only change passed
`git diff --check`, `git verify-commit HEAD`, sequential push and exact remote-HEAD
verification. The signature again matches the pinned primary. No helper changed,
so the previously recorded exact-copy fixture results remain applicable; no
runtime regression rerun is claimed for this documentation-only unit. The missing
secret-boundary gate is not treated as a passing test.

Final documentation checks: `git diff --check` passed; a Python Markdown-target
check resolved every local link in the five changed documents. The exact earlier
controller validation archive SHA-256 is
`9d4ace92e2d3088d4dc7fbce577f287d47e00d2bf5a797f07ab0be060675babe`.
Final Forgejo code-page proof of signed HEAD and retained inactive state is
`C:/Users/william/AppData/Local/Temp/cords-ci-keys-20261011/evidence/controller-warning-pushed.jpg`.
Preparing a separate publication repository and consumer migration inventory was
subsequently authorized. The README-only repository was created with Actions
disabled, then superseded by the user's single-monorepo correction below. No
credential migration or runner transition was applied.

## Single-monorepo correction — plan

The sole canonical Forgejo repository is `firebadnofire/cords`. The current
[plan](../ci-monorepo-plan.md) supersedes the split control/publication architecture,
including the conditional controller registration transition. Preserve the two
physical runners and all release targets; no other permanent Cords repository.
Privileged execution requires independent OS/credential and authorization boundaries,
not repository separation, labels, conditions or ignored job `permissions`.

## Single-monorepo correction — implementation

- [Scaffolding inventory](ci-monorepo-consolidation.md): controller HEAD
  `381b2205d856be015ef3b6be8dfb2a06398fc237`, publication HEAD
  `b90cb74451513d68bfe575e764dee85f1aa4f966`. All 11 useful controller helper/pin files
  match canonical Git objects; publication contains only README. Both Actions units
  rechecked disabled. No deletion, repository mutation or credential installation.
- Canonical Cargo repository metadata now points to Forgejo Cords. No package version,
  dependency, product behavior, wire format or vault/storage migration changes.
- Old CI/release YAML retained exactly in `.forgejo/workflow-proposals/legacy-*.yml`,
  outside Actions discovery. Local discovered release workflow contains one explicitly
  skipped activation gate, no signing/registry/GitHub job or secret expression.
  Existing Linux CI remains; direct native Mac and Mac-hosted ARM execution are
  removed from its matrix. Windows ARM cross check routes to the Linux runner.
  Native/ARM release requirements remain in package scripts, preserved definitions
  and policy; their integration gates remain unpassed.
- Inactive `workflow-proposals/ci.yml` prepares all branches, PRs and v/V tags,
  immutable event SHA checkout/verification, credential persistence off for checkout,
  cargo-deny 0.20.2, existing Rust/frontend/PostgreSQL/Compose/Windows checks.
  It is not activated on the unsafe current runners. Action/image digest pinning,
  Linux ARM execution and enforced native Mac backend remain required work.
- `.forgejo/release-policy.json` activation is disabled, credentials external-only.
  Nine package targets, checksums, both OCI destinations/platforms, pinned public
  identities and eight mandatory acceptance checks are explicit. The read-only
  `validate-release-request.py` rejects unknown/duplicate fields, source/pin/version
  mismatches, incomplete/stale references, unsafe entries and altered bytes. It
  explicitly certifies candidate shape/bytes only, never authorization/provenance.
- [External executor contract](../external-release-executor.md): independently
  installed reviewed service/policy revision, protected keys/tokens, independent tag,
  test order and provenance verification, immutable staging, approvals, durable retries,
  every-file `.asc`, both OCI signatures, strict drafts/mirror/finalization. No service
  is installed. Authenticated provenance authority remains an explicit unresolved
  gate. Independently approved native/manual isolated signing is the fallback.
- [Owner inventory](owner-secret-consumers.md): 71 repositories, 386 unique advertised
  commit trees; 11 projects with direct Forgejo/Gitea owner-secret references.
  No values read. All ten owner secrets unchanged. Complete ref accounting is not
  complete runtime-consumer proof: external/dynamic/override consumers still block
  an approved destination mapping and owner-secret migration.

## Single-monorepo correction — validation

Windows: Python **3.13.14**, PyYAML **6.0.3**, Git **2.53.0.windows.2**.
Linux `.54`: Python **3.13.15**, OpenSSL **3.5.3**, GnuPG **2.5.22**,
libgcrypt **1.12.4**, ShellCheck **0.11.0**. Strict SSH host checking retained;
no forwarding/private-key transfer, TLS bypass or host service change.

```powershell
python scripts/test-ci-monorepo.py
python scripts/test-release-request.py
python scripts/test-inventory-owner-secrets.py
python scripts/test-release-contract.py
python -m py_compile scripts/validate-release-request.py scripts/test-release-request.py scripts/test-ci-monorepo.py scripts/inventory-owner-secrets.py scripts/test-inventory-owner-secrets.py
cargo metadata --no-deps --offline --locked --format-version 1
git diff --check
```

PASS: four stored-workflow/policy gates; Windows 10 candidate tests with one explicit
Linux-only symlink skip; five inventory fixtures; seven release-contract regressions;
compilation, offline locked metadata and whitespace checks. All five YAML definitions
parse with PyYAML. `git show HEAD:<workflow>` comparison proves both legacy definitions
preserved exactly. Expanded trigger and exact checkout assertions pass. These are
static stored-definition checks, not hostile-job denial or actual dispatch evidence.

The release job's false condition stops its publication path; it is not proof that
Forgejo never assigns a task payload or transmits inherited owner secrets. That
behavior needs actual protected-environment dispatch evidence. No condition is
accepted as a credential/execution isolation boundary.

A Windows `python scripts/test-release-identity.py` attempt failed three of five
cases because `openssl` was absent from PATH (`WinError 2`); it is not reported as
passing. No product/library weakening was made. The documented Linux signing
environment prerequisites were checked and the full identity suite rerun there.
An initial one-off YAML checker had a Python Path/string precedence error; corrected
the checker and reran successfully before continuing implementation.

The LF-normalized, non-secret Linux validation archive is retained at
`C:/Users/william/AppData/Local/Temp/cords-ci-keys-20261011/monorepo-validation.tar`.
It contains Cargo.toml, policy, public pins, release Python/shell helpers and inventory
fixtures. SHA-256: `5e9af9eb925c95317b50d295a0e08a38f6480c5638cd38380c321463b1ac3a5d`.
Only repository metadata's URL changed after that archive; version/dependencies did
not. This archive is fixture evidence, not release provenance.

```powershell
scp.exe -o BatchMode=yes -o StrictHostKeyChecking=yes C:/Users/william/AppData/Local/Temp/cords-ci-keys-20261011/monorepo-validation.tar william@192.168.86.54:/home/william/cords-ci-review-20261011/monorepo-validation.tar
```

Exact Linux payload, piped through existing strict SSH (PowerShell CR stripped at
the boundary: `tr -d "\015" | bash`):

```sh
set -euo pipefail
cd /home/william/cords-ci-review-20261011
printf '%s\n' '5e9af9eb925c95317b50d295a0e08a38f6480c5638cd38380c321463b1ac3a5d  monorepo-validation.tar' | sha256sum --check
mkdir -p monorepo-validation
cd monorepo-validation
tar -xf ../monorepo-validation.tar
python3 scripts/test-release-request.py
python3 scripts/test-inventory-owner-secrets.py
python3 scripts/test-release-contract.py
python3 scripts/test-release-identity.py
bash scripts/test-release-contract.sh
bash -n scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh scripts/test-release-contract.sh
shellcheck scripts/sign-release-artifacts.sh scripts/verify-release-artifacts.sh scripts/test-release-contract.sh
```

PASS: transferred archive hash; 11 candidate tests including real symlink rejection;
5 inventory tests; 7 contract tests; 5 real public-identity tests. Shell signing suite
passes its additional seven contract tests and ten actual GnuPG rejection scenarios:
wrong public/private pins, private key not authoritative, missing config, invalid
base64, missing signature, altered asset/retry/signature and unpublished signing
subkey. Every fake fixture asset is signed; repeat signature bytes preserved. Shell
syntax and ShellCheck pass. No production private key or publication token used.

### Activation and rollback boundaries

Changes are local/unpushed. Therefore the revised YAML stop is not a claim that the
remote canonical legacy workflow or Global runner is administratively disabled.
Do not create a release tag/dispatch or enable either scaffolding repository.
No hosted pipeline, hostile dispatch, real external signer, protected-tag API proof,
credential migration or complete secure artifact transfer is claimed. The current
Global Mac runner can still be targeted by an arbitrary eligible repository's
workflow; local workflow editing cannot enforce that denial. An OS/executor boundary
must be validated before native CI activation. The current owner inheritance and
shared Linux daemon prevent credential-free ordinary/release execution claims.

No external release side effects occurred. Local code/docs rollback is possible,
but must not restore privileged workflow activation or direct untrusted native host
execution before security gates pass. Existing services, runners, deployments,
credentials and release history are preserved. Repositories are prepared as redundant
scaffolding; removal requires a separately approved backup/metadata check and deletion.
C0 remains incomplete pending native macOS GUI evidence; C1 remains inactive.

Final preparation checks also pass: `cargo fmt --all -- --check`, repeated four
stored-workflow gates, JSON parse and UTF-8/local-target validation for 12 changed
documents. No Rust product source changed, so no additional product integration
run is claimed for this CI-only unit. The complete prior C0 runtime evidence and
remaining native Mac GUI blocker remain in `rework-c0.md`.
