#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_dir}/.." && pwd)"

for command_name in base64 find gpg grep sha256sum wc; do
  command -v "${command_name}" >/dev/null || {
    echo "error: required test command is unavailable: ${command_name}" >&2
    exit 1
  }
done

test_root="$(mktemp -d)"
trap 'rm -rf -- "${test_root}"' EXIT
export GNUPGHOME="${test_root}/keyring"
mkdir -m 0700 "${GNUPGHOME}"
test_passphrase='cords-release-contract-test'
gpg --batch --quiet --pinentry-mode loopback --passphrase "${test_passphrase}" \
  --quick-generate-key 'Cords CI Test <ci-test@example.invalid>' rsa2048 sign 1d
CI_KEY_B64="$(
  gpg --batch --quiet --pinentry-mode loopback --passphrase "${test_passphrase}" \
    --export-secret-keys | base64 --wrap=0
)"
export CI_KEY_B64
export CI_KEY_PASSPHRASE="${test_passphrase}"
unset GNUPGHOME

release_directory="${test_root}/release"
mkdir "${release_directory}"
printf client > "${release_directory}/cords-client-linux-x86_64-0.1.0.tar.gz"
(
  cd "${release_directory}"
  sha256sum cords-client-linux-x86_64-0.1.0.tar.gz > \
    cords-client-linux-x86_64-0.1.0.tar.gz.sha256
)
printf dmg > "${release_directory}/cords-client-macos-universal-0.1.0.dmg"
printf cosign > "${release_directory}/cosign.pub"

bash "${repository_root}/scripts/sign-release-artifacts.sh" "${release_directory}" >/dev/null
test -s "${release_directory}/SHA256SUMS"
test -s "${release_directory}/SHA256SUMS.sig"
test "$(wc -l < "${release_directory}/SHA256SUMS")" -eq 4
if find "${release_directory}" -maxdepth 1 -type f -name '*.asc' -print -quit | grep -q .; then
  echo "error: legacy armored signatures were unexpectedly produced" >&2
  exit 1
fi

echo "Release signing contract passed"
