#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/lib/common.sh"
source "${script_dir}/lib/qt-client.sh"
require_macos
for command_name in codesign cmp grep pkgbuild pkgutil; do require_command "${command_name}"; done
qt_client_requirements
version="$(project_version)"
architecture="$(host_architecture)"
prepare_dist
staging_root="$(mktemp -d)"
trap 'rm -rf -- "${staging_root}"' EXIT
build_qt_client "macos-${architecture}" "${staging_root}/native" --tests
application="${staging_root}/native/Cords.app"
sign_qt_app "${application}"
mkdir -p "${staging_root}/payload/Applications"
cp -R "${application}" "${staging_root}/payload/Applications/Cords.app"
artifact="${DIST_DIR}/cords-client-macos-${architecture}-${version}.pkg"
rm -f -- "${artifact}" "${artifact}.sha256"
pkgbuild_arguments=(--root "${staging_root}/payload" --identifier chat.cords.desktop --version "${version}" --install-location /)
if [[ -n "${CORDS_PKG_SIGN_IDENTITY:-}" ]]; then
  pkgbuild_arguments+=(--sign "${CORDS_PKG_SIGN_IDENTITY}")
  log 'Creating package with the configured installer signing identity'
else
  log 'Creating unsigned installer package with an explicitly sealed app bundle'
fi
pkgbuild "${pkgbuild_arguments[@]}" "${artifact}"
pkgutil --payload-files "${artifact}" | grep -Eq '(^|/)Applications/Cords\.app/Contents/Info\.plist$' || die 'Installer is missing Cords.app'
pkgutil --expand-full "${artifact}" "${staging_root}/verified"
verified="${staging_root}/verified/Payload/Applications/Cords.app"
codesign --verify --deep --strict "${verified}"
cmp "${application}/Contents/MacOS/Cords" "${verified}/Contents/MacOS/Cords"
write_sha256 "${artifact}"
log "Created native Qt installer ${artifact}"
