# Cords server operator guide

This guide takes a new operator from an empty host to a reachable Cords server. It covers two
supported artifact layouts:

- the OCI image with Docker or Podman; and
- the unpacked Linux or macOS server tarball, with no container runtime.

Cords is still a development encrypted-messaging milestone, not a production-ready release. Read
the [current limitations](../README.md#validation-boundary) before inviting users.

## Contents

- [Choose a deployment path](#choose-a-deployment-path)
- [Before installation](#before-installation)
- [Verify release artifacts](#verify-release-artifacts)
- [Path A: OCI with Docker or Podman](#path-a-oci-with-docker-or-podman)
- [Path B: unpacked tarball without a container](#path-b-unpacked-tarball-without-a-container)
- [Confirm public reachability](#confirm-public-reachability)
- [Claim the first server owner](#claim-the-first-server-owner)
- [Routine operation](#routine-operation)
- [Common failures](#common-failures)

## Choose a deployment path

| Path             | Best fit                                                            | You provide                                                                        |
| ---------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| OCI image        | A Linux host where Docker or Podman already manages services        | Container engine, PostgreSQL, persistent volumes and an HTTPS reverse proxy        |
| Unpacked tarball | A Linux or macOS host where Cords should run as a normal executable | PostgreSQL, service supervision, persistent directories and an HTTPS reverse proxy |

Both paths run the same `cords-server` binary and PostgreSQL migrations. There is intentionally no
Windows server artifact.

## Before installation

Prepare the following:

- A dedicated DNS name such as `cords.example.com` pointing to the host.
- TCP ports 80 and 443 reachable by the HTTPS reverse proxy. Do not expose PostgreSQL publicly.
- PostgreSQL 17, matching the repository's supported deployment topology.
- Durable storage for PostgreSQL and the Cords server data directory.
- A current Cords image release or verified source tag from the canonical
  [Forgejo releases](https://pubcode.archuser.org/firebadnofire/cords/releases) or the
  [GitHub mirror](https://github.com/firebadnofire/cords/releases).

The public Cords origin must be a bare HTTPS origin. It may contain a port, but it must not contain
a path:

```text
https://cords.example.com
https://cords.example.com:4848
```

The TLS certificate and the persistent Cords signing key serve different purposes. Clients require
normal certificate validation and also pin the signing identity returned by authenticated server
discovery. Never disable certificate validation to work around a deployment problem.

### Configuration sources

Cords loads its TOML configuration first, applies `CORDS_*` environment overrides, and then applies
explicit command-line overrides. Nested TOML names use a double underscore in environment variables:

| TOML setting                       | Environment override                      | Purpose                                         |
| ---------------------------------- | ----------------------------------------- | ----------------------------------------------- |
| `server.listen`                    | `CORDS_SERVER__LISTEN`                    | Private address and port used by `cords-server` |
| `server.public_origin`             | `CORDS_SERVER__PUBLIC_ORIGIN`             | Bare HTTPS origin clients join                  |
| `server.name`                      | `CORDS_SERVER__NAME`                      | Public server display name                      |
| `server.join_policy`               | `CORDS_SERVER__JOIN_POLICY`               | `moderator_approval` (default) or `public`      |
| `server.data_dir`                  | `CORDS_SERVER__DATA_DIR`                  | Persistent server signing-key directory         |
| `database.url`                     | `CORDS_DATABASE__URL`                     | PostgreSQL connection URL                       |
| `database.migrations_dir`          | `CORDS_DATABASE__MIGRATIONS_DIR`          | Migrations packaged with the running release    |
| `authentication.challenge_seconds` | `CORDS_AUTHENTICATION__CHALLENGE_SECONDS` | Short-lived device challenge lifetime           |
| `authentication.session_seconds`   | `CORDS_AUTHENTICATION__SESSION_SECONDS`   | Device session lifetime                         |

The CLI also accepts `--listen`, `--public-origin`, `--server-name`, `--data-dir`,
`--database-url`, and `--migrations-dir`. Prefer one durable configuration mechanism instead of
mixing several overrides. Set `RUST_LOG` only to control structured log filtering; never place
credentials in it.

## Verify release artifacts

Prefer an immutable image digest or a versioned tarball. Avoid deploying `latest` without first
recording the resolved digest.

The automated release publishes Linux x86_64 and ARM64 server tarballs, each with
a neighboring `.sha256`, plus `SHA256SUMS` and its detached OpenPGP signature
`SHA256SUMS.sig`. Obtain the release-signing public key through an independently
trusted project channel, confirm its fingerprint, then verify the manifest and
the downloaded files. A checksum without a verified signature is not proof of
who built an artifact.

```sh
gpg --verify SHA256SUMS.sig SHA256SUMS
sha256sum --check --strict SHA256SUMS
```

The second command expects the complete release payload listed by the manifest
to be present. If you download only the server archive, verify its manifest line
directly instead of treating missing client files as a checksum failure.

For the OCI image, obtain `cosign.pub` from the same release, resolve the image to a digest, and
verify that digest rather than a mutable tag:

```sh
cosign verify --key cosign.pub \
  ghcr.io/firebadnofire/cords@sha256:RELEASE_DIGEST
```

The same multi-architecture image is published at
`pubcode.archuser.org/firebadnofire/cords` and `ghcr.io/firebadnofire/cords`.

## Path A: OCI with Docker or Podman

The commands below use a rootful engine and an administrative shell so protected environment files
remain readable only by the operator. Select one engine and keep using it for the whole installation:

```sh
sudo -s
ENGINE=docker
# Or:
# ENGINE=podman

"$ENGINE" version
```

Rootless Podman is also suitable, but use private paths under the service account instead of
`/etc/cords` and account for rootless networking and low-port forwarding. Do not mix rootful and
rootless commands: they use different image, volume and network stores.

The checked-in `deploy/compose.yaml` is a development topology: it builds from the checkout and
contains a conspicuous development-only database password. Do not deploy it unchanged on a public
host.

Docker Compose recognizes both `compose.yaml` and the legacy `docker-compose.yml` project-file
name. This guide uses `compose.yaml`, but operators who prefer `docker-compose.yml` may use that
name for an operator-owned Compose file without changing its contents. Run Compose commands from
the directory containing the file, or make the choice explicit everywhere:

```sh
docker compose -f docker-compose.yml config
docker compose -f docker-compose.yml up -d --wait
```

Do not keep both names in the same directory unless every command uses `-f`; automatic discovery
could otherwise select a different file than the operator intended. The filename choice does not
make the checked-in development topology suitable for production.

### 1. Create private state and configuration

Create a dedicated directory and a restrictive environment file. Generate a URL-safe database
password so it can be placed in a PostgreSQL URL without additional percent-encoding:

```sh
if test -e /etc/cords/postgres.env || test -e /etc/cords/server.env; then
  echo 'Cords configuration already exists; stop and inspect it instead of overwriting it.' >&2
  exit 1
fi
install -d -m 0700 /etc/cords
DB_PASSWORD="$(openssl rand -hex 32)"
umask 077
printf '%s\n' \
  'POSTGRES_DB=cords' \
  'POSTGRES_USER=cords' \
  "POSTGRES_PASSWORD=$DB_PASSWORD" > /etc/cords/postgres.env
printf '%s\n' \
  'CORDS_SERVER__LISTEN=0.0.0.0:4848' \
  'CORDS_SERVER__PUBLIC_ORIGIN=https://cords.example.com' \
  'CORDS_SERVER__NAME=My Cords Server' \
  'CORDS_SERVER__DATA_DIR=/var/lib/cords' \
  "CORDS_DATABASE__URL=postgres://cords:$DB_PASSWORD@cords-postgres:5432/cords" \
  'CORDS_DATABASE__MIGRATIONS_DIR=/opt/cords/migrations/postgres' \
  > /etc/cords/server.env
unset DB_PASSWORD
```

Create the private network and durable volumes. Repeating these commands is safe; an “already
exists” response means the existing object should be inspected and reused, not deleted:

```sh
"$ENGINE" network inspect cords >/dev/null 2>&1 || "$ENGINE" network create cords
"$ENGINE" volume inspect cords-postgres >/dev/null 2>&1 || "$ENGINE" volume create cords-postgres
"$ENGINE" volume inspect cords-server >/dev/null 2>&1 || "$ENGINE" volume create cords-server
```

### 2. Start PostgreSQL

```sh
"$ENGINE" run -d \
  --name cords-postgres \
  --restart unless-stopped \
  --network cords \
  --env-file /etc/cords/postgres.env \
  --volume cords-postgres:/var/lib/postgresql/data \
  docker.io/library/postgres:17-bookworm
```

Wait for PostgreSQL without publishing its port:

```sh
until "$ENGINE" exec cords-postgres pg_isready -U cords -d cords; do sleep 2; done
```

### 3. Migrate and start Cords

Select and verify an immutable image first:

```sh
CORDS_IMAGE='ghcr.io/firebadnofire/cords@sha256:RELEASE_DIGEST'
"$ENGINE" pull "$CORDS_IMAGE"
```

Run the packaged forward migrations using the protected server environment:

```sh
"$ENGINE" run --rm \
  --network cords \
  --env-file /etc/cords/server.env \
  "$CORDS_IMAGE" --config /opt/cords/server.toml migrate
```

Start the server on a loopback-only host port. Replace the origin and display name:

```sh
"$ENGINE" run -d \
  --name cords-server \
  --restart unless-stopped \
  --network cords \
  --publish 127.0.0.1:4849:4848 \
  --volume cords-server:/var/lib/cords \
  --env-file /etc/cords/server.env \
  "$CORDS_IMAGE" --config /opt/cords/server.toml serve --require-current-schema
```

The PostgreSQL URL remains visible to an engine administrator through the container configuration.
Protect `/etc/cords/server.env`, treat engine-administrator access as privileged, and never expose
an unauthenticated Docker or Podman API.

Docker applies `--restart unless-stopped` across daemon restarts. For Podman, configure the host's
`podman-restart.service` or translate the containers into managed Quadlet units so they return after
a reboot; verify this behavior with a planned host restart before inviting users.

### 4. Put HTTPS in front

Keep Cords on `127.0.0.1:4849` and use the host's existing TLS reverse proxy. A minimal Caddy site
is:

```caddyfile
cords.example.com {
    tls {
        protocols tls1.3
    }
    header Strict-Transport-Security "max-age=31536000"
    reverse_proxy 127.0.0.1:4849
}
```

Caddy needs ports 80 and 443 for automatic public certificate issuance. If another proxy is used,
require certificate validation, prefer TLS 1.3, disable SSL and TLS 1.0/1.1, and forward requests
without rewriting `/.well-known/cords/server` or `/api/v1`.

Allow public ingress only to the reverse proxy. Keep host port 4849 bound to loopback and leave
PostgreSQL unexposed.

## Path B: unpacked tarball without a container

The tarball contains `cords-server`, `migrations/postgres/`, `server.toml.example`, `README.md` and
`LICENSE`. PostgreSQL and the reverse proxy are not bundled.

### 1. Install PostgreSQL and create the database

Use the operating system's PostgreSQL 17 packages and keep the database listener private. On a
typical Linux installation, create the role interactively so its password does not enter shell
history:

```sh
sudo -u postgres createuser --pwprompt cords
sudo -u postgres createdb --owner cords cords
```

If the password contains URL-reserved characters, percent-encode it before placing it in the
database URL.

### 2. Unpack the release

After completing release verification, install into a versioned directory and maintain a stable
symlink. Replace the filename, architecture and version with the downloaded artifact:

```sh
sudo install -d -m 0755 /opt/cords/releases
tar -xzf cords-server-linux-x86_64-VERSION.tar.gz
sudo mv cords-server-linux-x86_64 /opt/cords/releases/VERSION
sudo ln -sfn /opt/cords/releases/VERSION /opt/cords/current
```

On macOS, use the matching `cords-server-macos-<architecture>-<version>.tar.gz` artifact and a
host-appropriate installation directory. The foreground server commands are otherwise equivalent.

### 3. Create a least-privilege service account

On Linux:

```sh
id cords >/dev/null 2>&1 || \
  sudo useradd --system --home-dir /var/lib/cords --shell /usr/sbin/nologin cords
sudo install -d -o cords -g cords -m 0700 /var/lib/cords
sudo install -d -o root -g cords -m 0750 /etc/cords
```

If the account already exists, inspect its home, shell and group rather than recreating it.

### 4. Configure the server

Create `/etc/cords/server.toml` with mode `0640`, owned by `root:cords`:

```toml
[server]
listen = "127.0.0.1:4849"
public_origin = "https://cords.example.com"
name = "My Cords Server"
join_policy = "moderator_approval"
data_dir = "/var/lib/cords"

[database]
url = "postgres://cords:URL_ENCODED_PASSWORD@127.0.0.1:5432/cords"
migrations_dir = "/opt/cords/current/migrations/postgres"

[authentication]
challenge_seconds = 60
session_seconds = 900
```

`server.join_policy` defaults to `moderator_approval`. A verified new device then
receives a pending response and cannot authenticate until the owner approves it
in Server settings → Members. Set `join_policy = "public"` explicitly only when
immediate admission is intended. Admission does not grant `channel.create`:
only the claimed owner can create channels in this implementation. Approval
does not add a device to an MLS conversation; an authorized existing channel
member must separately add it before ciphertext and history are available.
Existing memberships and channel records are not rewritten by the migration.

Protect it:

```sh
sudo chown root:cords /etc/cords/server.toml
sudo chmod 0640 /etc/cords/server.toml
```

### 5. Migrate and validate in the foreground

Migrations are forward-only. Back up an existing database before applying migrations; for a new
empty database, run:

```sh
sudo -u cords /opt/cords/current/cords-server \
  --config /etc/cords/server.toml migrate

sudo -u cords /opt/cords/current/cords-server \
  --config /etc/cords/server.toml serve --require-current-schema
```

In another terminal:

```sh
curl --fail --silent --show-error --output /dev/null http://127.0.0.1:4849/health/ready
```

Stop the foreground process after readiness succeeds.

### 6. Install a Linux systemd service

Create `/etc/systemd/system/cords-server.service`:

```ini
[Unit]
Description=Cords messaging server
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
Type=simple
User=cords
Group=cords
WorkingDirectory=/var/lib/cords
ExecStart=/opt/cords/current/cords-server --config /etc/cords/server.toml serve --require-current-schema
Restart=on-failure
RestartSec=5s
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/cords
CapabilityBoundingSet=
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX

[Install]
WantedBy=multi-user.target
```

Load and start it:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now cords-server
sudo systemctl status cords-server
```

Distribution service names vary; adjust `postgresql.service` in `After=` if the local PostgreSQL
unit has a different name.

On macOS, run the same binary under a dedicated account using the platform service manager. Do not
run the server from an interactive terminal as a long-term deployment.

### 7. Configure HTTPS

Use the same reverse-proxy policy shown in the OCI section. Only the proxy should be public;
`127.0.0.1:4849` and PostgreSQL remain private.

## Confirm public reachability

After DNS and TLS are active, verify readiness and signed discovery from a different machine:

```sh
curl --fail --silent --show-error --output /dev/null \
  https://cords.example.com/health/ready
curl --fail --silent --show-error \
  https://cords.example.com/.well-known/cords/server
```

Do not use `--insecure`. Fix DNS, certificate, chain or clock errors instead. A Cords desktop client
joins using only `https://cords.example.com`; it validates HTTPS, verifies signed discovery, and
pins the discovered server identity. A later signing-key change is rejected.

## Claim the first server owner

Joining does not imply ownership. After migrations and persistent identity setup, a fresh empty
server atomically enters `UNCLAIMED`, generates a 256-bit single-use code and emits the plaintext
once in the server log. PostgreSQL stores only a server-bound verifier. Restrict access to this log:

```sh
docker compose logs cords-server
```

Connect the desktop client to the normal HTTPS origin. It validates the certificate, verifies and
pins signed discovery, verifies signed ownership state, and presents the claim form before any
ordinary membership is created. Enter the logged code. The client proves both its device key and
account-root key; one database transaction creates the owner membership, consumes the challenge,
clears the verifier and marks the server `CLAIMED`.

Restarting an unclaimed server does not show or rotate the code. If an unused code may be exposed,
rotate it with a server-local action. For Compose:

```sh
docker compose run --rm cords-server --config /opt/cords/server.toml ownership-bootstrap rotate
```

For a tarball install, use the same database URL, migrations directory and data directory as the
service:

```sh
sudo -u cords /opt/cords/bin/cords-server \
  --config /etc/cords/server.toml ownership-bootstrap rotate
```

The rotation command is refused after ownership is claimed. It emits the replacement exactly once;
store it securely and do not put it in shell arguments, environment files or client storage.

An upgraded database that already has an owner is marked `CLAIMED` automatically. A populated
database with no owner and no bootstrap state fails closed instead of selecting the oldest account.
After taking and verifying a backup, the operator may explicitly create unclaimed state with the
same local command using `initialize` instead of `rotate`. This does not grant authority to any
existing membership; the eventual claimant must still supply the code and both signatures.

The current owner receives real `server.manage` and `channel.manage` capabilities. Invite issuance
and banning remain unavailable. Root-authenticated succession and operator recovery are specified
in [ADR 0005](adr/0005-server-departure-burn-succession.md).

### Owner lockdown and recovery

Back up PostgreSQL and the matching server signing-key directory before administrative changes.
An owner burn transfers to an accepted successor only after 30 full days from server acceptance;
otherwise it immediately enters `OWNER_LOCKDOWN`. Ordinary owner departure also enters lockdown.
The original first-owner bootstrap never reopens.

For an intentional operator-led ownership change, run the local administrative command:

```sh
cords-server --config /etc/cords/server.toml ownership-lockdown --confirm
cords-server --config /etc/cords/server.toml ownership-recovery-code
```

For a server already in lockdown, only the second command is needed. It emits a fresh one-time
code to restricted operator logs; rotation invalidates any earlier code. Give it only to the
intended new owner. In the client, click the red exclamation server icon and submit the code.
Incorrect submissions do not consume it or automatically prompt again. Ordinary members cannot
obtain the code. Normal messaging/membership mutation resumes only after an atomic successful
claim. A burned root cannot be reused for recovery. Existing server data is retained.

## Routine operation

### Health and logs

The endpoints have distinct meanings:

- `/health/live` means the HTTP process is running.
- `/health/ready` is exposed after database schema validation and server identity loading succeed.
- `/.well-known/cords/server` returns signed public discovery metadata.
- `/api/v1/ownership` returns signed `UNCLAIMED`, `CLAIMED` or `OWNER_LOCKDOWN` state, never a code.

Container logs:

```sh
"$ENGINE" logs --tail 200 cords-server
"$ENGINE" inspect --format '{{json .State.Health}}' cords-server
```

Tarball/systemd logs:

```sh
sudo journalctl -u cords-server --since today
sudo systemctl status cords-server
```

Errors are emitted as structured JSON. Do not post environment files, database URLs, ownership
codes or private signing-key material into support channels.

### Backups

Two server-owned state sets must be preserved together:

1. a PostgreSQL logical backup; and
2. the Cords data directory containing `server-signing.key`.

The database is bound to that signing identity. Restoring one without the other intentionally
fails rather than silently creating a different server identity.

OCI example:

```sh
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
install -d -m 0700 backups
"$ENGINE" exec cords-postgres pg_dump -U cords -d cords -Fc \
  > "backups/cords-$stamp.dump"
"$ENGINE" cp cords-server:/var/lib/cords/. "backups/server-state-$stamp"
test -s "backups/cords-$stamp.dump"
test -s "backups/server-state-$stamp/server-signing.key"
```

Tarball example:

```sh
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
sudo install -d -m 0700 /var/backups/cords
set -o pipefail
sudo -u postgres pg_dump -d cords -Fc | \
  sudo tee "/var/backups/cords/cords-$stamp.dump" >/dev/null
sudo cp -a /var/lib/cords "/var/backups/cords/server-state-$stamp"
```

Encrypt backups at rest and test restoration into an isolated deployment. Client-held private keys,
MLS state and message plaintext are not part of a server backup.

### Upgrades

1. Read the release notes and verify the new artifact.
2. Back up PostgreSQL and the server data directory.
3. Stop Cords while leaving PostgreSQL available.
4. Run the new artifact's `migrate` command exactly once.
5. Start the new artifact with `serve --require-current-schema`.
6. Verify readiness, discovery, the unchanged server fingerprint and a real client reconnection.

For tarballs, update `/opt/cords/current` only after verification and backup. For OCI, recreate the
server container with the new immutable digest while reusing the existing server volume and database.
Use the same stop/remove/start procedure described for ownership; do not delete either named volume.

Database migrations are forward-only. Repointing the executable or image to an older version is not
a safe rollback after migration. Restore the matching pre-upgrade database and server-data backup
into an isolated environment first, verify it, and only then replace the failed deployment.

## Common failures

| Symptom                                            | Check                                                                                                                                                |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `server.public_origin must be a bare HTTPS origin` | Remove paths and use the externally reachable HTTPS origin, including its non-default port if needed.                                                |
| Database connection failure                        | Confirm PostgreSQL readiness, private network routing, credentials and percent-encoding in the URL.                                                  |
| Database schema is not current                     | Run the same release's `migrate` command before `serve --require-current-schema`.                                                                    |
| Signing identity missing or mismatched             | Stop. Restore the matching server data directory; do not generate a replacement key for an established database.                                     |
| Client reports a server-key change                 | Stop. Confirm the expected signing key or complete a separately specified trust reset; a valid TLS certificate alone does not authorize replacement. |
| Public readiness works but clients cannot join     | Inspect signed discovery, `public_origin`, reverse-proxy routing, system clocks and certificate validity.                                            |
| Messages synchronize but realtime updates fail     | Confirm the proxy permits WebSocket upgrades on `/api/v1/events` and does not strip the Cords subprotocol.                                           |
| Ownership claim is denied                          | Confirm the 43-character code was copied from the initial or rotation log, is still unused, and the client trusts the expected HTTPS origin and server pin. |
| Server requests explicit bootstrap initialization | The database has memberships but no owner. Stop, take a verified backup, then run the local `ownership-bootstrap initialize` command; no account is selected automatically. |

If a corrective action would delete a volume, reset trust, replace `server-signing.key`, restore a
database, or discard an existing owner record, stop and take a verified backup first.
