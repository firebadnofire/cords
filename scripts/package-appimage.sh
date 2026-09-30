#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ui_dir="${repo_dir}/bins/cords-client/ui"

cd "${ui_dir}"
npm ci
npm run tauri:build -- --bundles appimage

artifact="$(find "${repo_dir}/target/release/bundle/appimage" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
test -n "${artifact}" || { echo "Tauri did not produce an AppImage" >&2; exit 1; }
install -Dm755 "${artifact}" "${repo_dir}/target/Cords.AppImage"
echo "Created target/Cords.AppImage"
