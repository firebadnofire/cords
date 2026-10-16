#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/lib/common.sh"
source "${script_dir}/lib/qt-client.sh"
require_linux
for command_name in tar install; do require_command "${command_name}"; done
qt_client_requirements
version="$(project_version)"
architecture="$(host_architecture)"
prepare_dist
staging_root="$(mktemp -d)"
trap 'rm -rf -- "${staging_root}"' EXIT
package_name="cords-client-linux-${architecture}"
package="${staging_root}/${package_name}"
build_qt_client "linux-${architecture}" "${package}" --tests
[[ -x "${package}/bin/Cords" ]] || die 'Qt deployment did not produce bin/Cords'
for plugin in qxcb qwayland qoffscreen; do
  [[ -s "${package}/plugins/platforms/lib${plugin}.so" ]] || die "Qt deployment is missing ${plugin}; install the matching Qt platform SDK archive"
done
install -m 0644 "${REPO_ROOT}/README.md" "${package}/README.md"
log 'Packaging native Qt executable, runtime, QML modules and sealed-vault migrations'
artifact="${DIST_DIR}/cords-client-linux-${architecture}-${version}.tar.gz"
create_tar_gz "${staging_root}" "${package_name}" "${artifact}"
log "Created ${artifact}; extract the complete directory and run bin/Cords"
