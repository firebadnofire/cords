#!/usr/bin/env bash
set -euo pipefail

[[ $# -eq 1 ]] || {
  echo "usage: $0 <release-directory>" >&2
  exit 2
}

release_directory="$1"
[[ -d "${release_directory}" ]] || {
  echo "error: release directory does not exist: ${release_directory}" >&2
  exit 1
}
release_directory="$(cd "${release_directory}" && pwd)"

: "${CI_KEY_B64:?configure CI_KEY_B64 with the base64-encoded OpenPGP private key}"
: "${CI_KEY_PASSPHRASE:?configure CI_KEY_PASSPHRASE}"

GNUPGHOME="$(mktemp -d)"
export GNUPGHOME
key_file="${GNUPGHOME}/release-key.gpg"
trap 'rm -rf -- "${GNUPGHOME}"' EXIT
chmod 0700 "${GNUPGHOME}"
umask 077

if ! printf '%s' "${CI_KEY_B64}" | base64 --decode > "${key_file}"; then
  echo "error: CI_KEY_B64 is not valid base64" >&2
  exit 1
fi
test -s "${key_file}" || {
  echo "error: CI_KEY_B64 decoded to an empty file" >&2
  exit 1
}
if ! gpg --batch --quiet --import "${key_file}"; then
  echo "error: CI_KEY_B64 did not decode to an importable OpenPGP private key" >&2
  exit 1
fi
rm -f -- "${key_file}"

signing_fingerprint="$(
  gpg --batch --with-colons --list-secret-keys | \
    awk -F: '$1 == "sec" { want_fingerprint = 1; next } want_fingerprint && $1 == "fpr" { print $10; exit }'
)"
[[ "${signing_fingerprint}" =~ ^[0-9A-Fa-f]{40,64}$ ]] || {
  echo "error: the imported key does not expose a usable primary-key fingerprint" >&2
  exit 1
}

mapfile -d '' signed_inputs < <(
  find "${release_directory}" -maxdepth 1 -type f \
    \( -name '*.tar.gz' -o -name '*.zip' -o -name '*.pkg' -o -name '*.sha256' -o -name 'cosign.pub' \) \
    -print0 | sort -z
)
(( ${#signed_inputs[@]} > 0 )) || {
  echo "error: no client packages, checksums, or cosign.pub were found to sign" >&2
  exit 1
}

for artifact in "${signed_inputs[@]}"; do
  signature="${artifact}.asc"
  rm -f -- "${signature}"
  printf '%s' "${CI_KEY_PASSPHRASE}" | \
    gpg --batch --yes --pinentry-mode loopback --passphrase-fd 0 \
      --local-user "${signing_fingerprint}" --armor --detach-sign \
      --output "${signature}" "${artifact}"
  gpg --batch --verify "${signature}" "${artifact}"
done

(
  cd "${release_directory}"
  printf '%s\0' "${signed_inputs[@]##*/}" | sort -z | xargs -0 sha256sum > SHA256SUMS
)

printf '%s' "${CI_KEY_PASSPHRASE}" | \
  gpg --batch --yes --pinentry-mode loopback --passphrase-fd 0 \
    --local-user "${signing_fingerprint}" --armor --detach-sign \
    --output "${release_directory}/SHA256SUMS.asc" "${release_directory}/SHA256SUMS"
gpg --batch --verify "${release_directory}/SHA256SUMS.asc" "${release_directory}/SHA256SUMS"
(
  cd "${release_directory}"
  sha256sum --check --strict SHA256SUMS
)

echo "Signed and verified release artifacts with ${signing_fingerprint}"
