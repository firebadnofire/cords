# Encrypted conversation development milestone

This CLI-first slice uses the real `cords-client-core`, native identities, OpenMLS,
PostgreSQL and independent SQLite installations. See ADR 0002 and the
[evidence report](reports/encrypted-milestone.md) for tested behavior and remaining
gates. A successful conversation is not a production release claim.

## Runtime and trust

Use Rust 1.97.1, Node 24, Python 3, SSH, Docker and Compose. Windows desktop also
requires Tauri prerequisites and WebView2. Linux desktop credential storage needs
a working Secret Service. Synchronize host clocks: creation/issuance timestamps
permit five seconds of skew; authorization, challenge and session expiry do not.

The isolated deployment is `/home/william/cords-milestone` on `192.168.86.54`,
Compose project `cords-milestone`. Its sole published port is
`192.168.86.54:5848`, origin `https://192.168.86.54:5848`. Caddy terminates TLS 1.3
and WSS, sets HSTS, and forwards to private HTTP. PostgreSQL is not published.
Do not replace or stop the separate `cords-phase0` deployment.

Explicitly trust the development CA using `--ca` or `CORDS_CLIENT_CA`. Certificate
and hostname/IP validation remain enabled. Obtain the persistent Ed25519 server
fingerprint independently from the operator before accepting it. The automated
runner accepts discovery's fingerprint only inside this controlled deployment;
that is not an out-of-band identity-verification workflow.

## Isolated deployment

Inspect `docker compose ls`, `docker ps` and `ss -ltn` on `.54` first. Stop if port
5848 belongs to anything other than the established milestone Caddy. Transfer a
source archive excluding `.git`, `target`, `node_modules`, generated bundles and
all credentials/state, then:

```sh
mkdir -p /home/william/cords-milestone
tar -xzf /home/william/cords-milestone-source.tar.gz -C /home/william/cords-milestone
cd /home/william/cords-milestone
```

Generate credentials once, outside Git. This refuses to replace an existing
`.env`; subsequent starts must reuse the existing credentials.

```sh
python3 - <<'PY'
import os, secrets
password = secrets.token_urlsafe(48)
descriptor = os.open('.env', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(descriptor, 'w') as output:
    output.write('CORDS_POSTGRES_PASSWORD=' + password + '\n')
    output.write('CORDS_DATABASE__URL=postgres://cords:' + password + '@postgres:5432/cords\n')
PY
docker compose --env-file .env -f deploy/compose.milestone.yaml up --build -d --wait
docker compose --env-file .env -f deploy/compose.milestone.yaml ps
docker compose --env-file .env -f deploy/compose.milestone.yaml cp caddy:/data/caddy/pki/authorities/local/root.crt /home/william/cords-milestone-ca.crt
```

Copy the public CA certificate over authenticated SSH to
`target/cords-milestone-ca.crt` locally. Do not copy its private key. Caddy's
`default_sni` supports IP-origin clients. Use a publicly trusted certificate for
an Internet-facing deployment.

Authentication defaults are `challenge_seconds = 60` and `session_seconds = 900`
in `[authentication]`. Server environment overrides are
`CORDS_AUTHENTICATION__CHALLENGE_SECONDS` and
`CORDS_AUTHENTICATION__SESSION_SECONDS`. Add overrides explicitly to the Compose
server environment; undeclared `.env` variables are not passed into containers.

## CLI initialization and conversation

From the repository root, run A and B in separate terminals:

```powershell
cargo build -p cords-dev
target/debug/cords-dev.exe --state C:/cords-test/a --migrations migrations/sqlite --ca target/cords-milestone-ca.crt
# In another terminal:
target/debug/cords-dev.exe --state C:/cords-test/b --migrations migrations/sqlite --ca target/cords-milestone-ca.crt
```

Never copy A's database/unlock material to B. An installation lock prevents
concurrent cryptographic mutation. Without a passphrase flag, initialization
stores a random master key in the OS credential store. If unavailable, use
`--passphrase-stdin`: the first stdin line is the passphrase, at least 12 bytes.
Do not put it in arguments, scripts, shell history or logs. Subsequent stdin lines
are JSON commands. Match response `id` values; asynchronous notifications share
stdout.

On each client, provide only the HTTPS origin. The client verifies signed discovery over
certificate-validated HTTPS, pins the discovered server identity on first use, and rejects a
different identity on later connections:

```json
{"id":1,"op":"trust","origin":"https://192.168.86.54:5848"}
{"id":2,"op":"authenticate"}
{"id":3,"op":"publish"}
{"id":4,"op":"status"}
```

A creates a channel, then approves B's device ID:

```json
{"id":5,"op":"create","name":"development conversation"}
{"id":6,"op":"add","route":"CHANNEL_ID","device":"B_DEVICE_ID"}
```

B processes its recipient-specific Welcome. Both clients then connect and send:

```json
{"id":7,"op":"join","route":"CHANNEL_ID"}
{"id":8,"op":"connect"}
{"id":9,"op":"send","route":"CHANNEL_ID","body":"Hello through MLS"}
{"id":10,"op":"history","route":"CHANNEL_ID"}
```

`connect` subscribes to authenticated WSS before HTTP catch-up. `disconnect` stops
WSS and polling; `sync` catches up explicitly; `retry` resubmits the exact last
application upload. `members` and `channels` list public server state. `remove`
with `route`/`device` denies delivery and submits an MLS removal. `revoke`
permanently revokes the current device using its root. `pending-removals` with
`route` returns root-authenticated removals awaiting the creator's `remove` call.
The creator must be available for this slice's management operations.

Restart with the same state directory, credential store/passphrase and CA.
Unprocessable history fails visibly. Never delete state to hide a synchronization
failure or restore an older MLS snapshot into an installation that has continued
sending or receiving.

## Real-server acceptance

These runners intentionally restart only the isolated milestone server/database,
preserve volumes, and exit nonzero on failed assertions. Run them sequentially.

```powershell
cargo build -p cords-dev --features fault-injection
python scripts/encrypted-acceptance.py --client target/debug/cords-dev.exe --ca target/cords-milestone-ca.crt --crash-boundaries --report target/encrypted-acceptance-crash.json
python scripts/lifecycle-acceptance.py --client target/debug/cords-dev.exe --ca target/cords-milestone-ca.crt --report target/lifecycle-acceptance.json
```

The first runner checks independent identities, live bidirectional MLS, exact
retries, offline catch-up, client termination/restart, server kill/restart,
PostgreSQL restart, pending-commit recovery, durable outbox recovery and lost
acknowledgements. Server fingerprints must match across restarts. Each marker
must occur exactly once in each client's decrypted history; protocol and
application events both count toward contiguous route cursors.

The lifecycle runner uses four independent installations, offline processing
across multiple epochs, removal, revocation and a queued MLS removal. Both scan
a consistent logical `pg_dump` for raw UTF-8, hex, base64/base64url with/without
padding, JSON Unicode escapes, SQL quote escaping, octal/byte escapes and doubled
backslashes. Reports contain counts and public evidence, never unlock keys.
Runner passphrases exist only in memory; its leftover encrypted test databases
cannot be reopened after exit without retained unlock material. Manual
installations should use deliberately retained unlock material or the OS store.

## State, restart and backup

Compose owns `postgres-data`, `server-data` and `caddy-data` volumes under project
`cords-milestone`. The server key is in `/var/lib/cords`. PostgreSQL holds public
identities, memberships, sessions, coordination, ciphertext and idempotency
results. `STATE/client.db` holds protected client state, ciphertext and individually
AEAD-protected message-cache records. OS master keys use credential service
`Cords` and the installation identifier. Passphrase mode stores the salt and
authenticated wrapped master key in SQLite.

```sh
cd /home/william/cords-milestone
docker compose --env-file .env -f deploy/compose.milestone.yaml restart cords-server
docker compose --env-file .env -f deploy/compose.milestone.yaml restart postgres
docker compose --env-file .env -f deploy/compose.milestone.yaml up -d --wait
```

Before upgrading, preserve a private dump and matching signing-key state:

```sh
set -eu
umask 077
stamp=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p backups
docker compose --env-file .env -f deploy/compose.milestone.yaml exec -T postgres pg_dump -U cords -d cords -Fc > backups/pre-upgrade-$stamp.dump
docker compose --env-file .env -f deploy/compose.milestone.yaml cp cords-server:/var/lib/cords backups/server-state-$stamp
test -s backups/pre-upgrade-$stamp.dump
docker compose --env-file .env -f deploy/compose.milestone.yaml up --build -d --wait
```

Do not run `down --volumes`. Restore is an explicit operator operation using a
matched key/database backup into an isolated deployment first. Never overwrite a
running deployment or automatically roll back client MLS state. Missing signing
keys against initialized database state are fatal. Epoch snapshots added in
migration 4 cannot reconstruct missing older rosters.

## Minimal desktop

For the tested portable Windows release, run from the repository root:

```powershell
./build-scripts/windows-client.ps1
Get-FileHash dist/cords-client-windows-x86_64-0.1.0.zip -Algorithm SHA256
Get-Content dist/cords-client-windows-x86_64-0.1.0.zip.sha256
Expand-Archive dist/cords-client-windows-x86_64-0.1.0.zip C:/cords-test/package
$env:CORDS_CLIENT_STATE='C:/cords-test/desktop'
$env:CORDS_CLIENT_CA=(Resolve-Path target/cords-milestone-ca.crt).Path
& C:/cords-test/package/cords-client-windows-x86_64/Cords.exe
```

Keep the packaged `migrations/sqlite` directory beside `Cords.exe`. No migration
environment override or source checkout is needed at runtime. Extract into a new
directory for upgrades; retain the existing installation state and OS credential
store. WebView2 must be installed. The ZIP is portable, not an installer.

On `.54`, after the backup and port checks above, build and select the exact OCI
script output:

```sh
cd /home/william/cords-milestone
bash build-scripts/linux-server-oci-img.sh
# Set CORDS_MILESTONE_IMAGE=cords-server:0.1.0 in the existing private .env.
docker compose --env-file .env -f deploy/compose.milestone.yaml up --no-build -d --wait
```

For a local debug build instead:

```powershell
cd bins/cords-client/ui
npm ci
npm run build
cd ../../..
cargo build -p cords-client --features custom-protocol
$env:CORDS_CLIENT_STATE='C:/cords-test/desktop'
$env:CORDS_CLIENT_CA=(Resolve-Path target/cords-milestone-ca.crt).Path
target/debug/cords-client.exe
```

Unlock, enter the HTTPS origin, then **Trust and authenticate**. Inspect the pinned fingerprint
after connecting if independent verification is required for the deployment.
Publish a KeyPackage before another member adds this device. Refresh channels
and select one to process its Welcome. Use the composer for real messages.
Connection/trust failures are visible. The previous discovery demonstration is
preserved in `Discovery.svelte`; it is not the conversation data source.

For a real CLI peer to the desktop device:

```powershell
python scripts/desktop-peer.py --client target/debug/cords-dev.exe --ca target/cords-milestone-ca.crt --device DESKTOP_DEVICE_ID
```

Select its new `desktop-smoke` channel, send `desktop encrypted smoke`, and verify
`CLI reply to desktop` appears. The peer writes `target/desktop-smoke.json`.
UI polish, full enrollment/recovery, DMs, attachments, calls, reactions and
federation remain deferred.
