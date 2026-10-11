#!/usr/bin/env bash
set -euo pipefail

[[ $# -eq 1 && -d "$1" ]] || { echo "usage: $0 <release-directory>" >&2; exit 2; }
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
release_directory="$(cd "$1" && pwd)"
: "${CI_KEY_B64:?configure the base64-encoded OpenPGP private key}"
: "${CI_KEY_PASSPHRASE:?configure CI_KEY_PASSPHRASE}"
for command_name in python3 openssl gpg gpgconf base64 cmp mktemp; do
  command -v "${command_name}" >/dev/null || { echo "error: missing ${command_name}" >&2; exit 1; }
done
umask 077
work="$(mktemp -d)"
export GNUPGHOME="${work}/keyring"
mkdir -m 0700 "${GNUPGHOME}" "${work}/stage" "${work}/verification"
cleanup() {
  local status=$?
  gpgconf --homedir "${GNUPGHOME}" --kill gpg-agent || status=1
  gpgconf --homedir "${work}/verification" --kill gpg-agent || status=1
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
pinned_public="${pins[1]}"
if [[ -e "${release_directory}/cosign.pub" ]]; then
  cmp --silent "${release_directory}/cosign.pub" "${pins[2]}" || {
    echo 'error: release Cosign public key differs from checked-in authority' >&2; exit 1;
  }
fi
gpg --no-options --homedir "${work}/verification" --batch --quiet --import "${pinned_public}"
# Preflight before importing private material or mutating outputs. Old .sig
# layouts require a fresh staging directory and are never rewritten in place.
python3 "${script_dir}/release-contract.py" inputs "${release_directory}" > "${work}/inputs"
python3 "${script_dir}/release-contract.py" manifest "${release_directory}" > "${work}/stage/SHA256SUMS"
if [[ -e "${release_directory}/SHA256SUMS" ]]; then
  cmp "${work}/stage/SHA256SUMS" "${release_directory}/SHA256SUMS" || {
    echo 'error: existing manifest differs; use a fresh staging directory' >&2; exit 1;
  }
fi
printf '%s' "${CI_KEY_B64}" | base64 --decode > "${work}/private.gpg"
[[ -s "${work}/private.gpg" ]] || { echo 'error: empty decoded key' >&2; exit 1; }
gpg --batch --quiet --import "${work}/private.gpg"
rm -- "${work}/private.gpg"
gpg --batch --with-colons --list-secret-keys > "${work}/keys"
mapfile -t primary_keys < <(awk -F: '$1=="sec" {want=1;next} want && $1=="fpr" {print $10;want=0}' "${work}/keys")
[[ ${#primary_keys[@]} -eq 1 && "${primary_keys[0]}" == "${expected}" ]] || {
  echo 'error: imported primary signing identity differs from the configured public identity' >&2; exit 1;
}
printf 'SHA256SUMS\n' >> "${work}/inputs"
while IFS= read -r name; do
  input="${release_directory}/${name}"
  [[ "${name}" != SHA256SUMS ]] || input="${work}/stage/SHA256SUMS"
  if [[ -e "${release_directory}/${name}.asc" ]]; then
    gpg --no-options --homedir "${work}/verification" --batch --no-auto-key-retrieve --status-fd 3 \
      --verify "${release_directory}/${name}.asc" "${input}" 3> "${work}/status"
    python3 "${script_dir}/release-contract.py" signature-status "${expected}" "${work}/status"
  else
    printf '%s' "${CI_KEY_PASSPHRASE}" | gpg --batch --pinentry-mode loopback --passphrase-fd 0 \
      --local-user "${expected}" --armor --detach-sign --output "${work}/stage/${name}.asc" "${input}"
    gpg --no-options --homedir "${work}/verification" --batch --no-auto-key-retrieve --status-fd 3 \
      --verify "${work}/stage/${name}.asc" "${input}" 3> "${work}/status"
    python3 "${script_dir}/release-contract.py" signature-status "${expected}" "${work}/status"
  fi
done < "${work}/inputs"
# Only move new verified files; preserve existing signatures and manifest.
for staged in "${work}/stage/"*; do
  destination="${release_directory}/$(basename "${staged}")"
  if [[ -e "${destination}" ]]; then
    [[ "$(basename "${staged}")" == SHA256SUMS ]] && cmp "${staged}" "${destination}"
  else
    mv -n -- "${staged}" "${destination}"
  fi
done
RELEASE_GPG_PUBLIC_KEY="${pinned_public}" RELEASE_SIGNING_FINGERPRINT="${expected}" \
  bash "${script_dir}/verify-release-artifacts.sh" "${release_directory}"
echo "Signed and verified every release asset with pinned primary key ${expected}"
