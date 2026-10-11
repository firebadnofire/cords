#!/usr/bin/env bash
# Local, disposable Cords development deployment.
# Usage: ./cords-up.sh up | start | restart | stop | down [--yes] | status
set -Eeuo pipefail

# Match the existing server + Raspberry Pi Nginx topology.
readonly INSTANCE="${CORDS_INSTANCE:-cords}"
[[ "$INSTANCE" =~ ^[a-z][a-z0-9-]{0,62}$ ]] || { printf 'Invalid CORDS_INSTANCE\n' >&2; exit 2; }
readonly SERVER_CONTAINER="${INSTANCE}-server"
readonly POSTGRES_CONTAINER="${INSTANCE}-postgres"
readonly PUBLIC_ORIGIN="${CORDS_PUBLIC_ORIGIN:-https://cords.archuser.org}"
readonly SERVER_NAME="${CORDS_SERVER_NAME:-Cords}"
readonly JOIN_POLICY="${CORDS_JOIN_POLICY:-moderator_approval}"
readonly PUBLISH="${CORDS_PUBLISH:-0.0.0.0:4849:4848}"  # Restrict production HTTP to the TLS proxy.
[[ "$PUBLISH" =~ ^[0-9.]+:([0-9]+):4848$ ]] || { printf 'Use IPv4-address:port:4848 for CORDS_PUBLISH\n' >&2; exit 2; }
readonly HEALTH_PORT="${BASH_REMATCH[1]}"

export CORDS_IMAGE_NAME="${CORDS_IMAGE_NAME:-$SERVER_CONTAINER}"
export CORDS_IMAGE_TAG="${CORDS_IMAGE_TAG:-0.1.0}"
readonly IMAGE="${CORDS_IMAGE_NAME}:${CORDS_IMAGE_TAG}"
readonly POSTGRES_IMAGE='postgres:17-bookworm'
readonly CONF_DIR="${CORDS_CONF_DIR:-/etc/$INSTANCE}"
[[ "$CONF_DIR" == /* && "$CONF_DIR" != / && "$CONF_DIR" != /etc && "$CONF_DIR" != /home && "$CONF_DIR" != /tmp && "$CONF_DIR" != *'/../'* && "$CONF_DIR" != */.. ]] || { printf 'Unsafe CORDS_CONF_DIR\n' >&2; exit 2; }
if [[ "$INSTANCE" != cords ]]; then
  [[ "$CORDS_IMAGE_NAME" == "$INSTANCE"-* ]] || { printf 'Named instances require an instance-specific image name\n' >&2; exit 2; }
fi
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR

log() { printf '[cords-up] %s\n' "$*"; }
die() { printf '[cords-up] ERROR: %s\n' "$*" >&2; exit 1; }
usage() {
  cat <<'HELP'
Usage:
  ./cords-up.sh up             Build and deploy a NEW Cords server
  ./cords-up.sh start          Start existing containers; preserve all state
  ./cords-up.sh restart        Stop/start existing containers; preserve all state
  ./cords-up.sh stop           Stop existing containers; preserve all state
  ./cords-up.sh down           PERMANENTLY purge Cords (asks for confirmation)
  ./cords-up.sh down --yes     Purge without interactive confirmation
  ./cords-up.sh status         Show Cords resources

Place this script at the root of the Cords source checkout, alongside
build-scripts/. Up refuses to overwrite any existing Cords installation.
Named test deployment: set CORDS_INSTANCE, CORDS_CONF_DIR, CORDS_PUBLISH,
CORDS_PUBLIC_ORIGIN and optionally CORDS_JOIN_POLICY / CORDS_SERVER_NAME.
Default production names/configuration remain unchanged. Named resources are labelled.
Keep the same environment for subsequent lifecycle operations. Stop is not purge.
HELP
}
require() { command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"; }
as_root() {
  if (( EUID == 0 )); then "$@"; else sudo "$@"; fi
}

preflight() {
  require docker
  docker info >/dev/null 2>&1 || die 'Cannot reach Docker. Check Docker service/group permissions.'
  if (( EUID != 0 )); then require sudo; fi
}

container_exists() { docker container inspect "$1" >/dev/null 2>&1; }
volume_exists()    { docker volume inspect "$1" >/dev/null 2>&1; }
network_exists()   { docker network inspect "$1" >/dev/null 2>&1; }
image_exists()     { docker image inspect "$1" >/dev/null 2>&1; }

assert_fresh() {
  local name
  for name in "$SERVER_CONTAINER" "$POSTGRES_CONTAINER"; do
    ! container_exists "$name" || die "Container $name exists; inspect it or run down first."
  done
  for name in "$SERVER_CONTAINER" "$POSTGRES_CONTAINER"; do
    ! volume_exists "$name" || die "Volume $name exists; inspect it or run down first."
  done
  ! network_exists "$INSTANCE" || die "Docker network $INSTANCE exists; inspect it before proceeding."
  ! as_root test -e "$CONF_DIR" || die "$CONF_DIR exists; inspect it or run down first."
}

wait_for_postgres() {
  local attempt
  for (( attempt=1; attempt<=60; attempt++ )); do
    if docker exec "$POSTGRES_CONTAINER" pg_isready -U cords -d cords >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  die "PostgreSQL did not become ready. Inspect: docker logs $POSTGRES_CONTAINER"
}

wait_for_cords() {
  local attempt
  for (( attempt=1; attempt<=60; attempt++ )); do
    if curl --max-time 2 -fsS -o /dev/null "http://127.0.0.1:$HEALTH_PORT/health/ready" 2>/dev/null; then
      return 0
    fi
    sleep 2
  done
  die "Cords did not become ready. Inspect: docker logs $SERVER_CONTAINER"
}

# Legacy default deployments predate labels. Named instances must prove ownership
# before any existing resource is started, stopped or purged.
assert_owned() {
  [[ "$INSTANCE" != cords ]] || return 0
  local kind name actual
  local -a names
  for kind in container volume network; do
    case "$kind" in
      container|volume) names=("$SERVER_CONTAINER" "$POSTGRES_CONTAINER") ;;
      network) names=("$INSTANCE") ;;
    esac
    for name in "${names[@]}"; do
      if docker "$kind" inspect "$name" >/dev/null 2>&1; then
        if [[ "$kind" == container ]]; then
          actual="$(docker "$kind" inspect --format '{{index .Config.Labels "chat.cords.instance"}}' "$name")"
        else
          actual="$(docker "$kind" inspect --format '{{index .Labels "chat.cords.instance"}}' "$name")"
        fi
        [[ "$actual" == "$INSTANCE" ]] || die "Ownership label mismatch: $kind $name"
      fi
    done
  done
  if as_root test -e "$CONF_DIR"; then
    actual="$(as_root cat "$CONF_DIR/instance" 2>/dev/null)" || die "Missing instance marker: $CONF_DIR"
    [[ "$actual" == "$INSTANCE" ]] || die "Configuration ownership mismatch: $CONF_DIR"
  fi
}

stop() {
  assert_owned
  local name
  for name in "$SERVER_CONTAINER" "$POSTGRES_CONTAINER"; do
    if container_exists "$name"; then docker stop "$name" >/dev/null; fi
  done
  log "Stopped $INSTANCE; database, server keys, volumes, configuration and image retained."
}

start() {
  assert_owned
  if ! container_exists "$POSTGRES_CONTAINER" || ! container_exists "$SERVER_CONTAINER"; then
    die 'Existing deployment containers are required; use up for a fresh instance.'
  fi
  docker start "$POSTGRES_CONTAINER" >/dev/null
  wait_for_postgres
  docker start "$SERVER_CONTAINER" >/dev/null
  wait_for_cords
  log "Started $INSTANCE with existing durable state."
}

up() {
  require openssl
  require curl
  if (( EUID != 0 )); then as_root -v; fi
  assert_fresh
  [[ -f "$SCRIPT_DIR/build-scripts/linux-server-oci-img.sh" ]] ||
    die 'Place cords-up.sh in the Cords repository root, next to build-scripts/.'

  log "Building fresh local image: $IMAGE"
  ( cd "$SCRIPT_DIR" && bash ./build-scripts/linux-server-oci-img.sh )
  image_exists "$IMAGE" || die "The build did not produce $IMAGE"

  log 'Creating private configuration and generating a new database password'
  as_root install -d -m 0700 "$CONF_DIR"
  printf '%s\n' "$INSTANCE" | as_root tee "$CONF_DIR/instance" >/dev/null
  as_root chmod 0600 "$CONF_DIR/instance"
  as_root install -m 0600 /dev/null "$CONF_DIR/postgres.env"
  as_root install -m 0600 /dev/null "$CONF_DIR/server.env"
  local db_password
  db_password="$(openssl rand -hex 32)"
  printf '%s\n' \
    'POSTGRES_DB=cords' \
    'POSTGRES_USER=cords' \
    "POSTGRES_PASSWORD=$db_password" | as_root tee "$CONF_DIR/postgres.env" >/dev/null
  printf '%s\n' \
    'CORDS_SERVER__LISTEN=0.0.0.0:4848' \
    "CORDS_SERVER__PUBLIC_ORIGIN=$PUBLIC_ORIGIN" \
    "CORDS_SERVER__NAME=$SERVER_NAME" \
    "CORDS_SERVER__JOIN_POLICY=$JOIN_POLICY" \
    'CORDS_SERVER__DATA_DIR=/var/lib/cords' \
    "CORDS_DATABASE__URL=postgres://cords:$db_password@$POSTGRES_CONTAINER:5432/cords" \
    'CORDS_DATABASE__MIGRATIONS_DIR=/opt/cords/migrations/postgres' \
    'RUST_LOG=info' | as_root tee "$CONF_DIR/server.env" >/dev/null
  unset db_password

  log 'Creating isolated Docker network and persistent volumes'
  docker network create --label "chat.cords.instance=$INSTANCE" "$INSTANCE" >/dev/null
  docker volume create --label "chat.cords.instance=$INSTANCE" "$POSTGRES_CONTAINER" >/dev/null
  docker volume create --label "chat.cords.instance=$INSTANCE" "$SERVER_CONTAINER" >/dev/null

  log 'Starting PostgreSQL 17'
  as_root docker run -d --name "$POSTGRES_CONTAINER" --restart unless-stopped \
    --label "chat.cords.instance=$INSTANCE" \
    --network "$INSTANCE" --env-file "$CONF_DIR/postgres.env" \
    --volume "$POSTGRES_CONTAINER:/var/lib/postgresql/data" \
    "$POSTGRES_IMAGE" >/dev/null
  wait_for_postgres

  log 'Applying database migrations'
  as_root docker run --rm --network "$INSTANCE" --env-file "$CONF_DIR/server.env" \
    "$IMAGE" --config /opt/cords/server.toml migrate

  log "Starting Cords at $PUBLISH"
  as_root docker run -d --name "$SERVER_CONTAINER" --restart unless-stopped \
    --label "chat.cords.instance=$INSTANCE" \
    --network "$INSTANCE" --publish "$PUBLISH" \
    --volume "$SERVER_CONTAINER:/var/lib/cords" \
    --env-file "$CONF_DIR/server.env" \
    "$IMAGE" --config /opt/cords/server.toml serve --require-current-schema >/dev/null
  wait_for_cords

  log 'Cords is ready. Verify HTTPS through the existing Nginx proxy:'
  printf '  curl -fsS %s/health/ready\n' "$PUBLIC_ORIGIN"
  printf '  curl -fsS %s/.well-known/cords/server\n' "$PUBLIC_ORIGIN"
  printf '  curl -fsS %s/api/v1/ownership\n' "$PUBLIC_ORIGIN"
  log 'For the ONE-TIME ownership claim code, read the LOCAL logs:'
  printf '  docker logs %s\n' "$SERVER_CONTAINER"
  log 'Do not share the bootstrap code. Restrict host TCP/4849 to your reverse proxy.'
}

down() {
  assert_owned
  local confirmed="${1:-}"
  if [[ "$confirmed" != '--yes' ]]; then
    cat >&2 <<'WARNING'
WARNING: This permanently deletes ONLY the named Cords Docker containers,
volumes, network, local image tag, and selected configuration directory.
All Cords messages, database records, bootstrap/owner state, and the server
signing key will be lost. Clients will reject the new server identity until
their old server trust pin is deliberately reset.
WARNING
    printf 'Selected target: %s; configuration: %s; image: %s\n' "$INSTANCE" "$CONF_DIR" "$IMAGE" >&2
    [[ -t 0 ]] || die 'Interactive confirmation required (or use down --yes).'
    local answer
    read -r -p 'Type PURGE to continue: ' answer
    [[ "$answer" == 'PURGE' ]] || die 'Purge cancelled.'
  fi

  log "Confirmed purge target: $INSTANCE, $CONF_DIR, $IMAGE"

  local name
  for name in "$SERVER_CONTAINER" "$POSTGRES_CONTAINER"; do
    if container_exists "$name"; then
      log "Removing container $name"
      docker rm -f "$name" >/dev/null
    fi
  done
  for name in "$SERVER_CONTAINER" "$POSTGRES_CONTAINER"; do
    if volume_exists "$name"; then
      log "Removing persistent volume $name"
      docker volume rm "$name" >/dev/null
    fi
  done
  if network_exists "$INSTANCE"; then
    log "Removing Docker network $INSTANCE"
    docker network rm "$INSTANCE" >/dev/null
  fi
  if as_root test -e "$CONF_DIR"; then
    log "Deleting $CONF_DIR"
    as_root rm -rf -- "$CONF_DIR"
  fi
  if image_exists "$IMAGE"; then
    log "Removing locally built image tag $IMAGE"
    docker image rm "$IMAGE" >/dev/null
  fi
  log 'Cords purge completed. Other Docker services were not touched.'
}

status() {
  log 'Containers:'
  docker ps -a --filter "name=^/$SERVER_CONTAINER$" --filter "name=^/$POSTGRES_CONTAINER$"
  log 'Volumes:'
  docker volume ls --filter "name=^$SERVER_CONTAINER$" --filter "name=^$POSTGRES_CONTAINER$"
  log 'Network:'
  docker network ls --filter "name=^$INSTANCE$"
  log 'Local image:'
  docker image ls "$IMAGE"
  if as_root test -e "$CONF_DIR"; then log "$CONF_DIR exists"; else log "$CONF_DIR absent"; fi
}

main() {
  (( $# >= 1 )) || { usage; exit 2; }
  case "$1" in
    up)
      (( $# == 1 )) || { usage; exit 2; }
      preflight
      up
      ;;
    down)
      (( $# == 1 )) || { (( $# == 2 )) && [[ "$2" == '--yes' ]]; } || { usage; exit 2; }
      preflight
      if (( EUID != 0 )); then as_root -v; fi
      down "${2:-}"
      ;;
    start|stop|restart)
      (( $# == 1 )) || { usage; exit 2; }
      preflight
      if [[ "$1" == restart ]]; then stop; start; else "$1"; fi
      ;;
    status)
      (( $# == 1 )) || { usage; exit 2; }
      preflight
      status
      ;;
    -h|--help|help) usage ;;
    *) usage; exit 2 ;;
  esac
}

main "$@"
