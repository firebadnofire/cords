# Cords local builders

Desktop builders compile the Qt 6 Quick/QML frontend and Rust bridge. They do
not run npm or build Tauri. Workflow parity remains in progress; see
`docs/plans/qt-desktop-migration.md`.

| Script | Host | Output under `dist/` |
|---|---|---|
| `windows-client.ps1` | Native Windows x64/ARM64, MSVC | Portable Qt ZIP |
| `linux-client-tar.sh` | Native Linux x64/ARM64 | Deployed Qt directory tarball |
| `mac-client-pkg.sh` | Native macOS Intel/Apple Silicon | Qt application PKG |
| `mac-client-dmg.sh` | macOS with both Rust targets and universal Qt SDK | Universal Qt DMG |
| `linux-windows-client-crosscomp.sh` | Linux x64, Clang/MSVC cross-toolchain | Portable Windows x64 Qt ZIP |
| `linux-server-tar.sh` | Linux x64/ARM64 | Server tarball |
| `mac-server-tar.sh` | macOS Intel/Apple Silicon | Server tarball |
| `linux-server-oci-img.sh` | Linux with Docker/Podman | Locally loaded server image |

File artifacts include SHA-256 sidecars. Nothing is published. Server builders
do not require Qt. There is no Windows server builder.

## Desktop prerequisites

- Rust 1.97.1 and Cargo, selected by `rust-toolchain.toml`.
- Qt **6.10.3**, including Core, Gui, Network, Qml, Quick, QuickControls2,
  Widgets, Concurrent, Test and QuickTest; SDK architecture must match the build.
- CMake >=3.24, Python 3 and a native C++ compiler (MSVC 2022, GCC or Apple Clang).
- Native platform dependencies required by Qt. Linux also needs `pkg-config`,
  OpenGL development libraries and X11/Wayland platform plugin dependencies.
- Unix packaging needs `tar` and `sha256sum` or `shasum`.

Set `CORDS_QT_PREFIX` to the SDK folder containing `bin/qmake` or
`bin/qmake.exe`. `QT_ROOT_DIR` is also accepted. Builders verify the exact SDK
version and fail on missing dependencies. Windows offers approved WinGet setup
as described below; Unix builders do not install system packages. Builders do
not fall back to the old frontend.

Every entry point prints its complete, script-specific requirements and setup
guidance when a prerequisite or preflight/build command fails, alongside the
original error. These lists are also available in `build-scripts/requirements/`.
Printing setup commands does not execute them. Windows retains its explicit
interactive installation prompt and noninteractive safeguards below.
On Unix, `python3 scripts/test-build-requirements.py` checks missing-dependency
reporting for all seven shell entry points and failure/cleanup propagation.
On Windows, `pwsh -NoProfile -File scripts/test-build-requirements.ps1` checks
the noninteractive/redirected-input safeguards and original error reporting.

## Windows ZIP

```powershell
$env:CORDS_QT_PREFIX = 'E:\Qt\6.10.3\msvc2022_64'
& .\build-scripts\windows-client.ps1
```

Use PowerShell, the native MSVC Rust toolchain, Visual Studio 2022 C++ tools and
redistributables, Python 3 and Windows `tar.exe`. Visual Studio's CMake is
found when CMake is absent from PATH. Optional `-QtPrefix`, `-CMake` and
`CORDS_PYTHON` select explicit tools. `VCINSTALLDIR` is discovered through the
Visual Studio installer when unset.

Missing prerequisites show the installation commands and ask `Y/n`; pressing
Enter selects Yes. Approval installs the relevant package through WinGet,
refreshes this process's PATH, rechecks the dependency and continues the build.
Declining stops the build. Installer failures stop immediately.

For Qt, WinGet supplies `miurahr.aqtinstall`; the approved setup then downloads
Qt 6.10.3 with the matching MSVC architecture and required archives into
`$env:LOCALAPPDATA\Cords\Qt`. Subsequent builds detect that SDK automatically.
If Qt is already installed elsewhere, use `-QtPrefix` to select it.

Use `-NonInteractive` for automation: missing prerequisites then fail with the
installation commands instead of prompting or installing. CI and redirected-input
runs also disable installation prompts. Existing complete setups do not prompt.

Extract the complete ZIP and run `Cords.exe`. Preserve Qt DLLs, plugins, QML
modules and `share/cords/migrations/sqlite` in their packaged layout. WebView2
is not used. Build success does not establish clean-machine runtime acceptance
or signed Windows distribution.
Qt deployment includes the MSVC redistributable installer when Visual Studio
provides it. A machine without that runtime may require the installer before
launching; the builder does not install it on the build host.

For output on another drive:

```powershell
$env:CORDS_QT_BUILD_DIR = 'E:\cords-build\qt'
$env:CARGO_TARGET_DIR = 'E:\cords-build\cargo'
$env:CORDS_DIST_DIR = 'E:\cords-build\dist'
& .\build-scripts\windows-client.ps1
```

## Linux tarball

```sh
export CORDS_QT_PREFIX="$HOME/Qt/6.10.3/gcc_64"
bash build-scripts/linux-client-tar.sh
```

Extract the complete folder and run `bin/Cords`. Qt libraries/plugins and
migrations are deployed in the archive; it is no longer an AppImage. System
C library, graphics, font and display-server compatibility depends on the build
host and target distribution.
X11, Wayland and the headless acceptance backend are included explicitly.

## Linux-to-Windows ZIP (supported x64 cross-build)

```sh
export CORDS_QT_HOST_PREFIX="$HOME/Qt/6.10.3/gcc_64"
export CORDS_QT_WINDOWS_PREFIX="$HOME/Qt/6.10.3/msvc2022_64"
bash build-scripts/linux-windows-client-crosscomp.sh
```

Install Rust 1.97.1 with `x86_64-pc-windows-msvc`, cargo-xwin 0.23.1,
CMake >=3.24, Ninja, Python >=3.9, and LLVM tools (`clang-cl`, `lld-link`,
`llvm-lib`, `llvm-rc`, `llvm-mt`). Both Qt SDKs must be **6.10.3**, including
the desktop components listed above. The Windows SDK is the x64 MSVC 2022
edition; aqtinstall can download it on Linux. The Linux SDK supplies native
MOC, RCC and QML tools, while the Windows SDK supplies headers and libraries.
Put rustup's Cargo ahead of any distribution Cargo in PATH.

The builder uses `cargo xwin env` and its generated CMake toolchain. cargo-xwin
caches Microsoft's CRT and Windows SDK under the user's cache directory; its
use is subject to the [Microsoft license described upstream](https://github.com/rust-cross/cargo-xwin#cargo-xwin).
The builder does not install system packages or require Wine. It compiles the
Rust bridge, Qt application and Windows test executables, then runs QML lint
with the Linux host tools. Windows runtime tests must run separately on Windows;
cross-build success alone does not mean those tests passed.

To run the cross-built media/controller suites on Windows, set
`CORDS_WINDOWS_TEST_SOURCE_ROOT` to the Windows checkout path and
`CORDS_WINDOWS_TEST_OUTPUT_DIR` to an existing Windows capture directory before
building (use forward slashes, for example `C:/work/cords`). Copy the three
`Cords*Tests.exe` files from the build directory to a separate Windows test
directory, use the deployed Qt DLL/plugin/QML paths, and run them there. Fixtures
must match the source snapshot compiled on Linux. Without these overrides,
fixture paths default to the build checkout, as they do for native builds.

The ZIP uses the same name and directory layout as the native Windows builder.
It includes the release Qt DLLs and complete release plugin/QML trees so dynamic
imports and styles remain available without running Windows deployment tools
on Linux. This makes the archive larger than a minimal native deployment.
The target needs the Microsoft Visual C++ 2015-2022 x64 runtime. Optionally set
`CORDS_VC_REDIST` to an existing redistributable installer to include it in the ZIP.
No installer is executed. Extract the complete folder and run `Cords.exe`.

`CORDS_QT_BUILD_DIR`, `CARGO_TARGET_DIR`, `CORDS_DIST_DIR`, `CORDS_QT_PROFILE`
and job controls work as above. Cross output defaults to `target/qt-windows-cross`
and two parallel jobs. Use a separate build directory from native builds.
ARM64 cross-builds and clean-machine/signing acceptance are outside this path.
The [October 20 validation report](../docs/reports/build-scripts-cross-validation-2026-10-20.md)
records the `.54` build and Windows execution evidence.

## macOS packages

```sh
export CORDS_QT_PREFIX="$HOME/Qt/6.10.3/macos"
bash build-scripts/mac-client-pkg.sh
bash build-scripts/mac-client-dmg.sh
```

Both paths deploy Qt frameworks/plugins and migrations inside `Cords.app`.
Xcode command-line tools and `codesign` are required. PKG also needs `pkgbuild`
and `pkgutil`; DMG needs `hdiutil`, `lipo` and both `aarch64-apple-darwin` and
`x86_64-apple-darwin` Rust targets. The DMG builder verifies both architectures
in every deployed Mach-O dependency.

Apps receive ad hoc signatures by default. `APPLE_SIGNING_IDENTITY` selects
an installed distribution identity. `CORDS_PKG_SIGN_IDENTITY` selects an
installer identity. Builders verify package contents before checksumming.
They do not notarize, install into `/Applications`, or change Gatekeeper policy.
Native macOS execution and distribution acceptance remain separate gates.

## Controls and validation

Desktop builds default to optimized Rust `release`. Set `CORDS_QT_PROFILE=dev`
(or Windows `-Profile dev`) for development validation. `CORDS_QT_BUILD_DIR`,
`CARGO_TARGET_DIR` and `CORDS_DIST_DIR` select output folders. Unix
`CORDS_BUILD_JOBS` bounds CMake parallelism; Cargo respects `CARGO_BUILD_JOBS`.
Native builders run QML lint and native/QML tests before packaging. The Linux
Windows cross-builder runs host QML lint and builds the Windows test executables;
execute those tests on Windows as described above.

`scripts/build-client-qt.py` is the common build/test/install helper. Native OS
integration, clean-machine dependencies, signing and publication require
separate acceptance evidence.

## Server artifacts

```sh
bash build-scripts/linux-server-tar.sh
bash build-scripts/mac-server-tar.sh
bash build-scripts/linux-server-oci-img.sh
```

Tarballs include the executable, PostgreSQL migrations, example config, README
and license. Replace example origins, credentials and paths before use. No
persistent data is packaged. OCI uses `deploy/Dockerfile`, prefers Docker over
Podman, loads locally and never pushes. Override its name/tag using
`CORDS_IMAGE_NAME` and `CORDS_IMAGE_TAG`. Docker copies the Qt Rust workspace
member for manifest resolution; building the server does not need a Qt SDK.

Server tarball builders honor `CARGO_TARGET_DIR` for compilation and staging.
On Unix, run `python3 scripts/test-server-builders.py` to check custom target
paths, archive contents, checksums and repeated packaging with a fake compiler.
This regression check does not establish native compilation or runtime acceptance.
