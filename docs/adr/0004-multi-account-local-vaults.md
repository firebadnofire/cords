# ADR 0004: Multi-account local vaults and native locking

**Status:** Accepted  
**Date:** October 10, 2026

## Context

The original desktop milestone treated one application data directory as one installation. Its
single SQLite database contained one account root, one device identity, MLS state, encrypted
history, server trust, and a bearer session. A random local master key was either wrapped by an
Argon2id passphrase or stored directly in the operating-system credential store. The WebView
displayed an installation unlock form and could initialize a new identity through that same path.

That model cannot safely support several local identities: a cosmetic picker would still share one
native client, one connection, and one storage key. It also allows a passwordless OS-store path to
make a user-chosen local password irrelevant.

## Decision

The desktop owns a non-secret device registry and one independent vault directory per local
account. The registry contains only the stable account identifier needed to select a vault, a
random local directory identifier, cached nickname/avatar presentation, privacy flags, and
auto-lock policy. It never contains a vault master key, identity secret, MLS state, server bearer
token, or decrypted message.

Every new vault has its own random 256-bit master key, independent Argon2id salt, account root,
device identity, MLS state, SQLite database, and file lock. The existing Cords Argon2id parameters
(64 MiB memory, three iterations, one lane, version 1.3) derive a wrapping key. XChaCha20-Poly1305
wraps the master key using the vault identifier as associated-data context. A password of at least
12 Unicode scalar values is mandatory for new desktop accounts. Spaces, Unicode, and long
passphrases remain valid. Local weak-password assessment may require an explicit warning override;
the password is never sent or persisted.

Operating-system or hardware protection may later wrap an additional factor or protect the
password-wrapped result, but it must not silently bypass or replace the mandatory account password.
Legacy OS-credential-store vaults remain readable only for explicit migration.

The native runtime may hold at most one unlocked `Client`. Lock and switch drop that client and its
identity-bound network task. Sign out first removes the persisted server bearer session and then
drops the client. Removing an account requires its local password, closes its SQLite pool, and
deletes only that registered local vault. None of these operations creates a device revocation or
root-key transition.

Inactivity and detected suspend gaps are evaluated by a native watchdog. WebView user-input
signals may refresh the activity timestamp; synchronization, socket traffic, and other background
work do not. Locking disconnects identity-bound network activity. A reliable operating-system
session-lock event remains platform integration work; the policy is stored default-on but must not
be represented as enforced on a platform until its native event path is validated.

Locked-screen avatars are cached local PNG data. The picker does not fetch remote content.
Genericize Mode suppresses the cached image, and optional nickname hiding substitutes an ordered
local label. Explicit HTTPS image import rejects credentials, redirects, private/local/special-use
destinations, oversized responses, and unsupported content types before the established inert
decode/rasterization pipeline stores the bounded PNG.

## Migration

When the legacy `client.db` exists, migration requires its current unlock material and a new
mandatory account password. Cords opens the source, records its account/device identifiers, closes
it, copies the database into a fresh account directory, rewraps the same master key, and reopens
the copy with the new password. Registration occurs only after the account and device identifiers
match. The source database is not deleted automatically and remains rollback material.

## Consequences

- Unlocking one account cannot derive or obtain another vault master key.
- Account switching does not require application restart and does not carry an active native
  session or connection across identities.
- Lock-screen metadata is intentionally not secret; users can suppress its identifying portions.
- Losing a local password can make that local vault unrecoverable. It does not destroy the portable
  identity if separately authorized devices or recovery material exist.
- OS credential-store supplementation and reliable OS-session-lock integration require separate,
  platform-specific validation.
