# CI/CD rework evidence — 2026-10-11

Status: **incomplete, not activated**. No hosted workflow, production release, tag,
registry object or shared gh-sync repository was modified by these local tests.
The Cords working-tree changes have not been committed or pushed.
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
