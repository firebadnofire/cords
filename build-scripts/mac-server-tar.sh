#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-scripts/lib/common.sh
source "${script_dir}/lib/common.sh"

require_macos
for command_name in cargo find tar install xcrun; do
  require_command "${command_name}"
done

version="$(project_version)"
architecture="$(host_architecture)"
prepare_dist
staging_root="$(mktemp -d)"
trap 'rm -rf -- "${staging_root}"' EXIT
package_name="cords-server-macos-${architecture}"
package_root="${staging_root}/${package_name}"
mkdir -p "${package_root}"

log "Building Cords macOS server ${version} for ${architecture}"
cargo build --locked --release --manifest-path "${REPO_ROOT}/Cargo.toml" --bin cords-server
stage_server_distribution "${REPO_ROOT}/target/release/cords-server" "${package_root}"

artifact="${DIST_DIR}/cords-server-macos-${architecture}-${version}.tar.gz"
log "Packaging ${artifact}"
create_tar_gz "${staging_root}" "${package_name}" "${artifact}"
log "Created ${artifact}"

