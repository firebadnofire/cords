# ADR 0001: Deterministic CBOR profile for Phase 0 metadata

- Status: Accepted
- Date: 2026-08-05

## Context

The discovery response is JSON, but its signature must cover a deterministic CBOR representation. Map-key sorting, optional fields, and generic serializers can otherwise create ambiguous bytes.

## Decision

`cords-protocol` uses `minicbor` only as an encoding primitive. `ServerMetadataV1` is encoded as a definite-length nine-element CBOR array in protocol field order, with definite-length string arrays and no optional values. The bytes are prefixed with `CORDS-SERVER-METADATA-V1` before Ed25519 signing. JSON field order is not signed.

Any field addition requires a new version and stable test vector. General application code must call the protocol type's encoder rather than constructing signing bytes.

## Consequences

The encoding is compact, deterministic, and independent of JSON formatting. Array positions are not self-describing, so versioning and protocol-owned tests are mandatory. This ADR selects only Phase 0 metadata encoding; it does not preselect encoding details for later identity or MLS objects.

