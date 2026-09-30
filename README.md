# Cords

*Right on the wire*

Cords is a self-hosted communication system designed to keep message and attachment plaintext on authorized client devices. This repository currently implements **Phase 0 only**: a Tauri v2/Svelte desktop shell, signed server discovery, protocol negotiation, storage adapters, and Docker-first server startup.

The conversation timeline is deliberately fixture-backed and carries a persistent **Local UI demo** label. Cords does not yet implement accounts, membership, server-key pinning, messaging, MLS, attachments, or calls.

## Repository

- `bins/cords-client`: Tauri v2 desktop client and Svelte/TypeScript UI.
- `bins/cords-server`: Axum server executable and configuration.
- `crates/cords-protocol`: public schemas and deterministic signed encoding.
- `crates/cords-client-core`: TLS discovery and UI-facing view models.
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

Forgejo Actions builds signed client packages for Linux, Windows, and macOS and
publishes a Cosign-signed multi-architecture server image. Forgejo is canonical;
the workflow mirrors source, release files, and the container to
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

Unit tests prove protocol and state invariants. Compose readiness proves that the container, PostgreSQL migration, persistent identity, and HTTP process start together. A successful Tauri build proves packaging on that build platform. None of these claims prove encrypted messaging, identity enrollment, a data plane, cross-platform signing, or a published release.
