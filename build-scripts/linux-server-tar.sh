#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-scripts/lib/common.sh
source "${script_dir}/lib/common.sh"

require_linux
for command_name in cargo find tar install; do
  require_command "${command_name}"
done

version="$(project_version)"
architecture="$(host_architecture)"
prepare_dist
staging_root="$(mktemp -d)"
trap 'rm -rf -- "${staging_root}"' EXIT
package_name="cords-server-linux-${architecture}"
package_root="${staging_root}/${package_name}"
mkdir -p "${package_root}"

log "Building Cords Linux server ${version} for ${architecture}"
cargo_target="${CARGO_TARGET_DIR:-${REPO_ROOT}/target}"
cargo build --locked --release --target-dir "${cargo_target}" --manifest-path "${REPO_ROOT}/Cargo.toml" --bin cords-server
stage_server_distribution "${cargo_target}/release/cords-server" "${package_root}"

artifact="${DIST_DIR}/cords-server-linux-${architecture}-${version}.tar.gz"
log "Packaging ${artifact}"
create_tar_gz "${staging_root}" "${package_name}" "${artifact}"
log "Created ${artifact}"

