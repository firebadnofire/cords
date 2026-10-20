#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/lib/common.sh"
source "${script_dir}/lib/qt-client.sh"
require_macos
for command_name in codesign file find grep hdiutil lipo ln rustup; do require_command "${command_name}"; done
qt_client_requirements
for rust_target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target list --installed | grep -Fxq "${rust_target}" || die "Rust target is not installed: ${rust_target}"
done
verify_universal() {
  local binary="$1" architecture
  for architecture in arm64 x86_64; do
    lipo "${binary}" -verify_arch "${architecture}" || die "Missing ${architecture} architecture: ${binary}"
  done
}
verify_universal "${CORDS_QT_PREFIX}/lib/QtCore.framework/Versions/A/QtCore"
version="$(project_version)"
prepare_dist
staging_root="$(mktemp -d)"
mounted=false
cleanup() {
  local status=$?
  if [[ "${mounted}" == true ]]; then hdiutil detach "${staging_root}/mounted" || status=1; fi
  rm -rf -- "${staging_root}"
  exit "${status}"
}
trap cleanup EXIT
# Keep both arrays nonempty: Apple's Bash 3.2 rejects empty arrays under set -u.
arm_arguments=(--rust-target aarch64-apple-darwin --mac-architecture arm64)
intel_arguments=(--rust-target x86_64-apple-darwin --mac-architecture x86_64)
if [[ "$(host_architecture)" == aarch64 ]]; then arm_arguments+=(--tests); else intel_arguments+=(--tests); fi
build_qt_client macos-arm64 "${staging_root}/arm" "${arm_arguments[@]}"
build_qt_client macos-x86_64 "${staging_root}/intel" "${intel_arguments[@]}"
application="${staging_root}/arm/Cords.app"
lipo -create "${application}/Contents/MacOS/Cords" "${staging_root}/intel/Cords.app/Contents/MacOS/Cords" -output "${staging_root}/Cords-universal"
mv "${staging_root}/Cords-universal" "${application}/Contents/MacOS/Cords"
# Verify every deployed Mach-O dependency, not just the application executable.
while IFS= read -r -d '' binary; do
  if file -b "${binary}" | grep -q 'Mach-O'; then
    verify_universal "${binary}"
  fi
done < <(find "${application}" -type f -print0)
sign_qt_app "${application}"
mkdir "${staging_root}/image" "${staging_root}/mounted"
mv "${application}" "${staging_root}/image/Cords.app"
ln -s /Applications "${staging_root}/image/Applications"
artifact="${DIST_DIR}/cords-client-macos-universal-${version}.dmg"
rm -f -- "${artifact}" "${artifact}.sha256"
hdiutil create -volname Cords -srcfolder "${staging_root}/image" -format UDZO "${artifact}"
hdiutil verify "${artifact}"
hdiutil attach -readonly -nobrowse -mountpoint "${staging_root}/mounted" "${artifact}"
mounted=true
codesign --verify --deep --strict "${staging_root}/mounted/Cords.app"
verify_universal "${staging_root}/mounted/Cords.app/Contents/MacOS/Cords"
write_sha256 "${artifact}"
log "Created native Qt universal DMG ${artifact}"
