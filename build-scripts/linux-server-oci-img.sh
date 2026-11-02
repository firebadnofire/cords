#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-scripts/lib/common.sh
source "${script_dir}/lib/common.sh"

require_linux
version="$(project_version)"
image_name="${CORDS_IMAGE_NAME:-cords-server}"
image_tag="${CORDS_IMAGE_TAG:-${version}}"
[[ "${image_name}" =~ ^[a-z0-9._/-]+$ ]] || die "unsafe container image name: ${image_name}"
[[ "${image_tag}" =~ ^[A-Za-z0-9._-]+$ ]] || die "unsafe container image tag: ${image_tag}"

if command -v docker >/dev/null 2>&1; then
  engine=docker
elif command -v podman >/dev/null 2>&1; then
  engine=podman
else
  die "Docker or Podman is required to build and load the server image"
fi

reference="${image_name}:${image_tag}"
revision="$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || printf unknown)"
log "Building local OCI image ${reference} with ${engine}"
"${engine}" build --pull \
  --file "${REPO_ROOT}/deploy/Dockerfile" \
  --build-arg "VCS_REF=${revision}" \
  --build-arg "VERSION=${version}" \
  --tag "${reference}" \
  "${REPO_ROOT}"

"${engine}" image inspect "${reference}" >/dev/null || \
  die "container engine cannot inspect the completed image: ${reference}"
log "Loaded local image ${reference}"
printf '%s\n' "${reference}"

