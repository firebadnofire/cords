# Native Cords desktop

Qt 6 Quick/QML and a narrow typed CXX bridge to the existing Rust core.
Migration acceptance is recorded in `docs/plans/qt-desktop-migration.md`.
This target is under active migration; the old frontend has not passed its
removal gate. Do not interpret a native build as complete feature parity.

## Dependencies and build

Use the repository Rust toolchain (1.97.1), Python 3, CMake >=3.24, and **Qt
6.10.3** with Core, Gui, Qml, Quick, QuickControls2, Widgets, Concurrent, Test
and QuickTest. The Qt SDK must match the native C++ toolchain and architecture.
Windows requires MSVC 2022 x64. Linux requires a native compiler, Qt's X11/Wayland
runtime dependencies and D-Bus for platform integration. macOS requires Xcode's
command-line tools and a matching macOS Qt SDK. Android is not implemented.

The build helper performs no SDK installation, service registration or account
operation. It checks the selected Qt version, uses Cargo.lock, and installs into
a dedicated output directory. Rerunning it updates that same output safely.
Choose a drive with sufficient free space for Rust/Qt build artifacts.

```sh
python scripts/build-client-qt.py --qt-prefix /absolute/Qt/6.10.3/native-sdk \
  --build-dir /absolute/build/cords-qt --package-dir /absolute/packages/cords-qt --tests
```

On Windows, provide `--cmake` if Visual Studio's bundled CMake is not on PATH.
`--profile dev` provides a faster development build; codec initialization is
substantially slower in unoptimized Rust. Packages default to optimized Rust.
Development executables require `CORDS_CLIENT_MIGRATIONS` or an installed tree;
source-tree migration fallback is explicitly disabled in package builds.

The installed Windows/Linux executable is under `bin`; macOS produces
`Cords.app`. Qt's deployment helper collects runtime libraries, platform plugins
and imported QML modules. Inspect the package on a clean target machine: SDK
availability during a build is not evidence of complete runtime deployment.

## Local state and startup

The existing `chat.cords.desktop` application data root is retained. Set
`CORDS_CLIENT_STATE`, `CORDS_CLIENT_MIGRATIONS`, or `CORDS_CLIENT_CA` only when an
explicit alternate root, migration directory, or trusted additional CA is needed.
TLS certificate checks and persistent server signing-key pins remain mandatory.
Native window settings live under the selected state root, outside account keys.

`--list-accounts` prints picker-safe IDs/labels. `--account ID --password-stdin`
accepts one bounded UTF-8 line from redirected stdin. Never put real passwords
in argv, environment variables, scripts or files. Account secrets are not returned
to QML. `--capture`, `--window-size` and `--quit-after` support local native
layout acceptance. Capture only disposable fixture data.

## Validation

`--tests` runs QML lint, Qt Quick control tests and real native JXL rendering tests.
The Rust bridge's vault tests run without Qt:

```sh
cargo test -p cords-qt-bridge --lib --test accounts
cargo clippy -p cords-qt-bridge --all-targets -- -D warnings
```

`rust/tests/live.rs` is an ignored acceptance test against the existing disposable
TLS/PostgreSQL desktop fixture, with two independent encrypted account vaults.
It is deliberately not a frontend mock. Native media tests cover timing,
bounded decoder-slot fairness, hidden/offscreen cleanup and pause/resume.
Linux/macOS compilation, platform services and runtime acceptance still need
their native hosts; Windows checks do not establish those results.

## Distribution

Qt is dynamically linked. Distribution must include applicable Qt and third-party
license notices and satisfy the chosen LGPLv3/GPLv3 or commercial license terms,
including source/replacement obligations where applicable. Inventory the modules
and deployed plugins, not just directly linked libraries. Preserve the application
AGPL license and bundled codec notices. Signing, notarization, installers, source
offers and clean-machine platform acceptance remain release gates.
