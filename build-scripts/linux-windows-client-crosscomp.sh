#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-scripts/lib/common.sh
source "${script_dir}/lib/common.sh"

usage() {
  printf 'usage: %s [--target x86_64|aarch64]\n' "$0" >&2
  exit 2
}

target_architecture=""
while (( $# > 0 )); do
  case "$1" in
    --target)
      (( $# == 2 )) || usage
      target_architecture="$(normalize_architecture "$2")"
      shift 2
      ;;
    *) usage ;;
  esac
done

require_linux
target_architecture="${target_architecture:-$(host_architecture)}"
host_arch="$(host_architecture)"
[[ "${target_architecture}" == "${host_arch}" ]] || \
  die "Windows ${target_architecture} builds require a matching ${target_architecture} Linux runner; the current host is ${host_arch}"
for command_name in cargo cargo-xwin clang lld-link ln npm rustup tar install; do
  require_command "${command_name}"
done

tool_shim_directory=""
cleanup() {
  if [[ -n "${tool_shim_directory}" ]]; then
    rm -rf -- "${tool_shim_directory}"
  fi
}
trap cleanup EXIT
if ! command -v clang-cl >/dev/null 2>&1; then
  tool_shim_directory="$(mktemp -d)"
  ln -s "$(command -v clang)" "${tool_shim_directory}/clang-cl"
  export PATH="${tool_shim_directory}:${PATH}"
  log "Using a private clang-cl driver shim for $(command -v clang)"
fi
require_command clang-cl "install Clang with clang-cl driver support"

case "${target_architecture}" in
  x86_64) rust_target=x86_64-pc-windows-msvc ;;
  aarch64) rust_target=aarch64-pc-windows-msvc ;;
  *) die "unsupported Windows target architecture: ${target_architecture}" ;;
esac

rustup target list --installed | grep -Fxq "${rust_target}" || \
  die "Rust target ${rust_target} is not installed; run: rustup target add ${rust_target}"

version="$(project_version)"
prepare_dist
SOURCE_DATE_EPOCH="$(source_date_epoch)"
export SOURCE_DATE_EPOCH
build_frontend

log "Cross-compiling Cords Windows client ${version} for ${target_architecture}"
cargo xwin build --locked --release --manifest-path "${REPO_ROOT}/Cargo.toml" \
  --target "${rust_target}" -p cords-client
client="${REPO_ROOT}/target/${rust_target}/release/cords-client.exe"
[[ -s "${client}" ]] || die "Windows client was not produced: ${client}"

log "Packaging portable Windows client archive"
bash "${REPO_ROOT}/scripts/package-client-tarball.sh" \
  windows "${target_architecture}" "${version}" "${client}" "${DIST_DIR}"
artifact="${DIST_DIR}/cords-client-windows-${target_architecture}-${version}.tar.gz"
[[ -s "${artifact}" ]] || die "Windows client archive was not produced: ${artifact}"
write_sha256 "${artifact}"
log "Created ${artifact}"

