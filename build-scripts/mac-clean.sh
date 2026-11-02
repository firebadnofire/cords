#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "${script_dir}/lib/clean.sh"
[[ "$(uname -s)" == Darwin ]] || { printf 'error: mac-clean.sh requires macOS\n' >&2; exit 1; }
dry_run=false
case "${1:-}" in
  '') ;;
  --dry-run) dry_run=true ;;
  --help|-h) printf 'Usage: %s [--dry-run]\n' "${0##*/}"; exit 0 ;;
  *) printf 'error: unknown option: %s\n' "$1" >&2; exit 2 ;;
esac
[[ $# -le 1 ]] || { printf 'error: too many arguments\n' >&2; exit 2; }
root="$(cd "${script_dir}/.." && pwd -P)"
clean_repo_outputs "$root" "$dry_run"
