#!/usr/bin/env bash
set -euo pipefail

[[ $# -eq 1 ]] || {
  echo "usage: $0 <canonical|github>" >&2
  exit 2
}
publication_target="$1"

: "${CANONICAL_IMAGE:?CANONICAL_IMAGE is required}"
: "${TRACE_TAG:?TRACE_TAG is required}"
: "${RELEASE_TAG:?RELEASE_TAG is required}"
: "${COSIGN_B64:?configure COSIGN_B64 with the base64-encoded Cosign private key}"
: "${COSIGN_PASSWORD:?configure COSIGN_PASSWORD}"
: "${RELEASE_DIRECTORY:?RELEASE_DIRECTORY is required}"

for value in "${TRACE_TAG}" "${RELEASE_TAG}"; do
  [[ "${value}" =~ ^[A-Za-z0-9._-]+$ ]] || {
    echo "error: unsafe container tag: ${value}" >&2
    exit 2
  }
done

for command in base64 cmp cosign docker install jq; do
  command -v "${command}" >/dev/null || {
    echo "error: required command is unavailable: ${command}" >&2
    exit 1
  }
done

case "${publication_target}" in
  canonical)
    : "${AMD64_TAG:?AMD64_TAG is required for canonical publication}"
    : "${ARM64_TAG:?ARM64_TAG is required for canonical publication}"
    for value in "${AMD64_TAG}" "${ARM64_TAG}"; do
      [[ "${value}" =~ ^[A-Za-z0-9._-]+$ ]] || {
        echo "error: unsafe architecture tag: ${value}" >&2
        exit 2
      }
    done
    ;;
  github)
    : "${GHCR_IMAGE:?GHCR_IMAGE is required for GitHub mirroring}"
    : "${EXPECTED_CONTAINER_DIGEST:?EXPECTED_CONTAINER_DIGEST is required for GitHub mirroring}"
    [[ "${EXPECTED_CONTAINER_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]] || {
      echo "error: EXPECTED_CONTAINER_DIGEST is not a SHA-256 digest" >&2
      exit 2
    }
    command -v skopeo >/dev/null || {
      echo "error: required command is unavailable: skopeo" >&2
      exit 1
    }
    ;;
  *)
    echo "error: publication target must be canonical or github" >&2
    exit 2
    ;;
esac

mkdir -p "${RELEASE_DIRECTORY}"
RELEASE_DIRECTORY="$(cd "${RELEASE_DIRECTORY}" && pwd)"
cosign_key="$(mktemp)"
derived_public_key="$(mktemp)"
trap 'rm -f -- "${cosign_key}" "${derived_public_key}"' EXIT
chmod 0600 "${cosign_key}"
if ! printf '%s' "${COSIGN_B64}" | base64 --decode > "${cosign_key}"; then
  echo "error: COSIGN_B64 is not valid base64" >&2
  exit 1
fi
test -s "${cosign_key}" || {
  echo "error: COSIGN_B64 decoded to an empty file" >&2
  exit 1
}

cosign public-key --key "${cosign_key}" > "${derived_public_key}"
test -s "${derived_public_key}"
if [[ "${publication_target}" == canonical ]]; then
  install -m 0644 "${derived_public_key}" "${RELEASE_DIRECTORY}/cosign.pub"
else
  test -s "${RELEASE_DIRECTORY}/cosign.pub" || {
    echo "error: canonical cosign.pub is unavailable for GitHub mirroring" >&2
    exit 1
  }
  cmp --silent "${derived_public_key}" "${RELEASE_DIRECTORY}/cosign.pub" || {
    echo "error: COSIGN_B64 does not match the canonical release public key" >&2
    exit 1
  }
fi

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

sign_and_verify() {
  local image="$1"
  local digest="$2"
  local public_key="${RELEASE_DIRECTORY}/cosign.pub"
  if cosign verify --key "${public_key}" "${image}@${digest}" >/dev/null 2>&1; then
    echo "${image}@${digest} already has a valid Cosign signature"
  else
    cosign sign --yes --new-bundle-format=false --use-signing-config=false \
      --key "${cosign_key}" "${image}@${digest}"
  fi
  cosign verify --key "${public_key}" "${image}@${digest}" >/dev/null
}

canonical_trace="${CANONICAL_IMAGE}:${TRACE_TAG}"
canonical_release="${CANONICAL_IMAGE}:${RELEASE_TAG}"
canonical_latest="${CANONICAL_IMAGE}:latest"

if [[ "${publication_target}" == canonical ]]; then
  docker buildx imagetools create \
    --tag "${canonical_trace}" \
    --tag "${canonical_release}" \
    --tag "${canonical_latest}" \
    "${CANONICAL_IMAGE}:${AMD64_TAG}" \
    "${CANONICAL_IMAGE}:${ARM64_TAG}"

  canonical_digest="$(resolve_digest "${canonical_trace}")"
  require_digest "${canonical_release}" "${canonical_digest}"
  require_digest "${canonical_latest}" "${canonical_digest}"
  sign_and_verify "${CANONICAL_IMAGE}" "${canonical_digest}"

  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    {
      echo "container_digest=${canonical_digest}"
      echo "canonical_reference=${CANONICAL_IMAGE}@${canonical_digest}"
    } >> "${GITHUB_OUTPUT}"
  fi
  echo "Published and verified ${CANONICAL_IMAGE}@${canonical_digest}"
  exit 0
fi

require_digest "${canonical_trace}" "${EXPECTED_CONTAINER_DIGEST}"
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

require_digest "${ghcr_trace}" "${EXPECTED_CONTAINER_DIGEST}"
require_digest "${GHCR_IMAGE}:${RELEASE_TAG}" "${EXPECTED_CONTAINER_DIGEST}"
require_digest "${GHCR_IMAGE}:latest" "${EXPECTED_CONTAINER_DIGEST}"
sign_and_verify "${GHCR_IMAGE}" "${EXPECTED_CONTAINER_DIGEST}"

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "ghcr_reference=${GHCR_IMAGE}@${EXPECTED_CONTAINER_DIGEST}" >> "${GITHUB_OUTPUT}"
fi
echo "Mirrored and verified ${GHCR_IMAGE}@${EXPECTED_CONTAINER_DIGEST}"
