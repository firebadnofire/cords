# Linux-to-Windows Qt build validation — October 20, 2026

`linux-windows-client-crosscomp.sh` is now a supported Linux x64 to Windows x64
builder. The native Qt client was compiled and packaged on `192.168.86.54`,
then executed and tested on the supplied Windows host. This replaces the
unsupported wrapper recorded in the October 19 audit.

## Plan

Use the existing native Qt Quick/QML and Rust/CXX architecture. Configure the
MSVC ABI cross-toolchain, build on Linux, package the Windows runtime, then
validate the extracted artifact on Windows. Keep prerequisite installation
user-local, bound concurrency to two jobs, and verify repeated packaging.

## Implementation

- The shell entry point checks prerequisites and consumes `cargo xwin env`.
  That environment supplies Windows SDK/CRT paths and a matching CMake toolchain.
- `scripts/build-client-qt-cross.py` selects the Windows MSVC Qt SDK and matching
  Linux Qt host tools. It builds the application and three native test executables,
  runs host QML lint, and installs the application/data into a temporary directory.
- The cross-package includes the release Qt DLLs, plugin and QML trees, migrations,
  documentation and avatar-codec licenses. A portable `qt.conf` selects bundled
  paths. ZIP timestamps are clamped to the format's 1980 minimum because Qt SDK
  archives contain earlier timestamps. Packaging uses a temporary ZIP followed
  by replacement; failures propagate and successful artifacts receive SHA-256
  sidecars. No Windows deployment executable or Wine is required on Linux.
- CMake's native Qt deployment remains enabled by default. The cross-helper
  explicitly disables it and supplies its own runtime packaging. Test-only source
  and capture path overrides let cross-built tests use fixtures on Windows;
  native defaults retain the existing checkout/build paths.

Environment: openSUSE x86_64 Linux at `.54`, Rust/Cargo **1.97.1**, cargo-xwin
**0.23.1**, Clang/LLVM **23.1.3**, CMake **3.31.6**, Ninja, Python 3.13 and Qt
**6.10.3** for both `gcc_64` host tools and `msvc2022_64` target libraries.
The Windows Rust target, Ninja and target Qt SDK were installed under the user's
existing tools/cache/audit directories. No system package installation was used.
The Microsoft SDK/CRT cache already existed. The target requires the Microsoft
Visual C++ x64 runtime; this validation host has it installed.

Source and logs are retained under `~/cords-build-audit-20261019/` on `.54`.
The final `cross-source-manifest.json` and logs are copied into local
`target/build-script-audit-20261019/`. All **246** relevant application, Rust,
fixture, migration and builder inputs checked against that remote manifest matched
the local working tree. Concurrent unrelated edits were preserved.

## Validation

| Check | Result |
|---|---|
| Linux configure/build | Passed; Rust MSVC bridge, Windows application and all three Windows test executables linked |
| Host QML lint | Passed |
| ZIP integrity, SHA-256 and migrations | Passed; 1,767 archive files; packaged SQLite migrations match source bytes |
| Repeated builder runs | Passed; final repeat archive contains identical file names and contents; generated-file timestamps change the ZIP hash |
| Windows `--version` | Exit 0, `Cords 0.1.0` |
| Windows `--list-accounts` | Exit 0, empty isolated registry |
| Windows timed UI startup/capture | Exit 0, empty stderr, account-picker capture inspected |
| Cross-built media suite | 8 passed, 0 failed |
| Cross-built controller suite | 12 passed, 0 failed, 1 skipped |
| Cross-built row-filter suite | 3 passed, 0 failed |
| Windows QML-controls suite | 20 passed, 0 failed |

The live-server admission case was skipped because no admission fixture was
provided. Controller tests emitted temporary-directory cleanup warnings; the
row-filter suite emitted an invalid-type warning. Those warnings are retained
in the logs and were not treated as evidence of a failed assertion or repaired
as part of this build-script change.

Package startup used a restricted system PATH, no Qt SDK overrides and an isolated
account registry. Runtime suites used the deployed package's DLL/plugin/QML paths;
the standard Windows `qmltestrunner.exe` came from the matching Qt SDK. Test fixture
and capture paths were configured for the Windows checkout before cross-compilation.
The cross-builder itself does not claim to run Windows runtime suites on Linux.

Validated local artifact:
`dist/cross/cords-client-windows-x86_64-0.1.0.zip` (**52,440,917 bytes**).
SHA-256: `24df24a3cf658d524fb9a09ab43afbf829757d308e087dd1849359f359565c2e`.
Evidence includes `windows-cross-validation.json`, `cross-repeat-validation.json`,
the four `cross-*.txt` test logs, Linux build logs and `windows-cross-startup.png`
under the local audit directory.

This establishes compilation, packaging and runtime tests on the supplied Windows
machine. It does not establish clean-machine redistributable installation,
ARM64 cross-building, live network/admission acceptance, signing or publication.
