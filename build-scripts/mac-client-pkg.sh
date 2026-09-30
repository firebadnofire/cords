#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-scripts/lib/common.sh
source "${script_dir}/lib/common.sh"

require_macos
for command_name in cargo grep npm pkgbuild pkgutil xcrun; do
  require_command "${command_name}"
done

version="$(project_version)"
architecture="$(host_architecture)"
prepare_dist
staging_root="$(mktemp -d)"
trap 'rm -rf -- "${staging_root}"' EXIT

log "Building Cords macOS client ${version} for ${architecture}"
(
  cd "${REPO_ROOT}/bins/cords-client/ui" || \
    die "client UI directory is unavailable"
  npm ci
  npm run tauri:build -- --bundles app
)
application="${REPO_ROOT}/target/release/bundle/macos/Cords.app"
[[ -d "${application}" ]] || die "macOS application bundle was not produced: ${application}"

mkdir -p "${staging_root}/Applications"
cp -R "${application}" "${staging_root}/Applications/Cords.app"
artifact="${DIST_DIR}/cords-client-macos-${architecture}-${version}.pkg"
rm -f -- "${artifact}" "${artifact}.sha256"
pkgbuild_arguments=(
  --root "${staging_root}"
  --identifier chat.cords.desktop
  --version "${version}"
  --install-location /
)
if [[ -n "${CORDS_PKG_SIGN_IDENTITY:-}" ]]; then
  pkgbuild_arguments+=(--sign "${CORDS_PKG_SIGN_IDENTITY}")
  log "Creating signed macOS package with the configured installer identity"
else
  log "Creating unsigned macOS package"
fi
pkgbuild "${pkgbuild_arguments[@]}" "${artifact}"
[[ -s "${artifact}" ]] || die "macOS package was not produced: ${artifact}"
pkgutil --payload-files "${artifact}" | \
  grep -Eq '(^|/)Applications/Cords\.app/Contents/Info\.plist$' || \
  die "macOS package does not contain the expected Cords.app payload"
write_sha256 "${artifact}"
log "Created ${artifact}"

