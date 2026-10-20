#!/usr/bin/env bash
set -Eeuo pipefail
cd "${CORDS_SOURCE_DIR:-/home/testuser/source}"
[[ $(. /etc/os-release; printf '%s' "$ID") == ubuntu ]]
export PATH="$HOME/.cargo/bin:$HOME/Qt/gcc_64/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.97.1 CARGO_BUILD_JOBS=2 CORDS_BUILD_JOBS=2
export CARGO_TARGET_DIR="$HOME/build/cargo-linux"
export CORDS_QT_BUILD_DIR="$HOME/build/qt-linux-$(basename "$PWD")"
exec 9>"$HOME/build/native-gui.lock"
flock -n 9 || { echo 'Another native GUI builder owns this guest cache' >&2; exit 2; }
export CORDS_DIST_DIR="$HOME/artifacts/linux"
export CMAKE_PREFIX_PATH="$HOME/Qt/gcc_64"
export CORDS_QT_PREFIX="$HOME/Qt/gcc_64"
export TMPDIR="$HOME/build/tmp"
mkdir -p "$TMPDIR"
bash build-scripts/linux-client-tar.sh
(cd "$CORDS_DIST_DIR" && sha256sum -c cords-client-linux-x86_64-0.1.0.tar.gz.sha256)
bash "$HOME/tools/run-linux-ui-tests.sh" "$CORDS_QT_BUILD_DIR/linux-x86_64" "$PWD"
