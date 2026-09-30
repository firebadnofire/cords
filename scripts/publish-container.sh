#!/usr/bin/env bash
set -euo pipefail

: "${CANONICAL_IMAGE:?CANONICAL_IMAGE is required}"
: "${GHCR_IMAGE:?GHCR_IMAGE is required}"
: "${TRACE_TAG:?TRACE_TAG is required}"
: "${RELEASE_TAG:?RELEASE_TAG is required}"
: "${AMD64_TAG:?AMD64_TAG is required}"
: "${ARM64_TAG:?ARM64_TAG is required}"
: "${COSIGN_B64:?configure COSIGN_B64 with the base64-encoded Cosign private key}"
: "${COSIGN_PASSWORD:?configure COSIGN_PASSWORD}"
: "${RELEASE_DIRECTORY:?RELEASE_DIRECTORY is required}"

for value in "${TRACE_TAG}" "${RELEASE_TAG}" "${AMD64_TAG}" "${ARM64_TAG}"; do
  [[ "${value}" =~ ^[A-Za-z0-9._-]+$ ]] || {
    echo "error: unsafe container tag: ${value}" >&2
    exit 2
  }
done

for command in base64 cosign docker jq skopeo; do
  command -v "${command}" >/dev/null || {
    echo "error: required command is unavailable: ${command}" >&2
    exit 1
  }
done

mkdir -p "${RELEASE_DIRECTORY}"
RELEASE_DIRECTORY="$(cd "${RELEASE_DIRECTORY}" && pwd)"
cosign_key="$(mktemp)"
trap 'rm -f -- "${cosign_key}"' EXIT
chmod 0600 "${cosign_key}"
if ! printf '%s' "${COSIGN_B64}" | base64 --decode > "${cosign_key}"; then
  echo "error: COSIGN_B64 is not valid base64" >&2
  exit 1
fi
test -s "${cosign_key}" || {
  echo "error: COSIGN_B64 decoded to an empty file" >&2
  exit 1
}

cosign public-key --key "${cosign_key}" > "${RELEASE_DIRECTORY}/cosign.pub"
test -s "${RELEASE_DIRECTORY}/cosign.pub"

resolve_digest() {
  local reference="$1"
  docker buildx imagetools inspect "${reference}" --format '{{json .Manifest}}' | \
    jq -er '.digest | select(test("^sha256:[0-9a-f]{64}$"))'
}

require_digest() {
  local reference="$1"
  local expected="$2"
  local actual
  actual="$(resolve_digest "${reference}")"
  [[ "${actual}" == "${expected}" ]] || {
    echo "error: ${reference} resolves to ${actual}, expected ${expected}" >&2
    return 1
  }
}

canonical_trace="${CANONICAL_IMAGE}:${TRACE_TAG}"
canonical_release="${CANONICAL_IMAGE}:${RELEASE_TAG}"
canonical_latest="${CANONICAL_IMAGE}:latest"
docker buildx imagetools create \
  --tag "${canonical_trace}" \
  --tag "${canonical_release}" \
  --tag "${canonical_latest}" \
  "${CANONICAL_IMAGE}:${AMD64_TAG}" \
  "${CANONICAL_IMAGE}:${ARM64_TAG}"

canonical_digest="$(resolve_digest "${canonical_trace}")"
require_digest "${canonical_release}" "${canonical_digest}"
require_digest "${canonical_latest}" "${canonical_digest}"

if cosign verify --key "${RELEASE_DIRECTORY}/cosign.pub" \
  "${CANONICAL_IMAGE}@${canonical_digest}" >/dev/null 2>&1; then
  echo "Canonical container already has a valid Cosign signature"
else
  cosign sign --yes --new-bundle-format=false --use-signing-config=false \
    --key "${cosign_key}" "${CANONICAL_IMAGE}@${canonical_digest}"
fi
cosign verify --key "${RELEASE_DIRECTORY}/cosign.pub" \
  "${CANONICAL_IMAGE}@${canonical_digest}" >/dev/null

docker_auth_file="${DOCKER_CONFIG:-${HOME}/.docker}/config.json"
test -s "${docker_auth_file}" || {
  echo "error: Docker registry authentication file is unavailable" >&2
  exit 1
}

ghcr_trace="${GHCR_IMAGE}:${TRACE_TAG}"
skopeo copy --all --preserve-digests --authfile "${docker_auth_file}" \
  "docker://${canonical_trace}" "docker://${ghcr_trace}"

docker buildx imagetools create \
  --tag "${GHCR_IMAGE}:${RELEASE_TAG}" \
  --tag "${GHCR_IMAGE}:latest" \
  "${ghcr_trace}"

ghcr_digest="$(resolve_digest "${ghcr_trace}")"
[[ "${ghcr_digest}" == "${canonical_digest}" ]] || {
  echo "error: GHCR digest ${ghcr_digest} does not match canonical digest ${canonical_digest}" >&2
  exit 1
}
require_digest "${GHCR_IMAGE}:${RELEASE_TAG}" "${canonical_digest}"
require_digest "${GHCR_IMAGE}:latest" "${canonical_digest}"

if cosign verify --key "${RELEASE_DIRECTORY}/cosign.pub" \
  "${GHCR_IMAGE}@${ghcr_digest}" >/dev/null 2>&1; then
  echo "GHCR container already has a valid Cosign signature"
else
  cosign sign --yes --new-bundle-format=false --use-signing-config=false \
    --key "${cosign_key}" "${GHCR_IMAGE}@${ghcr_digest}"
fi
cosign verify --key "${RELEASE_DIRECTORY}/cosign.pub" \
  "${GHCR_IMAGE}@${ghcr_digest}" >/dev/null

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  {
    echo "container_digest=${canonical_digest}"
    echo "canonical_reference=${CANONICAL_IMAGE}@${canonical_digest}"
    echo "ghcr_reference=${GHCR_IMAGE}@${ghcr_digest}"
  } >> "${GITHUB_OUTPUT}"
fi

echo "Published and verified ${CANONICAL_IMAGE}@${canonical_digest}"
echo "Mirrored and verified ${GHCR_IMAGE}@${ghcr_digest}"
