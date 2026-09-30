#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-scripts/lib/common.sh
source "${script_dir}/lib/common.sh"

require_linux
for command_name in cargo file npm pkg-config tar install find wget; do
  require_command "${command_name}"
done
for module in webkit2gtk-4.1 ayatana-appindicator3-0.1 librsvg-2.0; do
  pkg-config --exists "${module}" || \
    die "required Linux Tauri development package is unavailable: ${module}"
done

version="$(project_version)"
architecture="$(host_architecture)"
prepare_dist
SOURCE_DATE_EPOCH="$(source_date_epoch)"
export SOURCE_DATE_EPOCH

log "Building Cords Linux client ${version} for ${architecture}"
bash "${REPO_ROOT}/scripts/package-appimage.sh"
appimage="${REPO_ROOT}/target/Cords.AppImage"
[[ -s "${appimage}" ]] || die "AppImage build did not produce ${appimage}"

log "Packaging Linux client archive"
bash "${REPO_ROOT}/scripts/package-client-tarball.sh" \
  linux "${architecture}" "${version}" "${appimage}" "${DIST_DIR}"
artifact="${DIST_DIR}/cords-client-linux-${architecture}-${version}.tar.gz"
[[ -s "${artifact}" ]] || die "Linux client archive was not produced: ${artifact}"
write_sha256 "${artifact}"
log "Created ${artifact}"

