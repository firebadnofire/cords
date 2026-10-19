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
| `linux-windows-client-crosscomp.sh` | Linux | Explicit unsupported-toolchain error |
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
host and target distribution. A Qt/MSVC Linux-to-Windows toolchain is not yet
validated; use the native Windows script for Windows artifacts.
X11, Wayland and the headless acceptance backend are included explicitly.

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
Builders run QML lint and native/QML tests before packaging.

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
