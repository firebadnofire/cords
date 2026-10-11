#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
original_scripts="${script_dir}"
for command_name in python3 openssl base64 gpg gpgconf sha256sum cmp; do
  command -v "${command_name}" >/dev/null || { echo "error: missing ${command_name}" >&2; exit 1; }
done
umask 077
test_root="$(mktemp -d)"
export GNUPGHOME="${test_root}/keyring"
mkdir -m 0700 "${GNUPGHOME}"
cleanup() {
  local status=$?
  gpgconf --homedir "${test_root}/keyring" --kill gpg-agent || status=1
  rm -rf -- "${test_root}" || status=1
  exit "${status}"
}
trap cleanup EXIT
python3 "${script_dir}/test-release-contract.py"
python3 "${script_dir}/release-identity.py"
source_keys="${script_dir}/../.forgejo/release-keys"
fixture_root="${test_root}/fixture"
mkdir -p "${fixture_root}/scripts" "${fixture_root}/.forgejo/release-keys"
cp "${script_dir}/"{sign-release-artifacts.sh,verify-release-artifacts.sh,release-contract.py,release-identity.py} "${fixture_root}/scripts/"
cp "${source_keys}/"{identity.json,cosign.pub} "${fixture_root}/.forgejo/release-keys/"
script_dir="${fixture_root}/scripts"
# Ephemeral fixture authority, with a distinct signing subkey. No production keys.
test_passphrase=cords-release-contract-test
printf '%s' "${test_passphrase}" | gpg --batch --quiet --pinentry-mode loopback --passphrase-fd 0 \
  --quick-generate-key 'Cords CI Test <ci-test@example.invalid>' ed25519 cert 1d
RELEASE_SIGNING_FINGERPRINT="$(gpg --batch --with-colons --list-keys | awk -F: '$1=="fpr" {print $10;exit}')"
export RELEASE_SIGNING_FINGERPRINT
printf '%s' "${test_passphrase}" | gpg --batch --quiet --pinentry-mode loopback --passphrase-fd 0 \
  --quick-add-key "${RELEASE_SIGNING_FINGERPRINT}" ed25519 sign 1d
printf '%s' "${test_passphrase}" | gpg --batch --quiet --pinentry-mode loopback --passphrase-fd 0 \
  --export-secret-keys "${RELEASE_SIGNING_FINGERPRINT}" > "${test_root}/private.gpg"
CI_KEY_B64="$(base64 --wrap=0 "${test_root}/private.gpg")"
export CI_KEY_B64 CI_KEY_PASSPHRASE="${test_passphrase}"
export RELEASE_GPG_PUBLIC_KEY="${test_root}/public.asc"
gpg --batch --armor --export "${RELEASE_SIGNING_FINGERPRINT}" > "${RELEASE_GPG_PUBLIC_KEY}"
cp "${RELEASE_GPG_PUBLIC_KEY}" "${fixture_root}/.forgejo/release-keys/gpg.pub"
python3 - "${fixture_root}/.forgejo/release-keys" "${RELEASE_SIGNING_FINGERPRINT}" <<'PY'
import hashlib,json,sys
from pathlib import Path
keys=Path(sys.argv[1])
config=json.loads((keys/'identity.json').read_text())
config['gpg_primary_fingerprint']=sys.argv[2]
config['gpg_sha256']=hashlib.sha256((keys/'gpg.pub').read_bytes()).hexdigest()
(keys/'identity.json').write_text(json.dumps(config))
PY
unset GNUPGHOME
release_directory="${test_root}/release"
mkdir "${release_directory}"
# Intentionally fake bytes: these tests cover signing contracts, not packaging.
printf client > "${release_directory}/cords-client-linux-x86_64-0.1.0.tar.gz"
(cd "${release_directory}"; sha256sum cords-client-linux-x86_64-0.1.0.tar.gz > cords-client-linux-x86_64-0.1.0.tar.gz.sha256)
printf dmg > "${release_directory}/cords-client-macos-universal-0.1.0.dmg"
printf pkg > "${release_directory}/cords-client-macos-aarch64-0.1.0.pkg"
cp "${fixture_root}/.forgejo/release-keys/cosign.pub" "${release_directory}/cosign.pub"
bash "${script_dir}/sign-release-artifacts.sh" "${release_directory}" > "${test_root}/sign.log" 2>&1
bash "${script_dir}/verify-release-artifacts.sh" "${release_directory}" > "${test_root}/verify.log" 2>&1
test "$(wc -l < "${release_directory}/SHA256SUMS")" -eq 5
test "$(find "${release_directory}" -name '*.asc' | wc -l)" -eq 6
# Idempotent retry must preserve every signature byte, not regenerate it.
(cd "${release_directory}"; sha256sum -- *.asc SHA256SUMS > "${test_root}/before")
bash "${script_dir}/sign-release-artifacts.sh" "${release_directory}" > "${test_root}/retry.log" 2>&1
(cd "${release_directory}"; sha256sum -- *.asc SHA256SUMS > "${test_root}/after")
cmp "${test_root}/before" "${test_root}/after"
expect_failure() {
  local label="$1"
  shift
  if "$@" > "${test_root}/negative.log" 2>&1; then
    echo "error: ${label} unexpectedly succeeded" >&2; exit 1
  fi
  echo "PASS: ${label} rejected"
}
expect_failure wrong-public-pin env RELEASE_SIGNING_FINGERPRINT=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA \
  bash "${script_dir}/verify-release-artifacts.sh" "${release_directory}"
expect_failure wrong-private-pin env RELEASE_SIGNING_FINGERPRINT=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA \
  bash "${script_dir}/sign-release-artifacts.sh" "${release_directory}"
expect_failure private-key-not-authoritative env -u RELEASE_SIGNING_FINGERPRINT -u RELEASE_GPG_PUBLIC_KEY \
  bash "${original_scripts}/sign-release-artifacts.sh" "${release_directory}"
grep -Fq 'imported primary signing identity differs' "${test_root}/negative.log"
mv "${fixture_root}/.forgejo/release-keys/identity.json" "${test_root}/identity.json"
expect_failure missing-config bash "${script_dir}/sign-release-artifacts.sh" "${release_directory}"
mv "${test_root}/identity.json" "${fixture_root}/.forgejo/release-keys/identity.json"
expect_failure invalid-base64 env CI_KEY_B64=not-base64 bash "${script_dir}/sign-release-artifacts.sh" "${release_directory}"
mv "${release_directory}/cosign.pub.asc" "${test_root}/signature"
expect_failure missing-signature bash "${script_dir}/verify-release-artifacts.sh" "${release_directory}"
mv "${test_root}/signature" "${release_directory}/cosign.pub.asc"
printf altered > "${release_directory}/cords-client-macos-universal-0.1.0.dmg"
expect_failure altered-asset bash "${script_dir}/verify-release-artifacts.sh" "${release_directory}"
expect_failure altered-retry bash "${script_dir}/sign-release-artifacts.sh" "${release_directory}"
printf dmg > "${release_directory}/cords-client-macos-universal-0.1.0.dmg"
printf tampered > "${release_directory}/cosign.pub.asc"
expect_failure altered-signature bash "${script_dir}/verify-release-artifacts.sh" "${release_directory}"
# Same primary identity cannot silently activate an unpublished signing subkey.
printf '%s' "${test_passphrase}" | gpg --homedir "${test_root}/keyring" --batch --quiet \
  --pinentry-mode loopback --passphrase-fd 0 --quick-add-key "${RELEASE_SIGNING_FINGERPRINT}" ed25519 sign 1d
printf '%s' "${test_passphrase}" | gpg --homedir "${test_root}/keyring" --batch --quiet \
  --pinentry-mode loopback --passphrase-fd 0 --export-secret-keys "${RELEASE_SIGNING_FINGERPRINT}" > "${test_root}/updated-private.gpg"
CI_KEY_B64="$(base64 --wrap=0 "${test_root}/updated-private.gpg")"
export CI_KEY_B64
mkdir "${test_root}/unpublished"
printf fixture > "${test_root}/unpublished/client.zip"
expect_failure unpublished-signing-subkey bash "${script_dir}/sign-release-artifacts.sh" "${test_root}/unpublished"
test ! -e "${test_root}/unpublished/SHA256SUMS"
test ! -e "${test_root}/unpublished/client.zip.asc"
echo 'Release signing contract passed: 7 Python tests and 10 rejection scenarios; authoritative pins; all assets signed; retry bytes preserved'
