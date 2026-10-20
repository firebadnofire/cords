# Cords split-host E2E

This harness executes Linux acceptance on `192.168.86.54` and coordinates native
Windows Qt testing on the workstation. Python orchestrates real `cords-dev`
processes; Rust owns identity, authorization, signatures, MLS and storage.
`coverage.json` lists both implemented scenarios and missing acceptance coverage.
A successful VM boot or build is never an application acceptance result.

## Plan and execution boundaries

Work in independently validated units: framework/storage, snapshot/build/deploy,
independent clients, native GUI, evidence/cleanup. Stop at a failed gate and inspect
that run's report before starting dependent work. See `VALIDATION.md` for executed
milestones, failures and limitations.

Prerequisites on `.54`: the existing libvirt default network, working passwordless
sudo, QEMU/libvirt/virt-install, cloud-localds, guestfish, SELinux management tools,
Docker, Git, Python 3.12+, OpenSSL, Rust 1.97.1 and the existing package cache on
`192.168.122.1:3142`. The workstation needs Python 3.12+, OpenSSH/scp and an unlocked
interactive Windows session for GUI work. No existing Windows VM is used.

The framework is cloned as the SSH user into `~/codex-cloud-init-automation`, pinned
to `86a4c86557d8588453d3589c2aa394e28c4c0172`. Setup checks the ownership marker and
rejects unexpected edits. `framework.py` reproduces the checked-in
`framework-paths.patch` as remote `framework.patch` and
records reviewed upstream files and the patch hash in `framework.json` on `.54`.
The existing framework, SDP example, Cords checkout and unrelated VMs are preserved.

## Commands

Run from the Cords checkout; a commit or push is unnecessary:

```powershell
$env:CORDS_PYTHON = 'C:\path\to\python.exe'
.\test-e2e.ps1 infrastructure
.\test-e2e.ps1 smoke
.\test-e2e.ps1 scenario messaging
.\test-e2e.ps1 scenario public
.\test-e2e.ps1 scenario permissions
.\test-e2e.ps1 scenario profiles
.\test-e2e.ps1 resilience
.\test-e2e.ps1 full
.\test-e2e.ps1 gui
```

Linux entry point: `bash test-e2e.sh <same arguments>`. Both entry points always
send Linux work to `.54`. `--skip-build` requires matching application input hashes
and matching cached binary checksums; it cannot bypass the build gate.

Exit statuses are `0` for all selected assertions passing, `1` for application
assertion failure, `2` for infrastructure/build/provisioning/collection error,
and `3` for unavailable required coverage. `full` currently returns `3` when its
implemented scenarios pass because the inventory still has coverage gaps.
Do not interpret that status as full acceptance.

Snapshots include tracked working-tree bytes and explicitly selected untracked
files under `tests/e2e` plus the two entry points. Other untracked files are excluded.
Potential secret filenames fail closed. Generated outputs are excluded. Each
manifest records HEAD, tracked diff, included untracked paths, per-file SHA-256
and deterministic archive SHA-256. Review the manifest before sharing evidence.

## Ubuntu builds

All Windows cross-compilation runs inside an owned Ubuntu 24.04 guest on `.54`.
The inspected base SHA-256 is
`04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94`.
If the upstream rolling release URL changes, setup refuses the new checksum;
restore the pinned cached image or explicitly review a new pin.

On `.54`, after framework setup:

```sh
python3 /mnt/1tb-disk/cords-e2e/tools/host.py build-guest
python3 /mnt/1tb-disk/cords-e2e/tools/guest_build.py \
  /mnt/1tb-disk/cords-e2e/runs/cords-e2e-<id>/cords-e2e-<id>-ubuntu.json \
  --archive /mnt/1tb-disk/cords-e2e/sources/<sha>.tar.gz \
  --manifest /mnt/1tb-disk/cords-e2e/sources/<sha>.json --platform windows
```

Use `--platform linux` for the supported Linux Qt client builder and its four
component/native test groups. These fixture checks remain distinct from live GUI
acceptance. `provision-builder.sh` installs declared dependencies without package
upgrades and records installed versions. LLVM 19 is needed by the inspected MSVC
headers; Rust is 1.97.1 and cargo-xwin is 0.23.1. The controller locks the guest,
verifies transferred archives, and verifies every extracted SDK and source file.
Build directories are distinct for each source snapshot. Artifact directories are
also keyed by source hash. Extraction refreshes input timestamps after verifying
bytes so deterministic archive timestamps cannot hide changed files from Cargo.

The existing Qt 6.10.3 SDKs must be staged as
`cache/guest-transfer/qt-linux.tar` (top-level `gcc_64`) and `qt-windows.tar`
(top-level `msvc2022_64`). Their inspected SHA-256 values are respectively
`ef2a2fea4ca717ac6df320e54ec843c07e6aa0f50c041b293045df3b25df51db` and
`381a4001c8fb3aa99b7631d7e8aebfabc596b7c1868309911a71a9cfef31db65`.
Provisioning a different SDK is an explicit dependency update, not an automatic
fallback. Artifacts and receipts are copied back to the owned run before teardown.

An optional `cache/guest-transfer/rust-dependencies.tar` restores only
`.cargo/registry` and `.cache/cargo-xwin`. Its adjacent
`rust-dependencies.sha256` is required and verified before transfer and extraction.
Python's data extraction filter rejects escaping links; the generated absolute
`clang-cl` alias is excluded from portable exports and recreated by the builder.
Before retiring a builder, with no build running, export this cache on `.54`:

```sh
cd /mnt/1tb-disk/cords-e2e/tools
python3 -c 'from host import VM; from pathlib import Path; from guest_build import save_dependency_cache; save_dependency_cache(VM.recover(Path("/mnt/1tb-disk/cords-e2e/runs/cords-e2e-<id>/cords-e2e-<id>-ubuntu.json")))'
```

The export omits Cargo credentials and compiled targets. The existing APT proxy,
pinned SDK archives and declared Rust/xwin cache can be reused by fresh guests.

## Live Windows GUI

`gui` creates the real server and three Linux clients, then prints the local
`gui-context.json` and private `gui-ca.crt`. Its bridge is bounded to 20 minutes.
It opens one temporary `.54` firewall rule for that TLS port from the SSH
workstation address only; cleanup removes that exact rule. It expires by itself
if orchestration is interrupted.

Launch the complete verified ZIP using `windows-profile.ps1`, with a fresh
`profile-*` directory for each run and the run's CA. The launcher scopes
`CORDS_CLIENT_STATE`, AppData, LocalAppData, temporary files and Qt disk caches.
It records the owned PID and executable. `-ListAccounts` supports profile-isolation
checks. No system trust store is changed and certificate verification stays enabled.

`windows-sky.mjs` is the reusable Computer Use UIA driver. Select the exact owned
executable, observe, inspect, perform one action, and observe again. It saves UIA
trees and screenshots. Existing stable identifiers include `accountPassword`,
`accountSubmit`, `messageComposer` and `sendMessage`. Native UIA text input works;
indexed clicks have exhibited unreliable geometry in this environment, so the
executed preflight used screenshot-derived button coordinates. This remains a
limitation for unattended GUI acceptance.
Use `clickAt` only with coordinates inspected in the current screenshot. For
editors using QML `textEdited`, use real `typeText`/`pressKey` after focusing and
inspecting the control: UIA `set_value` alone does not mark those editors dirty.

Create a disposable account through the GUI, connect to the context origin, and
observe pending admission. The Linux owner reads `membership-requests`, verifies
the GUI device, then approves it. In the GUI, check approval and publish a fresh
KeyPackage before the owner adds that device to the encrypted route. Synchronize
and load messages through the GUI. A server operation alone is not GUI evidence.

File RPC example, invoked with `python tests/e2e/gui_request.py --context <context>
--request <request.json>`:

```json
{"action":"client","client":"a","operation":"send","fields":{"route":"<context route>","body":"Linux to Windows marker"}}
```

Observe the decrypted marker in the GUI, then send a different marker through the
GUI composer. Assert it on independent Linux client `b`:

```json
{"action":"assert-peer-message","client":"b","route":"<context route>","body":"Windows to Linux marker"}
```

The assertion waits up to 45 seconds for actual peer delivery. Finish with
`{"action":"finish"}`. Copy screenshots, UIA assertions and package/source
receipts to the remote run's `gui/windows` directory before finishing. Partial GUI
proof never clears the full-suite unavailable result automatically.

## Storage, isolation and security

All owned base images, VM overlays, UEFI NVRAM, build caches, snapshots and large
artifacts live under `/mnt/1tb-disk/cords-e2e`. Only NoCloud ISOs use
`/var/lib/libvirt/cloud-init`. Backing paths are absolute; bases are checksummed
and read-only. Names include `cords-e2e-<run-id>` and collision checks precede
resource creation. SELinux remains enforcing; only owned disk paths have a
`virt_image_t` policy. Never recursively restore labels on live VM disk files.

Server instances have dedicated configuration, network, server/database volumes,
container names, image tags, origin and ports. The backend binds loopback; the test
TLS endpoint requires TLS 1.3. The temporary CA is trusted only by test processes;
native clients still verify signed discovery and pin the server signing key.
`CORDS_PREBUILT_IMAGE` is restricted to explicit non-default instances. Never run
an unqualified `cords-up.sh down`, `purge`, or restart as part of this harness.

The suite permits one deployment at a time and limits builds to two jobs. Three
client guests use 1.5 GiB each; a builder uses 6 GiB and a 45 GiB overlay. Runs
require at least 6 GiB available host RAM and 20 GiB free owned storage. Network
faults run only inside manifest-owned disposable guests.

## Evidence and interrupted-run recovery

Remote runs contain JSON assertions/results, JUnit, exact source/environment and
resource manifests, build receipts, server/client logs and network-fault timelines.
The workstation receives reports under `target/e2e/<timestamp>`; large evidence
remains on `.54`. Test CA private keys and encrypted test identities are disposable
secrets: do not publish entire run directories indiscriminately.

Normal cleanup closes clients, safely shuts down guests, extracts required logs
offline and verifies nonempty artifacts before deleting owned disks. A failed
shutdown or missing required result preserves the disk and reports infrastructure
error. Provisioning failures and absent artifacts cannot become application passes.

Run the read-only ownership/storage audit on `.54` after cleanup:

```sh
python3 /mnt/1tb-disk/cords-e2e/tools/audit.py \
  --output /mnt/1tb-disk/cords-e2e/ownership-audit.json
```

`aggregate.py --qt <XML-directory> --output <directory>` and
`aggregate.py --gui <gui-validation.json> --output <directory>` produce the shared
JSON/JUnit reports. Missing XML or screenshot evidence is an infrastructure error.
An explicitly skipped Qt fixture remains unavailable.

After interruption, first inspect the run's manifests, `virsh domstate` and logs.
Wait for any still-running coordinator to finish. Recover only an exact VM
manifest with `VM.recover(path)` and call `stop_collect()`; it refuses unexpected
ownership paths and preserves evidence failures. Retained failure disks are
intentional resources, not automatic deletion candidates. Do not use broad name
globs to delete VMs or disks. Preserve a copy of evidence before any manual purge.
Use `firewall.py <exact-run> --remove` to remove a recorded temporary firewall rule.

Harness self-checks on `.54`:

```sh
cd /mnt/1tb-disk/cords-e2e/tools
python3 -m unittest -v test_harness
```

Failure validation uses temporary expectations, never modified production behavior:
`--expectations wrong-message.json` with `{"first_body":"wrong"}`, or
`scenario permissions --expectations wrong-permission.json` with
`{"unauthorized_create_allowed":true}`. Both must exit `1` with an actionable
scenario assertion. `infrastructure --fixture provisioning-failure` and
`infrastructure --fixture missing-result` must exit `2` and retain failed evidence.

The legacy `scripts/encrypted-acceptance.py` and `scripts/lifecycle-acceptance.py`
also require explicit `--origin`, `--host`, `--deployment-dir`, `--compose-file`,
`--env-file` and `--compose-project`. The project must start with `cords-e2e-`;
configuration files must remain inside its absolute deployment directory. They
no longer select the existing `cords-milestone` deployment implicitly. The new
VM suite is the preferred split-host entry point and handles admission/bootstrap.
