# Native build-script validation — October 19, 2026

All seven native build targets were produced and checked on the supplied hosts.
The Linux-to-Windows entry point is now supported and passed the October 20
follow-up build and Windows runtime tests; see
[cross-build validation](build-scripts-cross-validation-2026-10-20.md).
All eight entry points now produce their build targets. The original native
audit fixed four build issues in two source files.

## Plan

Inventory every entry point in `build-scripts/`, run it on its intended host,
retain failures, and verify the resulting packages rather than treating compiler
success as packaging proof. Diagnose failures before retrying. Keep tests enabled,
avoid publishing or installing the resulting products, and preserve concurrent
source changes. The eight entry points are listed below; `lib/common.sh` and
`lib/qt-client.sh` are sourced helpers exercised through those builds, while
`server.toml.example` is packaged configuration data.

## Implementation

Hosts: Windows x86_64 at localhost, Linux x86_64 at `192.168.86.54`
(openSUSE Tumbleweed), and macOS arm64 at `192.168.86.42`.

The initial working tree was based on commit
`59947a9d4809f5b16cb6dce5281e5b932d4da9d3` and included existing uncommitted
changes. A 560-file source snapshot and SHA-256 manifest are retained in
`target/build-script-audit-20261019/`. Remote work is isolated under
`~/cords-build-audit-20261019/`. Later concurrent avatar/editor/test/ledger edits
were not incorporated into the frozen remote snapshot. This report does not
claim to validate those later edits.

Build fixes were overlaid on that snapshot:

- `mac-client-dmg.sh`: use nonempty architecture argument arrays. Apple's Bash
  3.2 fails on an empty array expansion with `set -u`; the original idiom was
  reproduced failing on the Mac, and the replacement was checked for both host
  architectures.
- `mac-client-dmg.sh`: verify each Mach-O architecture separately. The installed
  `lipo` rejected combined `-verify_arch arm64 x86_64` calls even on the universal
  QtCore framework; individual checks of both slices passed.
- `bins/cords-client/qt/CMakeLists.txt`: place Unix native test executables in
  `bin/`. In the build root, the generated application `Cords/qmldir` shadows
  source QML imports and prefers resource URLs absent from the test executable.
  Linux controller tests failed before the fix; the identical executable passed
  an affected case when copied to an isolated directory. Full builder tests were
  then rerun without suppressing or weakening tests.
- `bins/cords-client/qt/CMakeLists.txt`: explicitly link Apple's Security and
  SystemConfiguration frameworks, including in the inherited native test link
  dependencies. The first native Mac client link failed on Security/Keychain
  symbols and `SCDynamicStoreCopyComputerName` from the Rust static archive.

With user approval, CMake 3.31.6, aqtinstall 3.3.0, and Qt 6.10.3 were installed
inside the remote audit directories using Python virtual environments and Qt SDK
downloads. Both Apple Rust targets were verified installed. The user reviewed and
accepted the Xcode license; a minimal C program then compiled and ran successfully.
No host system package installation was performed. Native Cargo builds used the pinned
Rust 1.97.1 toolchain and two Cargo jobs on the remote hosts.

## Validation

| Entry point | Result | Evidence |
|---|---|---|
| `windows-client.ps1` | Passed | Release ZIP; four CTest suites passed; extracted executable launched successfully. |
| `linux-client-tar.sh` | Passed after test-location fix | Four CTest suites passed; deployed runtime tarball; extracted offscreen launch passed. |
| `linux-server-tar.sh` | Passed | Native ELF tarball; checksum, contents, 16 migrations, `--version` and `--help` verified. |
| `linux-server-oci-img.sh` | Passed | Image loaded locally; non-root user, 16 migration hashes and executable startup verified. |
| `linux-windows-client-crosscomp.sh` | Passed in October 20 follow-up; supported x64 cross-build | Replaced the initial unsupported wrapper; Linux-produced Windows Qt ZIP and four Windows test suites passed. See the linked cross-build report. |
| `mac-server-tar.sh` | Passed after Xcode license acceptance | Native arm64 Mach-O tarball; checksum, contents, 16 migrations, `--version` and `--help` verified. |
| `mac-client-pkg.sh` | Passed after link fixes | Four CTest suites passed; PKG expanded and signature verified; extracted native Cocoa startup passed. |
| `mac-client-dmg.sh` | Passed after compatibility/link fixes | Both release architectures built; four native CTest suites passed; all deployed Mach-O slices verified; signed mounted-image app launched successfully. |

Windows and Linux extracted-client checks used an empty, isolated account registry,
removed SDK environment overrides, and a restricted PATH. `--version`,
`--list-accounts`, and a timed startup/capture all exited zero with empty stderr.
Linux used Qt's offscreen/software backend; its dependency inspection reported no
missing libraries and resolved Qt/ICU from the extracted package. All five SQLite
migrations were compared byte-for-byte. Startup captures were inspected.

The expanded macOS PKG independently passed deep/strict code-signature verification,
SQLite migration comparisons, and the same three executable checks using native
Cocoa with an isolated registry and restricted PATH. Its startup capture succeeded
with a four-second capture delay and eight-second quit deadline. All executable
checks exited zero with empty stderr. The main executable's Qt load commands refer
to bundled frameworks. An initial forced-offscreen smoke attempt aborted; native
Cocoa was then used successfully.
`pkgbuild` emitted five `write: Permission denied` lines while still returning zero.
Their origin is unconfirmed; subsequent expanded-payload signature, migration and
runtime checks passed. These diagnostics are preserved in the build log.

The universal DMG independently passed image verification, read-only mounting,
deep/strict app-signature verification, both main-executable architecture checks,
migration comparisons, and native ARM64 startup from the mounted image. The PKG
and DMG final smoke checks used Cocoa with the default renderer and no SDK path
overrides; all three commands per package exited zero with empty stderr. Both
captures were checked as newly written during the final run and inspected. Qt
framework load paths point inside each bundle. The image detached successfully.
macOS 27 emits deprecation warnings for `hdiutil attach`/`detach`; those commands
still succeeded, and the scripts retain compatibility with older macOS tooling.

The OCI image was exercised with `--network none --read-only --cap-drop ALL`.
It runs as `4848:4848`. Because the remote source snapshot omits `.git`, its
revision label is `unknown`; source provenance is recorded separately above.
Image ID: `sha256:2aa5ed887f92c28dc76497513b96c9002558f7e3b4284a29fa95c6053332adc6`.

The Unix server-builder regression test passed on Linux, covering both platform
packagers, custom Cargo target paths containing spaces, checksums, contents and
repeated packaging using a fake compiler. That regression is separate from the
real native builds above. Shell syntax validation also passed.

An intermediate Windows repeat hit MSVC C1083 permission denied on an object file.
The same file subsequently opened for writing without changing bytes or permissions;
a retry passed all four suites and produced another package, which independently
passed extraction and launch checks. The transient lock's origin is unconfirmed.
An auxiliary Linux repeat accidentally selected system Rust 1.99 and was explicitly
stopped; it is not counted as build proof. The successful native Linux builds used
Rust 1.97.1.

Raw logs, exit records, JSON artifact checks, diagnostic probes and captures are in
`target/build-script-audit-20261019/`, with remote logs copied into `linux-logs/`
and `macos-logs/`. Completed file artifacts and their checksum sidecars are copied
into local `dist/`; the OCI image remains on the Linux host. Build caches and the
approved user-local SDKs are retained for reproducibility.

These checks do not establish clean-machine distribution compatibility, real
Linux X11/Wayland interaction, Intel Mac execution, live PostgreSQL service
acceptance, notarization, trusted distribution signing, or publication. Existing
unavailable platform integrations remain unavailable. No test was bypassed to
produce a package. The controller suite's live-admission case retains its normal
skip when no explicit disposable network/database fixture is selected.

## File artifacts

All six files and their SHA-256 sidecars are available under local `dist/`.

| File | Bytes | SHA-256 |
|---|---:|---|
| `cords-client-linux-x86_64-0.1.0.tar.gz` | 73218870 | `959bc506fb291ebf0bd0a3e58dfe0590dde5a07a9794cfdf9bcc8edbbe8109e3` |
| `cords-client-macos-aarch64-0.1.0.pkg` | 62467847 | `de5c8fc6e2e439c76820ca14843798b9de0466f45c603db82abca1522fbbf646` |
| `cords-client-macos-universal-0.1.0.dmg` | 91215395 | `511acd6c2f90253a009e8b01ab76be3b0886ba4cf54c4059c0ebb6ab66ca2f02` |
| `cords-client-windows-x86_64-0.1.0.zip` | 69299402 | `989d7d8832545fdd9f94c2f0c599ffc0608014c47b968030ab30e1b9357291b1` |
| `cords-server-linux-x86_64-0.1.0.tar.gz` | 11327948 | `81c7474673783ecc9b811028c828fca80a1252d3b19400798ad7982a4824ee20` |
| `cords-server-macos-aarch64-0.1.0.tar.gz` | 9850721 | `7c7f6e20a3a69b575798c8d239a89e0ac22e03aa83a6295b61e4250b0958162e` |
