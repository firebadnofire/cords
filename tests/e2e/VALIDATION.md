# Executed split-host validation

Evidence date: 2026-10-20. Linux commands ran on `192.168.86.54` or its
manifest-owned libvirt guests. Native Windows interaction ran in the workstation's
interactive session, using an Ubuntu-guest cross-built complete package. No
existing Windows VM was used. Paths below are relative to
`/mnt/1tb-disk/cords-e2e/runs/` unless stated otherwise.

## Framework and storage

`cords-e2e-20261020-102728-f0df26` completed provisioning, a guest assertion,
graceful shutdown, offline extraction of required artifacts, and deletion of its
owned resources. Repeat setup preserved the live libvirt SELinux label. Framework
revision is `86a4c86557d8588453d3589c2aa394e28c4c0172`; recorded path patch SHA-256
is `fb5227a5c0a3afe20cde987c827503d43c2016ef3478a5e36f15b6bc0bb004f2`.
The framework record includes inspected upstream file hashes. Base and overlay
paths/backing chains are checked in each guest's manifest and domain XML. NVRAM is
also explicitly placed on the 1 TB disk. No SELinux enforcing-mode relaxation was
used. An early recursive relabel caused owned-guest I/O errors; the implementation
now labels directories only and leaves live dynamic MCS labels to libvirt.

## Snapshot, build and deployment

The harness integrity checks run on `.54` prove deterministic dirty-tree snapshots,
explicit untracked inclusion, tamper rejection, fresh extraction timestamps, and
distinct JSON/JUnit failure/error/unavailable results. They also reject an invalid
GUI send-request schema before transmission. These are harness checks, not product
acceptance substitutes.

The full run `cords-e2e-20261020-175628-0d01c0` caught a stale cached `cords-dev`:
the source contained `create-with-mode`, but the binary returned unknown operation.
Deterministic tar timestamps were zero and Cargo reused an older output. The fix
refreshes extracted file timestamps after checking hashes and invalidates version-1
build receipts. Previous receipts must not be used as proof of exact-source builds.

`cords-e2e-20261020-175833-f768ad` rebuilt both Linux binaries from snapshot
`c517308a9622e2e28ef8e1a0424a597397c629cafa216735255aac0b60387452` and then
passed the implemented full-suite scenarios. Its unavailable inventory predates
later additions; individual assertion events identify what actually executed.

The owned Ubuntu builder is `cords-e2e-20261020-100535-ff9d2f-ubuntu`.
Windows cross-build and ZIP verification completed using LLVM 19, Rust 1.97.1,
cargo-xwin 0.23.1, and inspected Qt 6.10.3 host/Windows SDKs. The previous Qt SDK
cache had corrupted files after the early storage failure; verified external SDK
archives restored it. LLVM 18 was incompatible with the inspected MSVC headers;
declared LLVM 19 packages resolved that prerequisite. Per-source Qt build paths
avoid CMake's cross-source cache conflict, and builder locks prevent overlapping
writers. Large SDK/build artifacts remain on the 1 TB disk or in the guest disk.

Deployment uses explicit instance/configuration/image/origin/port settings through
the real `cords-up.sh`; dedicated containers, PostgreSQL and persistent volumes
were created and purged repeatedly. Backend HTTP binds loopback. Clients use the
run's CA with normal certificate validation, TLS 1.3 and native signed discovery /
server-key pin verification. No default-deployment lifecycle command was run.

## Independent clients

`cords-e2e-20261020-175833-f768ad` used three independent disposable client VMs
and fresh password-protected accounts. Its passing assertions cover admission,
rejection/resubmission, encrypted bidirectional traffic, concurrent senders,
ordering, retries/idempotency, offline catch-up, device-signed public messaging,
role catalog create/edit/delete and assignment/revocation, signed profile/avatar
propagation, invalid/oversized inputs, client termination, owned-guest link outage,
100 ms latency / 10% packet loss, server/PostgreSQL restart persistence, invalid
TLS, MLS exclusion and denied sends without peer-history effects.

`cords-e2e-20261020-180439-77a3b1` passed real server-signing-key substitution
behind the same valid TLS endpoint: native Rust returned `CORDS_SERVER_KEY_CHANGED`
and then accepted the restored original pin. The earlier attempt
`cords-e2e-20261020-180246-e3a6e6` exposed a nested resource-directory cleanup
guard; it reported infrastructure error, and its exact leftover deployment/proxy
were collected and removed before retrying. Secondary deployments now have direct
owned run directories and proxy process manifests.

`cords-e2e-20261020-180637-403b55` passed absent/invalid/oversized session-token
denials, malformed JSON/schema rejection, and unchanged authorized channel state.
Wire probes contain no client identity, signing or authentication implementation.

## Native GUI evidence

`cords-e2e-20261020-174004-4e82e9/gui/windows/` contains UIA trees, screenshots,
peer observations, retrieved avatar bytes and `gui-validation.json`. The complete
Windows ZIP used for this run has SHA-256
`8fddb637882ec8415d3a661f795c3eec9f3a21353b6470e64982134226c89738`.
The later rebuilt source differs from this snapshot's application/build inputs
only in `cords-dev` and `cords-up.sh`; GUI/core source inputs were unchanged.

Actually executed through real QML controls: protected account creation, pending
admission denial, approval by the independent Linux owner, KeyPackage publication,
Linux-to-GUI decrypted messaging, GUI-to-Linux plaintext verification, signed
display-name propagation, avatar upload and independent retrieval with matching
350-byte size and SHA-256, restart history/profile persistence, creation of a
second account without the first account's membership/history, switching back to
the original account, and server-driven route exclusion disabling the composer.

The launcher scopes state, vaults, databases, AppData, LocalAppData, Qt cache and
temporary files to a dedicated profile. Real `--list-accounts` execution observed
the two GUI-created accounts in that profile and no accounts in a fresh profile.
It verifies every extracted ZIP file again before reuse.

UIA exposes real QML identifiers and accepts editable values. Indexed click
geometry is unreliable in this environment; inspected screenshot coordinates were
used for buttons. QML dirty-state editors require real keyboard input because
UIA `set_value` does not emit `textEdited`. The reusable runner exposes these
separate operations. The midnight appearance preference was also saved and
visually verified after a process restart. Full unattended GUI acceptance is
not claimed. Earlier GUI driver races/schema mistakes failed and cleaned up;
delivery assertions now wait for actual peer receipt for up to 45 seconds.

Linux Qt component/native groups also ran inside the Ubuntu guest. Offscreen runs
passed all four groups. The first Xvfb attempt found missing xcb runtime libraries;
the second found focus/Escape failures without a window manager. Declared xcb
dependencies and Openbox resolved those environment failures. Under Xvfb + software
rendering + Openbox, QML controls passed 20 cases, media 8, controller 12, and row
filter 3. One live-admission fixture was explicitly skipped because it was not
selected. These fixture checks are separate from live-server GUI acceptance.

## Failure assertions and retained evidence

Temporary wrong-message expectations in `cords-e2e-20261020-104004-8a1d74` and
wrong-permission expectations in `cords-e2e-20261020-104200-bba194` produced
scenario failure, actionable reports and exit 1. Production behavior was not
modified to make those tests fail.

Provisioning-failure fixture `cords-e2e-20261020-104234-fbaf75` and missing-result
fixture `cords-e2e-20261020-104431-f5fcc5` produced infrastructure errors / exit 2.
Their stopped disks remain intentionally retained because required evidence was
missing. An early failed probe is likewise retained. Cleanup never treats missing
results as application passes, and never reads a running guest disk offline.

Full acceptance remains unavailable while required capability gaps in
`coverage.json` remain. An inactive role catalog is not active-policy enforcement;
a headless Ubuntu guest without an OS credential store cannot validate that
identity-protection mode. Reports distinguish these from broken assertions and
environment errors.

## Latest integrated rerun

`cords-e2e-20261020-181859-51c7a9` completed the full Linux selection with 16
passing assertion groups and three explicit unavailable capabilities (exit 3).
It includes both interrupted-send crash boundaries, latency/packet loss, valid-TLS
server-key substitution, malformed/invalid sessions, public/encrypted messaging,
role catalog operations and cross-account avatar/profile rejection without effect.
`cords-e2e-20261020-181539-b7b665` independently passed the profile scenario.
The foreign-profile denial originally lacked an actionable server code because
`cords-dev` only printed the outer error; its development error output now includes
the error chain. The corrected run observed `CORDS_PERMISSION_DENIED` and unchanged
victim state. No production permission rule was changed to make this assertion pass.

Harness integrity checks were repeated on `.54` (three passed). Deliberately
aggregating missing Qt XML returned exit 2 with a structured infrastructure-error
JSON/JUnit report, rather than an empty passing result.

The wrong-message and wrong-permission overrides were repeated against the latest
native binaries: `cords-e2e-20261020-182600-a64d79` failed with peer plaintext
mismatch, and `cords-e2e-20261020-182748-8b8795` failed because the deliberately
incorrect permission expectation contradicted `CORDS_PERMISSION_DENIED`. Both
returned exit 1 and cleaned their owned resources.

Legacy acceptance helpers now require explicit `.54` host, origin, deployment
directory, Compose project/files and environment file. Configuration validation
ran on `.54`: correct quoting/project selection passed; a default project,
escaping environment path and relative deployment directory were rejected. These
checks executed no lifecycle commands. The older legacy scenarios themselves were
not rerun; the VM suite provides the runtime evidence described above.

PowerShell entry point `test-e2e.ps1 smoke --skip-build` completed
`cords-e2e-20261020-182936-0b7131` with six passing assertion groups, exit 0 and
complete owned-resource cleanup.

The final Ubuntu Linux GUI controller run rebuilt the supported client package,
passed all four offscreen groups, then collected Xvfb results (43 pass, one
explicitly unselected live-admission fixture), 39 screenshots and 49 checksummed
artifacts. Collection and receipt writing completed with exit 0. These files are
under the builder run's `artifacts-linux/c517308a9622e2e28ef8e1a0424a597397c629cafa216735255aac0b60387452/`.

Rust registry/xwin dependencies were exported and restored in the real Ubuntu
guest. The first restore rejected an absolute LLVM alias; exports now omit that
generated alias and retain the safe extraction filter. The validated portable
archive SHA-256 is `3cecda3e68126cf0b8ed4fdb7d8407c6ebacf5e72010ce7b0c57eae9fa47eea5`.
Credentials and compiled targets are excluded. Qt SDK archives and this cache
remain on the 1 TB disk for fresh builders.

After artifact hashes were reverified, the builder shut down gracefully, required
logs/results were extracted offline, and its disk/domain/NVRAM were removed.
Repeated cleanup passed. Final `ownership-audit-final.json` on `.54` found no
test containers, volumes, networks or TLS proxy processes. Only the three
intentional stopped failure-evidence guests remain; all their disks, backing files
and NVRAM are on the 1 TB disk. SELinux is enforcing and unrelated guests retain
their prior states. The owned Windows Cords process has also exited.
