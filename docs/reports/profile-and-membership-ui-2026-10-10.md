# Profile and membership UI fixes — 2026-10-10

## Plan

Trace the production profile, locked-picker, channel membership and departure paths;
fix their existing integration; validate focused regressions while preserving the
current Svelte/Tauri layout and native security boundaries.

## Implementation

- Other devices' messages previously rendered only a device ID and no profile
  lookup. Conversations and member lists now use native-verified signed member
  cards for nicknames and pictures. Device IDs remain available for disambiguation;
  nicknames are self-declared labels rather than verified global names.
- Saving a nickname/avatar invalidates cached sessions for device-proof renewal,
  which publishes the updated signed card through the existing authentication path.
  Local preference saves still work offline. Verified directory cards are sealed in
  the vault, scoped by server, refreshed during synchronization at a 30-second
  interval, and removed with local server data. Unlock/server selection also fetch
  the directory. This does not change account/device authorization or MLS rosters.
- Remote avatars preserve normalized crop/shape and fall back to initials with an
  unavailable hint. Changed/cleared sources cannot retain a prior resolved picture.
  Up to 16 image requests/results are shared across message rows, and the cache is
  cleared on account lock/switch. Existing native HTTPS loading and bounded safe
  rasterization remain in place; arbitrary remote images do not bypass CSP.
- Hidden locked nicknames use a numbered account label with a neutral icon.
  Genericize hides locked-picker pictures. Both settings explicitly describe their
  local-display scope and leave unlocked/shared profiles unchanged. Picker labels
  are registry presentation metadata; hiding is not a claim of encrypted registry
  metadata or anonymization.
- Channel details offer server-member selection by nickname and device suffix,
  display the exact selected device, and explain server admission, KeyPackage
  publication and future-only MLS history. New channels open with details visible.
  Public channels explain existing admitted-member access, require no MLS invitation,
  omit an MLS epoch badge and use a globe instead of a membership-lock icon.
- Leave server now offers a separate confirmed local-only action immediately,
  without a successful network call. The departure-failure fallback also handles
  HTTP 502 and other HTTP errors. It explicitly says remote removal was not verified;
  retained channel archives survive removal of active caches and MLS state.
- Removed sibling UI reference requirements from project Markdown. The production
  UI integration document now describes direct maintenance of the existing layout.

## Validation

- Client-core: **12 tests passed**, including sealed profile persistence, server
  isolation and local removal/restart, plus existing account/MLS/cache regressions.
- UI: **36 tests passed across 11 files**. New regressions cover another account's
  nickname and cropped picture, named member additions, neutral hidden-name icons,
  and separately confirmed local removal after an HTTP 502 departure failure.
- Svelte/TypeScript check, ESLint and production Vite build passed.
- Rust check and Clippy for client-core/Tauri, all targets/features with warnings
  treated as errors, passed. Rust formatting and Git whitespace checks passed.
- A project Markdown search found no remaining sibling UI name references.

The frontend dependency directory was incomplete; `npm ci` restored the existing
lockfile dependencies without changing the dependency manifest/lockfile. Its audit
reported zero vulnerabilities. No production server, account vault or database was
modified, and no deployment or installer replacement was performed.

## Evidence limits

Automatic approval review rejected creation of a fresh disposable PostgreSQL test
service with only “blocked by policy.” A new real two-client profile-publication
run therefore remains unverified. Earlier public-channel TLS acceptance is prior
evidence and is not presented as validation of these new profile changes.

Packaged desktop interaction and live FireBADnoFire/Arbiter rendering remain
unverified. Frontend regressions mock the narrow Tauri boundary; local client-core
tests use real encrypted SQLite but do not prove server publication. No claim is
made that HTTP 502 availability itself was repaired; local removal now works
independently of that server response.
