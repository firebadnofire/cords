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

## Linux container builds

On Fedora/Bazzite, install rootless Podman and verify `podman info` works as your
normal user. No sudo, privileged container, host socket, or host compiler/SDK is
needed. On Docker hosts, verify `docker info` works for the invoking user. The
scripts prefer Podman if both engines are installed; set `CONTAINER_ENGINE=docker`
(or `podman`) to choose explicitly. An unavailable selected engine fails without
fallback. Containerfiles pin Rust 1.97.1, Qt 6.10.3, CMake 3.31.6, aqtinstall
3.3.0, cargo-xwin 0.23.1, and LLVM 19 for the Windows cross build. Base OS packages use Debian Bookworm's package
repository. Base image tags and apt package revisions should be refreshed as a
reviewed release dependency update.

Run any script from any directory, for example:

```sh
bash /path/to/cords/build-scripts/linux-client-tar.sh
bash /path/to/cords/build-scripts/linux-windows-client-crosscomp.sh
bash /path/to/cords/build-scripts/linux-server-tar.sh
bash /path/to/cords/build-scripts/linux-server-oci-img.sh
CONTAINER_ENGINE=docker bash /path/to/cords/build-scripts/linux-client-tar.sh
CONTAINER_ENGINE=docker bash /path/to/cords/build-scripts/linux-windows-client-crosscomp.sh
CONTAINER_ENGINE=docker bash /path/to/cords/build-scripts/linux-server-tar.sh
CONTAINER_ENGINE=docker bash /path/to/cords/build-scripts/linux-server-oci-img.sh
```

The first build downloads a versioned build image and, for Qt targets, the Qt
SDK. Later runs reuse image layers. `build-scripts/lib/container.sh` stages the
working tree under `.cords-build-cache/source` and mounts that copy read-only;
this avoids relabeling the checkout under SELinux. It persists Cargo registry,
Git, and tool caches under `.cords-build-cache/`, and compilation output under
`target/`. `CORDS_BUILD_CACHE_DIR`, `CARGO_TARGET_DIR`, `CORDS_QT_BUILD_DIR`,
`CORDS_DIST_DIR`, job controls, and Qt SDK overrides remain available. The
source copy reflects uncommitted edits. Host UID/GID mapping keeps cache and
artifacts writable by the invoking user. Temporary build containers are removed
on success and failure; build images and caches persist. Do not share a cache
folder between simultaneous builds.

Output goes to `dist/` by default: native Linux client/server `.tar.gz` files,
Windows x64 client `.zip`, and SHA-256 sidecars. The OCI builder loads the
`cords-server:<workspace version>` image locally; `CORDS_IMAGE_NAME` and
`CORDS_IMAGE_TAG` override its reference. Nothing is published by the scripts.

On SELinux systems, the runner labels only its staged source and generated
output/cache directories. If a mount fails, check that those directories are
owned by your user and reside on a filesystem supporting SELinux labeling; use
`CORDS_BUILD_CACHE_DIR` or output overrides to move them. For rootless Podman
check `podman info` and the standard subordinate UID/GID mappings. Docker uses
host UID/GID for bind-mounted outputs. CI runners with Docker should set
`CONTAINER_ENGINE=docker`. When the Docker CLI runs inside a Forgejo job
container, set `CORDS_CONTAINER_TRANSFER=copy`: the helper uses `docker cp` to
transfer source and artifacts and Docker named volumes for persistent caches,
so it does not rely on job-container paths being visible to the daemon.
`.forgejo/workflows/ci.yml` invokes the same Linux client script with this
mode. In copy mode, Cargo and Qt compilation caches live in Docker named
volumes rather than the host `CARGO_TARGET_DIR` and `CORDS_QT_BUILD_DIR`
folders. A Docker daemon/socket must be available to that runner. The
inactive release workflow retains its existing publication gates.

Linux Qt images currently bundle the x86_64 Qt SDK. ARM64 server tarballs and
images use the native Rust base image; ARM64 Qt builds need a matching SDK image.
Windows execution tests and final clean-machine desktop acceptance remain
separate from cross-compilation.

Unix scripts print script-specific requirements on failures. The requirement
lists in `build-scripts/requirements/` describe the tools inside each build
container. Windows and macOS continue using native build environments.

## Return the checkout to source-only state

Stop any active builds first. Preview or remove generated repository output:

```sh
bash build-scripts/linux-clean.sh --dry-run
bash build-scripts/linux-clean.sh
# On macOS:
bash build-scripts/mac-clean.sh --dry-run
bash build-scripts/mac-clean.sh
```

Both cleaners remove the repository's `target/`, `dist/`, `.cords-build-cache/`,
CI-created `.qt-sdk/` and `.qt-tools/`, and the ignored frontend/legacy output
folders listed in `.gitignore`. They refuse symlinked paths and directories
containing tracked files, and work from any current directory. They leave
untracked user data such as `.env` files and databases, custom output folders
outside the checkout, installed SDKs outside the checkout, and container-engine
images/volumes intact. The Linux builder will recreate its image and caches on
the next build.

## Windows ZIP

To clean Windows build and package output (the equivalent of `make clean`):

```powershell
& .\build-scripts\clean-windows.ps1 -WhatIf
& .\build-scripts\clean-windows.ps1
```

The cleaner removes repository `target/` and `dist/`, including Qt/CMake and
Cargo output. It also removes custom output folders selected by
`CORDS_QT_BUILD_DIR`, `CARGO_TARGET_DIR` and `CORDS_DIST_DIR`, or the explicit
`-BuildDirectory`, `-CargoTargetDirectory` and `-DistDirectory` parameters.
Relative overrides resolve from the current directory, matching the Windows
builder. Custom folders must be dedicated generated-output directories; their
entire contents are removed. Source-tree paths outside `target/` and `dist/`,
filesystem roots, repository ancestors and linked paths are rejected before
any deletion. Stop running builds and clients first. Repeated runs are safe;
missing directories are reported as already clean. Only PowerShell is required.
Deleted output can be recreated by rebuilding.

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

## Linux client archives

Extract the complete Linux tarball and run `bin/Cords`; preserve its deployed
Qt libraries, plugins, QML modules and migrations. Extract the Windows ZIP and
run `Cords.exe` on a Windows x64 machine with the Microsoft Visual C++
2015–2022 runtime. `CORDS_VC_REDIST` can include an existing installer. The
cross build compiles Windows test executables and uses Linux Qt tools for QML
lint; it does not require Wine or PowerShell. Run those tests on Windows.

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

Server tarballs contain the executable, PostgreSQL migrations, example config,
README and license. Replace example origins, credentials and paths before use.
The OCI image uses `deploy/Dockerfile` and the host engine; it retains its
existing labels, nonroot runtime user, healthcheck, entrypoint, and configuration.
No persistent data is packaged.
