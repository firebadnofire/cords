#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-scripts/lib/common.sh
source "${script_dir}/lib/common.sh"

require_macos
for command_name in cargo codesign find grep hdiutil lipo npm rustup; do
  require_command "${command_name}"
done

for rust_target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target list --installed | grep -Fxq "${rust_target}" || \
    die "Rust target is not installed: ${rust_target}"
done

version="$(project_version)"
prepare_dist

log "Building Cords universal macOS client ${version}"
(
  cd "${REPO_ROOT}/bins/cords-client/ui" || \
    die "client UI directory is unavailable"
  npm ci
  npm run tauri:build -- --target universal-apple-darwin --bundles dmg
)

dmg_directory="${REPO_ROOT}/target/universal-apple-darwin/release/bundle/dmg"
[[ -d "${dmg_directory}" ]] || \
  die "macOS DMG directory was not produced: ${dmg_directory}"
dmg_count="$(find "${dmg_directory}" -maxdepth 1 -type f -name '*.dmg' | wc -l | tr -d '[:space:]')"
[[ "${dmg_count}" == 1 ]] || \
  die "expected exactly one universal macOS DMG in ${dmg_directory}, found ${dmg_count}"
source_dmg="$(find "${dmg_directory}" -maxdepth 1 -type f -name '*.dmg' -print -quit)"
# Tauri removes the intermediate .app when only the DMG bundle is requested.
# Verify the actual distributed app inside the completed, read-only mounted DMG.

artifact="${DIST_DIR}/cords-client-macos-universal-${version}.dmg"
rm -f -- "${artifact}" "${artifact}.sha256"
cp "${source_dmg}" "${artifact}"
[[ -s "${artifact}" ]] || die "macOS DMG was not produced: ${artifact}"
hdiutil verify "${artifact}"
mount_root="$(mktemp -d)"
mounted=false
cleanup() {
  local status=$?
  if [[ "${mounted}" == true ]]; then
    hdiutil detach "${mount_root}" || status=1
  fi
  rmdir "${mount_root}" || status=1
  exit "${status}"
}
trap cleanup EXIT
hdiutil attach -readonly -nobrowse -mountpoint "${mount_root}" "${artifact}"
mounted=true
codesign --verify --deep --strict "${mount_root}/Cords.app"
lipo "${mount_root}/Cords.app/Contents/MacOS/cords-client" -verify_arch arm64 x86_64
write_sha256 "${artifact}"
log "Created ${artifact}"
