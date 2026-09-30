#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <platform> <architecture> <release-id> <client-path> <output-directory>" >&2
  exit 2
}

[[ $# -eq 5 ]] || usage

platform="$1"
architecture="$2"
release_id="$3"
client_path="$4"
output_directory="$5"

case "${platform}" in
  linux | macos | windows) ;;
  *) echo "error: unsupported client platform: ${platform}" >&2; exit 2 ;;
esac

case "${architecture}" in
  x86_64 | aarch64) ;;
  *) echo "error: unsupported client architecture: ${architecture}" >&2; exit 2 ;;
esac

[[ "${release_id}" =~ ^[A-Za-z0-9._-]+$ ]] || {
  echo "error: release ID contains unsafe characters: ${release_id}" >&2
  exit 2
}

[[ -e "${client_path}" ]] || {
  echo "error: client artifact does not exist: ${client_path}" >&2
  exit 1
}

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ -f "${repo_dir}/README.md" && -f "${repo_dir}/LICENSE" ]] || {
  echo "error: README.md and LICENSE must exist at the repository root" >&2
  exit 1
}

mkdir -p "${output_directory}"
output_directory="$(cd "${output_directory}" && pwd)"
archive_name="cords-client-${platform}-${architecture}-${release_id}.tar.gz"
archive_path="${output_directory}/${archive_name}"
staging_root="$(mktemp -d)"
trap 'rm -rf -- "${staging_root}"' EXIT
package_root="${staging_root}/cords-client-${platform}-${architecture}"
mkdir -p "${package_root}"

case "${platform}" in
  linux)
    install -m 0755 "${client_path}" "${package_root}/Cords.AppImage"
    ;;
  windows)
    install -m 0755 "${client_path}" "${package_root}/Cords.exe"
    ;;
  macos)
    [[ -d "${client_path}" ]] || {
      echo "error: macOS client artifact must be an .app directory" >&2
      exit 1
    }
    cp -R "${client_path}" "${package_root}/Cords.app"
    ;;
esac

install -m 0644 "${repo_dir}/README.md" "${package_root}/README.md"
install -m 0644 "${repo_dir}/LICENSE" "${package_root}/LICENSE"

rm -f -- "${archive_path}"
if tar --version 2>/dev/null | grep -q 'GNU tar'; then
  source_date_epoch="${SOURCE_DATE_EPOCH:-0}"
  [[ "${source_date_epoch}" =~ ^[0-9]+$ ]] || {
    echo "error: SOURCE_DATE_EPOCH must be an integer" >&2
    exit 2
  }
  tar --sort=name \
    --mtime="@${source_date_epoch}" \
    --owner=0 --group=0 --numeric-owner \
    -C "${staging_root}" \
    -czf "${archive_path}" \
    "$(basename "${package_root}")"
else
  tar -C "${staging_root}" -czf "${archive_path}" "$(basename "${package_root}")"
fi

test -s "${archive_path}" || {
  echo "error: client archive was not created: ${archive_path}" >&2
  exit 1
}

echo "Created ${archive_path}"
