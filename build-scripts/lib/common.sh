#!/usr/bin/env bash

# Shared helpers for the Cords build scripts. Callers must enable
# `set -euo pipefail` before sourcing this file.

BUILD_SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "${BUILD_SCRIPTS_DIR}/.." && pwd)"
DIST_DIR="${CORDS_DIST_DIR:-${REPO_ROOT}/dist}"
BUILD_SCRIPT_NAME="${BASH_SOURCE[1]##*/}"
BUILD_REQUIREMENTS_PRINTED=false

print_build_requirements() {
  [[ "${BUILD_REQUIREMENTS_PRINTED}" == false ]] || return 0
  BUILD_REQUIREMENTS_PRINTED=true
  local requirements="${BUILD_SCRIPTS_DIR}/requirements/${BUILD_SCRIPT_NAME}.txt"
  printf '\nRequirements for %s:\n' "${BUILD_SCRIPT_NAME}" >&2
  if [[ -f "${requirements}" ]]; then
    printf '%s\n' "$(< "${requirements}")" >&2
  else
    printf 'Requirements file is missing: %s\n' "${requirements}" >&2
  fi
  printf 'Full setup and controls: %s/README.md\n' "${BUILD_SCRIPTS_DIR}" >&2
}

# Also cover dependency errors reported by Cargo, CMake and the Python helpers.
# ERR is separate from the entry points' existing EXIT cleanup traps.
set -E
trap 'print_build_requirements' ERR

log() {
  printf '==> %s\n' "$*"
}

die() {
  printf 'error: %s\n' "$*" >&2
  print_build_requirements
  exit 1
}

require_command() {
  local command_name="$1"
  local guidance="${2:-install it and ensure it is available on PATH}"
  command -v "${command_name}" >/dev/null 2>&1 || \
    die "required command '${command_name}' is unavailable; ${guidance}"
}

require_linux() {
  [[ "$(uname -s)" == Linux ]] || die "this script requires a Linux host"
}

require_macos() {
  [[ "$(uname -s)" == Darwin ]] || die "this script requires a macOS host"
}

project_version() {
  local version
  version="$({
    awk '
      { sub(/\r$/, "") }
      /^\[workspace\.package\]$/ { in_package = 1; next }
      /^\[/ { in_package = 0 }
      in_package && /^version[[:space:]]*=/ {
        value = $0
        sub(/^[^=]*=[[:space:]]*"/, "", value)
        sub(/"[[:space:]]*$/, "", value)
        print value
        exit
      }
    ' "${REPO_ROOT}/Cargo.toml"
  } || true)"
  [[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]] || \
    die "could not read a safe workspace version from Cargo.toml"
  printf '%s\n' "${version}"
}

normalize_architecture() {
  case "$1" in
    x86_64 | amd64) printf '%s\n' x86_64 ;;
    aarch64 | arm64) printf '%s\n' aarch64 ;;
    *) die "unsupported CPU architecture: $1" ;;
  esac
}

host_architecture() {
  normalize_architecture "$(uname -m)"
}

source_date_epoch() {
  local epoch="${SOURCE_DATE_EPOCH:-}"
  if [[ -z "${epoch}" ]] && command -v git >/dev/null 2>&1; then
    epoch="$(git -C "${REPO_ROOT}" show -s --format=%ct HEAD 2>/dev/null || true)"
  fi
  epoch="${epoch:-0}"
  [[ "${epoch}" =~ ^[0-9]+$ ]] || die "SOURCE_DATE_EPOCH must be an integer"
  printf '%s\n' "${epoch}"
}

prepare_dist() {
  mkdir -p "${DIST_DIR}"
  DIST_DIR="$(cd "${DIST_DIR}" && pwd)"
}

stage_common_files() {
  local destination="$1"
  install -m 0644 "${REPO_ROOT}/README.md" "${destination}/README.md"
  install -m 0644 "${REPO_ROOT}/LICENSE" "${destination}/LICENSE"
  mkdir -p "${destination}/licenses"
  cp -R "${REPO_ROOT}/assets/avatar-codec/licenses" "${destination}/licenses/avatar-codec"
}

stage_server_distribution() {
  local binary="$1"
  local destination="$2"
  [[ -s "${binary}" ]] || die "server binary was not produced: ${binary}"
  mkdir -p "${destination}/migrations"
  install -m 0755 "${binary}" "${destination}/cords-server"
  cp -R "${REPO_ROOT}/migrations/postgres" "${destination}/migrations/postgres"
  find "${destination}/migrations" -type d -exec chmod 0755 {} +
  find "${destination}/migrations" -type f -exec chmod 0644 {} +
  install -m 0644 "${BUILD_SCRIPTS_DIR}/server.toml.example" \
    "${destination}/server.toml.example"
  stage_common_files "${destination}"
}

create_tar_gz() {
  local staging_root="$1"
  local package_name="$2"
  local output="$3"
  local epoch
  epoch="$(source_date_epoch)"
  rm -f -- "${output}" "${output}.sha256"
  if tar --version 2>/dev/null | grep -q 'GNU tar'; then
    tar --sort=name --mtime="@${epoch}" --owner=0 --group=0 --numeric-owner \
      -C "${staging_root}" -czf "${output}" "${package_name}"
  else
    COPYFILE_DISABLE=1 tar -C "${staging_root}" -czf "${output}" "${package_name}"
  fi
  [[ -s "${output}" ]] || die "archive was not created: ${output}"
  write_sha256 "${output}"
}

write_sha256() {
  local artifact="$1"
  local directory basename temporary
  [[ -s "${artifact}" ]] || die "cannot checksum missing artifact: ${artifact}"
  directory="$(cd "$(dirname "${artifact}")" && pwd)"
  basename="$(basename "${artifact}")"
  temporary="${directory}/.${basename}.sha256.tmp"
  rm -f -- "${temporary}"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "${directory}" || exit; sha256sum "${basename}") > "${temporary}"
  elif command -v shasum >/dev/null 2>&1; then
    (cd "${directory}" || exit; shasum -a 256 "${basename}") > "${temporary}"
  else
    die "required SHA-256 tool is unavailable; install sha256sum or shasum"
  fi
  mv -f -- "${temporary}" "${artifact}.sha256"
  [[ -s "${artifact}.sha256" ]] || die "checksum was not created: ${artifact}.sha256"
}

