# Cords

*Right on the wire*

Cords is a self-hosted communication system designed to keep message and attachment plaintext on authorized client devices. The working tree now implements a development encrypted-channel slice: independent native identities, server pinning, device authentication, MLS, durable HTTP synchronization and WebSocket notifications.

New server operators should start with the [Cords server operator guide](docs/op-guide.md), which
covers OCI deployment with Docker or Podman and direct installation from the unpacked server
tarball.

The CLI and minimal Tauri conversation UI use the same Rust client core. See the
[encrypted milestone guide](docs/encrypted-milestone.md) and
[validation report](docs/reports/encrypted-milestone.md) for reproducible commands,
real `.54` evidence and remaining gates. Attachments, calls, DMs, full enrollment
and recovery remain deferred; this is not a production release claim.

## Repository

- `bins/cords-client`: Tauri v2 desktop client and Svelte/TypeScript UI.
- `bins/cords-server`: Axum server executable and configuration.
- `crates/cords-protocol`: public schemas and deterministic signed encoding.
- `bins/cords-dev`: development CLI around the real client core.
- `crates/cords-client-core`: protected state, identity, trust, MLS orchestration and synchronization.
- `crates/cords-server-core`: persistent server identity and HTTP services.
- `crates/cords-storage`: PostgreSQL and SQLite lifecycle adapters.
- `docs/adr`: architecture decisions; `AGENTS.md` is the normative product specification.

## Run the server

```sh
docker compose up --build --wait
curl -i http://127.0.0.1:4849/health/ready
```

The base Compose topology exposes internal HTTP on loopback port 4849 for health diagnostics. It is not a client origin. Run the optional Caddy profile for local HTTPS:

```sh
docker compose --profile proxy up --build --wait
```

Caddy uses its internal CA for this local example. Trust that CA explicitly before inspecting `https://localhost:4848`; Cords never disables normal TLS validation. Production deployments should use a publicly trusted certificate and set `CORDS_SERVER__PUBLIC_ORIGIN` to the matching HTTPS origin.

The Compose file uses a conspicuous development-only PostgreSQL password. A deployment must apply `deploy/compose.production.example.yaml` as an override and provide a matching, URL-encoded `CORDS_DATABASE__URL`; do not reuse the development credential publicly.

Clients join with the HTTPS URL only. Cords validates the certificate, verifies signed discovery,
pins the discovered server identity on first use, and refuses later identity changes for that
origin. The pinned fingerprint remains visible for inspection; it is not a join form field.

To bind the first server owner, open **Server administration → Ownership** in an authenticated
desktop client and generate a one-time claim code. Configure that exact value temporarily as
`CORDS_AUTHENTICATION__OWNER_CLAIM_CODE`, restart the server, and redeem it from the same client.
For Compose, set it only for the claim restart:

```sh
CORDS_AUTHENTICATION__OWNER_CLAIM_CODE='<client-generated-code>' docker compose --profile proxy up -d cords-server caddy
```

The authenticated claim atomically binds ownership to the Cords account. Remove the environment
value after success and restart normally. A used code cannot transfer ownership, codes shorter
than 32 characters are rejected, and Cords never logs the configured value.

## Run the desktop client

Install Node.js 24 and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/), then:

```sh
cd bins/cords-client/ui
npm ci
npm run tauri:dev
```

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
cd bins/cords-client/ui
npm run format:check
npm run check
npm run lint
npm test
```

## Releases and containers

Forgejo Actions builds signed client packages for Linux, Windows, and macOS,
Linux server tarballs for x86_64 and ARM64, and a Cosign-signed
multi-architecture server image. Forgejo is canonical; only after its release
succeeds does the workflow mirror source, release files, and the container to
`firebadnofire/cords` on GitHub. See
`.forgejo/docs/release-workflow.md` for runner assignments, required secrets,
artifact names, signature verification, and the hosted-validation boundary.

The public GitHub container is pulled with:

```sh
docker pull ghcr.io/firebadnofire/cords
```

Local native and cross-platform release builders are documented in
[`build-scripts/README.md`](build-scripts/README.md). They produce client and
server file artifacts under `dist/`, plus a locally loaded server container
image, without publishing anything.

## Validation boundary

Unit tests and Compose readiness alone do not prove encrypted messaging. The
real-server acceptance runner supplies that evidence. A Tauri build does not
substitute for native interaction testing, and neither establishes cross-platform
signing, a published release, or complete production identity UX.
