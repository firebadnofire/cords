# Phase 0 protocol

Phase 0 implements protocol version `1` for signed server discovery only.

- `GET /.well-known/cords/server` returns `ServerMetadataV1` fields plus `signature`.
- `GET /api/v1/capabilities` advertises protocol range `1..=1` and an empty feature list.
- `GET /api/v1/servers/self` returns the same signed metadata.
- `GET /health/live` proves the process is running.
- `GET /health/ready` is exposed only after database schema validation and identity loading complete.

The metadata signature authenticates consistency with the included server key. Normal Web PKI TLS authenticates the origin. Persistent server-key pinning is intentionally deferred to Phase 1 and the client labels the inspected identity as not pinned.

No message, join, identity, MLS, attachment, WebSocket, or RTC capability is advertised in Phase 0.

