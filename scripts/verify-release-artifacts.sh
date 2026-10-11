#!/usr/bin/env bash
set -euo pipefail

[[ $# -eq 1 && -d "$1" ]] || { echo "usage: $0 <release-directory>" >&2; exit 2; }
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
release_directory="$(cd "$1" && pwd)"
for command_name in python3 openssl cmp gpg gpgconf mktemp; do
  command -v "${command_name}" >/dev/null || { echo "error: missing ${command_name}" >&2; exit 1; }
done
umask 077
work="$(mktemp -d)"
export GNUPGHOME="${work}/keyring"
mkdir -m 0700 "${GNUPGHOME}"
cleanup() {
  local status=$?
  gpgconf --homedir "${GNUPGHOME}" --kill gpg-agent || status=1
  rm -rf -- "${work}" || status=1
  exit "${status}"
}
trap cleanup EXIT
python3 "${script_dir}/release-identity.py" --fields > "${work}/identity"
mapfile -t pins < "${work}/identity"
expected="${pins[0]}"
if [[ -n "${RELEASE_SIGNING_FINGERPRINT:-}" && "${RELEASE_SIGNING_FINGERPRINT^^}" != "${expected}" ]]; then
  echo 'error: fingerprint override differs from checked-in authority' >&2; exit 1
fi
if [[ -n "${RELEASE_GPG_PUBLIC_KEY:-}" ]]; then
  cmp --silent "${RELEASE_GPG_PUBLIC_KEY}" "${pins[1]}" || {
    echo 'error: supplied public certificate differs from checked-in authority' >&2; exit 1;
  }
fi
RELEASE_GPG_PUBLIC_KEY="${pins[1]}"
if [[ -e "${release_directory}/cosign.pub" ]]; then
  cmp --silent "${release_directory}/cosign.pub" "${pins[2]}" || {
    echo 'error: release Cosign public key differs from checked-in authority' >&2; exit 1;
  }
fi
python3 "${script_dir}/release-contract.py" verify-files "${release_directory}"
gpg --no-options --batch --quiet --import "${RELEASE_GPG_PUBLIC_KEY}"
gpg --batch --with-colons --list-keys > "${work}/keys"
mapfile -t primary_keys < <(awk -F: '$1=="pub" {want=1;next} want && $1=="fpr" {print $10;want=0}' "${work}/keys")
[[ ${#primary_keys[@]} -eq 1 && "${primary_keys[0]}" == "${expected}" ]] || {
  echo 'error: supplied public key differs from the pinned identity' >&2; exit 1;
}
python3 "${script_dir}/release-contract.py" inputs "${release_directory}" > "${work}/inputs"
printf 'SHA256SUMS\n' >> "${work}/inputs"
while IFS= read -r name; do
  gpg --no-options --batch --no-auto-key-retrieve --status-fd 3 \
    --verify "${release_directory}/${name}.asc" "${release_directory}/${name}" 3> "${work}/status"
  python3 "${script_dir}/release-contract.py" signature-status "${expected}" "${work}/status"
done < "${work}/inputs"
echo "Verified exact inventory, checksums and all signatures with pinned primary key ${expected}"
