#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/lib/common.sh"
require_linux
die 'The desktop now uses native Qt 6. The former cargo-xwin/Tauri command cannot build or deploy this UI. Use build-scripts/windows-client.ps1 on a native Windows MSVC host with Qt 6.10.3. A validated Qt/MSVC cross-toolchain is not yet available; no old-frontend artifact was produced.'
