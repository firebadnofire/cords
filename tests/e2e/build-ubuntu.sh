#!/usr/bin/env bash
# Runs INSIDE the owned Ubuntu guest, never on the Windows workstation.
set -Eeuo pipefail
cd "${CORDS_SOURCE_DIR:-/home/testuser/source}"
[[ $(. /etc/os-release; printf '%s' "$ID") == ubuntu ]]
export PATH="$HOME/.cargo/bin:$HOME/tools/llvm:$PATH"
export CARGO_BUILD_JOBS=2 CORDS_BUILD_JOBS=2
export CARGO_TARGET_DIR="$HOME/build/cargo-windows"
export CORDS_QT_BUILD_DIR="$HOME/build/qt-windows-$(basename "$PWD")"
export CORDS_DIST_DIR="$HOME/artifacts/windows"
export CORDS_QT_HOST_PREFIX="$HOME/Qt/gcc_64"
export CORDS_QT_WINDOWS_PREFIX="$HOME/Qt/msvc2022_64"
mkdir -p "$HOME/tools/llvm" "$HOME/artifacts"
export PATH="/usr/lib/llvm-19/bin:$PATH"
for tool in clang-cl lld-link llvm-lib llvm-rc llvm-mt; do
    if [[ "$tool" == clang-cl && -x /usr/lib/llvm-19/bin/clang ]]; then
        ln -sf /usr/lib/llvm-19/bin/clang "$HOME/tools/llvm/clang-cl"
    fi
    if ! command -v "$tool" >/dev/null; then
        candidate=$(find -L /usr/lib/llvm-*/bin -maxdepth 1 -name "$tool" -type f | sort -V | tail -n 1)
        # Ubuntu packages omit the clang-cl alias; Clang selects its MSVC driver by argv[0].
        if [[ -z "$candidate" && "$tool" == clang-cl ]]; then candidate=$(command -v clang); fi
        [[ -n "$candidate" ]] || { printf 'Missing LLVM tool: %s\n' "$tool" >&2; exit 2; }
        ln -sf "$candidate" "$HOME/tools/llvm/$tool"
    fi
done
if [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
    curl --proto '=https' --tlsv1.3 -fsSL https://sh.rustup.rs -o "$HOME/tools/rustup.sh"
    sh "$HOME/tools/rustup.sh" -y --profile minimal --default-toolchain 1.97.1
fi
rustup target add --toolchain 1.97.1 x86_64-pc-windows-msvc
if ! command -v cargo-xwin >/dev/null; then
    cargo install --locked cargo-xwin --version 0.23.1
fi
rustc --version
clang-cl --version
"$CORDS_QT_HOST_PREFIX/bin/qmake" -query QT_VERSION
bash build-scripts/linux-windows-client-crosscomp.sh
(cd "$CORDS_DIST_DIR" && sha256sum -c cords-client-windows-x86_64-0.1.0.zip.sha256)
