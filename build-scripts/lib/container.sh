#!/usr/bin/env bash
# Host-side OCI runner. Source after common.sh; called before host tool checks.
container_select_engine() {
  local requested="${CONTAINER_ENGINE:-}"
  if [[ -n "$requested" ]]; then
    case "$requested" in podman|docker) ;; *) die "CONTAINER_ENGINE must be podman or docker (got: $requested)" ;; esac
    command -v "$requested" >/dev/null 2>&1 || die "requested container engine '$requested' is unavailable"
    CONTAINER_ENGINE="$requested"
  elif command -v podman >/dev/null 2>&1; then CONTAINER_ENGINE=podman
  elif command -v docker >/dev/null 2>&1; then CONTAINER_ENGINE=docker
  else die 'Podman or Docker is required to build on Linux'; fi
  "$CONTAINER_ENGINE" info >/dev/null || die "container engine '$CONTAINER_ENGINE' is not accessible"
}
container_absolute_dir() {
  mkdir -p -- "$1"
  (cd "$1" && pwd -P)
}
container_run_script() {
  local kind="$1"; shift
  require_linux
  REPO_ROOT="$(cd "$REPO_ROOT" && pwd -P)"
  container_select_engine
  case "${CORDS_CONTAINER_TRANSFER:-bind}" in bind|copy) ;; *) die "CORDS_CONTAINER_TRANSFER must be bind or copy" ;; esac
  [[ "${CORDS_CONTAINER_TRANSFER:-bind}" != copy || "$CONTAINER_ENGINE" == docker ]] || die "Docker copy mode requires CONTAINER_ENGINE=docker"
  require_command tar
  local cache_root source_root dist_root target_root qt_root cargo_home_root xwin_cache_root image uid gid label=()
  [[ "$(uname -m)" == x86_64 ]] || [[ "$kind" == server ]] || die "bundled Qt 6.10.3 SDK supports x86_64 Linux only; provide a matching SDK and container image for $(uname -m)"
  cache_root="$(container_absolute_dir "${CORDS_BUILD_CACHE_DIR:-${REPO_ROOT}/.cords-build-cache}")"
  [[ "$cache_root" != "$REPO_ROOT" && "$REPO_ROOT" != "$cache_root"/* ]] || die "CORDS_BUILD_CACHE_DIR must not be the repository or its parent"
  source_root="${cache_root}/source"
  mkdir -p "${cache_root}/cargo" "${cache_root}/home" "${cache_root}/xwin"
  cargo_home_root="$(container_absolute_dir "${CARGO_HOME:-${cache_root}/cargo}")"
  xwin_cache_root="$(container_absolute_dir "${XDG_CACHE_HOME:-${cache_root}/xwin}")"
  dist_root="$(container_absolute_dir "$DIST_DIR")"
  target_root="$(container_absolute_dir "${CARGO_TARGET_DIR:-${REPO_ROOT}/target}")"
  local qt_default="${REPO_ROOT}/target/qt"
  [[ "$kind" == cross ]] && qt_default="${REPO_ROOT}/target/qt-windows-cross"
  qt_root="$(container_absolute_dir "${CORDS_QT_BUILD_DIR:-${qt_default}}")"
  local excludes=(--exclude='./.git' --exclude='./target' --exclude='./dist' --exclude='./.cords-build-cache' --exclude='./.qt-sdk' --exclude='./.qt-tools' --exclude='*/node_modules' --exclude='*/dist' --exclude='*/gen' --exclude='*/__pycache__' --exclude='./.env' --exclude='./.env.*' --exclude='./backups' --exclude='*.db' --exclude='*.db-*' --exclude='*.key' --exclude='*.dump' --exclude='*.tar.gz')
  local generated_dir
  for generated_dir in "$cache_root" "$cargo_home_root" "$xwin_cache_root" "$dist_root" "$target_root" "$qt_root"; do
    case "$generated_dir" in "$REPO_ROOT"/*) excludes+=(--exclude="./${generated_dir#"$REPO_ROOT"/}") ;; esac
  done
  rm -rf -- "$source_root"
  mkdir -p "$source_root"
  # Copy the working tree, including uncommitted edits, without labeling the checkout.
  (cd "$REPO_ROOT" && tar -cf - "${excludes[@]}" .) | (cd "$source_root" && tar -xf -)

  uid="$(id -u)"; gid="$(id -g)"
  [[ "$CONTAINER_ENGINE" == podman ]] && label=(--userns keep-id)
  local suffix=''
  [[ -e /sys/fs/selinux/enforce ]] && suffix=':Z'
  image="${CORDS_BUILD_IMAGE_PREFIX:-cords-build}-${kind}:rust-1.97.1-qt-6.10.3"
  log "Preparing ${kind} build image with ${CONTAINER_ENGINE}"
  "$CONTAINER_ENGINE" build -f "${BUILD_SCRIPTS_DIR}/containers/${kind}.Containerfile" -t "$image" "${BUILD_SCRIPTS_DIR}/containers"
  local mounts=( -v "${source_root}:/workspace:ro${suffix:+,Z}" -v "${dist_root}:/output${suffix}" -v "${target_root}:/build-target${suffix}" -v "${qt_root}:/build-qt${suffix}" -v "${cargo_home_root}:/cache/cargo${suffix}" -v "${cache_root}/home:/cache/home${suffix}" -v "${xwin_cache_root}:/cache/xwin${suffix}" )
  local envs=( -e "SOURCE_DATE_EPOCH=$(source_date_epoch)" -e CORDS_IN_CONTAINER=1 -e CORDS_DIST_DIR=/output -e CARGO_TARGET_DIR=/build-target -e CORDS_QT_BUILD_DIR=/build-qt -e CARGO_HOME=/cache/cargo -e HOME=/cache/home -e XDG_CACHE_HOME=/cache/xwin )
  local variable
  for variable in CORDS_QT_PROFILE CORDS_BUILD_JOBS CARGO_BUILD_JOBS CARGO_INCREMENTAL CARGO_NET_OFFLINE CARGO_ENCODED_RUSTFLAGS RUSTFLAGS SOURCE_DATE_EPOCH CORDS_WINDOWS_TEST_SOURCE_ROOT CORDS_WINDOWS_TEST_OUTPUT_DIR; do
    [[ -z "${!variable:-}" ]] || envs+=( -e "$variable=${!variable}" )
  done
  local prefix container_prefix
  for variable in CORDS_QT_PREFIX QT_ROOT_DIR CORDS_QT_HOST_PREFIX CORDS_QT_WINDOWS_PREFIX CORDS_VC_REDIST; do
    prefix="${!variable:-}"
    [[ -z "$prefix" ]] && continue
    [[ -e "$prefix" ]] || die "$variable does not exist: $prefix"
    prefix="$(cd "$(dirname "$prefix")" && pwd -P)/$(basename "$prefix")"
    container_prefix="/external/${variable}"
    mounts+=( -v "${prefix}:${container_prefix}:ro${suffix:+,Z}" )
    envs+=( -e "${variable}=${container_prefix}" )
  done
  if [[ "$kind" == client && -z "${CORDS_QT_PREFIX:-}" && -n "${QT_ROOT_DIR:-}" ]]; then envs+=( -e CORDS_QT_PREFIX=/external/QT_ROOT_DIR ); fi
  if [[ "$kind" == cross ]]; then
    [[ -n "${CORDS_QT_HOST_PREFIX:-}" ]] || envs+=( -e CORDS_QT_HOST_PREFIX=/opt/qt/6.10.3/gcc_64 )
    [[ -n "${CORDS_QT_WINDOWS_PREFIX:-}" ]] || envs+=( -e CORDS_QT_WINDOWS_PREFIX=/opt/qt/6.10.3/msvc2022_64 )
  fi
  log "Running ${BUILD_SCRIPT_NAME} in ${CONTAINER_ENGINE}"
  if [[ "$CONTAINER_ENGINE" == docker && "${CORDS_CONTAINER_TRANSFER:-}" == copy ]]; then
    [[ -z "${CORDS_QT_PREFIX:-}${QT_ROOT_DIR:-}${CORDS_QT_HOST_PREFIX:-}${CORDS_QT_WINDOWS_PREFIX:-}${CORDS_VC_REDIST:-}" ]] || die 'Docker copy mode requires bundled SDKs and no external file overrides'
    container_run_docker_copy "$image" "$source_root" "$dist_root" "$kind" "${envs[@]}" -- bash "/workspace/build-scripts/${BUILD_SCRIPT_NAME}" "$@"
    return
  fi
  "$CONTAINER_ENGINE" run --rm "${label[@]}" --user "${uid}:${gid}" --workdir /workspace "${mounts[@]}" "${envs[@]}" "$image" bash "/workspace/build-scripts/${BUILD_SCRIPT_NAME}" "$@"
}

# Docker image format retains HEALTHCHECK; Podman's OCI default discards it.
container_build_release_image() {
  local engine_args=()
  [[ "$CONTAINER_ENGINE" == podman ]] && engine_args=(--format docker)
  "$CONTAINER_ENGINE" build "${engine_args[@]}" "$@"
}

# Forgejo jobs may run the Docker CLI inside another container. Docker cp
# transfers the checkout and artifacts without assuming daemon-visible paths.
container_run_docker_copy() (
  set -euo pipefail
  local image="$1" source="$2" output="$3" kind="$4"; shift 4
  local args=() command=() delimiter=false item
  for item in "$@"; do
    if [[ "$item" == -- && "$delimiter" == false ]]; then delimiter=true; continue; fi
    if [[ "$delimiter" == false ]]; then args+=("$item"); else command+=("$item"); fi
  done
  local identity volume_name name container_id status
  require_command sha256sum
  require_command cut
  identity="$(printf '%s' "${FORGEJO_REPOSITORY:-${GITHUB_REPOSITORY:-$REPO_ROOT}}" | sha256sum | cut -c1-12)"
  local mounts=()
  for name in cargo home xwin target qt; do
    volume_name="cords-${identity}-${kind}-${name}"
    "$CONTAINER_ENGINE" volume create "$volume_name" >/dev/null
    case "$name" in
      cargo|home|xwin) mounts+=( -v "${volume_name}:/cache/${name}" ) ;;
      target) mounts+=( -v "${volume_name}:/build-target" ) ;;
      qt) mounts+=( -v "${volume_name}:/build-qt" ) ;;
    esac
  done
  container_id="$("$CONTAINER_ENGINE" create --workdir /workspace "${mounts[@]}" "${args[@]}" "$image" "${command[@]}")"
  trap '"$CONTAINER_ENGINE" rm --force "$container_id" >/dev/null 2>&1 || true' EXIT
  "$CONTAINER_ENGINE" cp "$source/." "${container_id}:/workspace/"
  "$CONTAINER_ENGINE" start --attach "$container_id" || true
  status="$("$CONTAINER_ENGINE" inspect --format '{{.State.ExitCode}}' "$container_id")"
  [[ "$status" =~ ^[0-9]+$ ]] || die "Docker did not report a valid container exit status: $status"
  [[ "$status" == 0 ]] || return "$status"
  "$CONTAINER_ENGINE" cp "${container_id}:/output/." "$output/"
)
