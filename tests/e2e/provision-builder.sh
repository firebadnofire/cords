#!/usr/bin/env bash
# Runs only inside an owned Ubuntu 24.04 guest. No distribution/package upgrades.
set -Eeuo pipefail
source /etc/os-release
[[ "$ID" == ubuntu && "$VERSION_ID" == 24.04 ]] || { echo 'Ubuntu 24.04 build guest required' >&2; exit 2; }
mkdir -p "$HOME/tools" "$HOME/build" "$HOME/artifacts"
receipt="$HOME/tools/builder-provision.sha256"
digest=$(sha256sum "$0" | cut -d' ' -f1)
if [[ ! -f "$receipt" || $(cat "$receipt") != "$digest" ]]; then
    sudo apt-get update
    sudo DEBIAN_FRONTEND=noninteractive apt-get install --no-upgrade --no-install-recommends -y \
        build-essential cmake ninja-build pkg-config libssl-dev clang-19 lld-19 llvm-19 \
        curl git python3-venv xvfb xauth openbox x11-utils libgl1-mesa-dev libxcb-cursor0 libxkbcommon-x11-0 \
        libxcb-icccm4 libxcb-keysyms1 libxcb-shape0 \
        libegl1-mesa-dev libvulkan1
    if [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
        curl --proto '=https' --tlsv1.3 -fsSL https://sh.rustup.rs -o "$HOME/tools/rustup.sh"
        sh "$HOME/tools/rustup.sh" -y --profile minimal --default-toolchain 1.97.1
    fi
    "$HOME/.cargo/bin/rustup" toolchain install 1.97.1 --profile minimal
    printf '%s\n' "$digest" > "$receipt"
fi
dpkg-query -W > "$HOME/artifacts/ubuntu-packages.txt"
"$HOME/.cargo/bin/rustc" +1.97.1 --version
/usr/lib/llvm-19/bin/clang --version
