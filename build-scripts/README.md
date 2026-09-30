# Cords build scripts

These scripts build release artifacts from any working directory and place
file-based output in the repository-level `dist/` directory by default. Set
`CORDS_DIST_DIR` to use another output directory. Existing artifacts with the
same exact name are replaced; unrelated files are preserved.

Artifact names use the authoritative workspace version from `Cargo.toml` and
the detected or selected CPU architecture. Every file artifact receives a
neighboring `.sha256` file containing a basename-only SHA-256 record.

The scripts do not install host dependencies, publish artifacts, push images,
sign releases with repository secrets, or perform notarization.

## Scripts

| Script | Host | Output |
|---|---|---|
| `windows-client.ps1` | Native Windows x64 or ARM64 | `cords-client-windows-<arch>-<version>.tar.gz` |
| `linux-windows-client-crosscomp.sh` | Linux x64 or ARM64 | The same portable Windows archive as the native script |
| `linux-client-tar.sh` | Linux x64 or ARM64 | `cords-client-linux-<arch>-<version>.tar.gz`, containing `Cords.AppImage` |
| `linux-server-tar.sh` | Linux x64 or ARM64 | `cords-server-linux-<arch>-<version>.tar.gz` |
| `mac-client-pkg.sh` | macOS Intel or Apple Silicon | `cords-client-macos-<arch>-<version>.pkg` |
| `mac-server-tar.sh` | macOS Intel or Apple Silicon | `cords-server-macos-<arch>-<version>.tar.gz` |
| `linux-server-oci-img.sh` | Linux with Docker or Podman | A locally loaded `cords-server:<version>` image |

There is intentionally no Windows server build script.

## Common dependencies

- Rust 1.97.1 and Cargo, as selected by `rust-toolchain.toml`
- The Rust targets used by the selected build
- Node.js 24 and npm for client builds
- `tar` and either `sha256sum` or `shasum` for Unix file artifacts

Scripts fail with an actionable message when a required command is absent.
They never invoke a system package manager.

## Windows client

Native Windows development validation requires PowerShell 7 or Windows
PowerShell with .NET APIs used by the script, Rust's MSVC host toolchain,
Node.js/npm, and the Windows `tar.exe` utility:

```powershell
& C:\path\to\cords\build-scripts\windows-client.ps1
```

The result is a portable archive containing `Cords.exe`, `README.md`, and the
license. Cords uses the system WebView2 runtime; it does not copy a WebView2
runtime into the archive. This script builds only the client.

## Linux-built Windows client

The CI-preferred Windows path requires Bash, `cargo-xwin` 0.23.1, Clang,
`lld-link`, Rust's matching Windows MSVC target, Node.js/npm, and archive tools.
When a Linux distribution exposes `clang` but not the `clang-cl` driver name,
the script creates a private temporary `clang-cl` symlink and removes it after
the build; it does not modify the host toolchain.
The Linux host architecture must match the Windows target architecture. This
reflects the current `ring`/MSVC cross-toolchain behavior and prevents a
non-working cross-CPU build from being advertised.

```sh
bash build-scripts/linux-windows-client-crosscomp.sh --target x86_64
bash build-scripts/linux-windows-client-crosscomp.sh --target aarch64
```

Use `ubuntu-22.04` for x86_64 and `arm-ubuntu-22.04` for ARM64. The script uses
genuine MSVC cross-compilation and never requires Wine.

## Linux artifacts

The client script uses the repository's Tauri AppImage build. In addition to
the common tools it requires `file`, `pkg-config`, `wget`, and development
packages exposing `webkit2gtk-4.1`, `ayatana-appindicator3-0.1`, and
`librsvg-2.0` through `pkg-config`.

```sh
bash build-scripts/linux-client-tar.sh
bash build-scripts/linux-server-tar.sh
```

The server archive contains `cords-server`, PostgreSQL migrations,
`server.toml.example`, `README.md`, and `LICENSE`. The configuration is a
template only; operators must replace its example origin, database password,
and paths as appropriate. No credentials or persistent data are packaged.

## macOS artifacts

Both macOS scripts build only the native host architecture. They do not claim
to produce universal binaries.

```sh
bash build-scripts/mac-client-pkg.sh
bash build-scripts/mac-server-tar.sh
```

The client requires the normal Tauri macOS prerequisites plus Apple's
`pkgbuild`, `pkgutil`, and Xcode command-line tools. It stages the real
`Cords.app` at `/Applications/Cords.app`, verifies that package payload, and
creates an unsigned `.pkg` by default. Set `CORDS_PKG_SIGN_IDENTITY` to an
already configured installer-signing identity to sign the package creation
step. Code signing the app, signing the installer, and Apple notarization are
separate concerns; the script does not silently perform the first or last.

## Local server image

The OCI script reuses `deploy/Dockerfile`, which is a multi-stage build with a
Debian slim runtime, non-root UID/GID 4848, port 4848, the server healthcheck,
and runtime configuration through the existing config file, command-line, and
`CORDS_*` environment mechanisms.

```sh
bash build-scripts/linux-server-oci-img.sh
docker image inspect cords-server:0.1.0
```

Docker is preferred when present; otherwise Podman is used. Override the local
reference with `CORDS_IMAGE_NAME` and `CORDS_IMAGE_TAG`. The script builds and
loads the image locally and never pushes it. It does not emit a file checksum
because no image archive is exported.

