# Preinitialized role catalog — 2026-10-14

## Plan

Install `$default`, `$moderator`, and `$admin` as persistent server defaults.
Preserve existing definitions, remove manual initialization from the editor, and
validate migrations and the packaged Windows UI without activating RBAC.

## Implementation

Schema 14 adds `202610140001_role_catalog_defaults.sql`. It seeds only an untouched
empty schema-13 catalog, retaining revision zero as the installation baseline.
It creates no synthetic user audit, account assignment, or MLS membership. Existing
catalogs and their revisions/audit/receipts are preserved. An inconsistent catalog
missing built-ins fails migration with an actionable error. The existing database
constraint still prevents role enforcement activation.

The Roles screen opens the preinitialized catalog. Empty legacy servers receive an
operator-upgrade message, not a setup button or a client-generated default catalog.
An explicit legacy `initialize` request can acknowledge the untouched baseline;
it cannot reset edited policy. Signing domains and existing message contracts are
unchanged. The plan documents schema compatibility and repair-forward boundaries.

## Validation

```powershell
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54
cargo clippy --locked -p cords-server-core --all-targets -- -D warnings
# In bins/cords-client/ui:
npm test
npm run check
npm run lint
npm run build
# From repository root:
$env:CORDS_DIST_DIR='D:/codex-build/cords-role-defaults-20261014'
./build-scripts/windows-client.ps1
pwsh -NoProfile -File scripts/test-role-policy.ps1 -SshHost william@192.168.86.54 -ForwardPort 58202 -DesktopFixtureDirectory D:/codex-build/cords-role-defaults-gui-20261014 -DefaultsOnly
```

- PostgreSQL 17.10 on `.54`, isolated loopback Docker fixture through verified SSH:
  **39 server tests passed, zero failed**, 66.01 seconds. Tests cover initial roles
  over verified TLS, fresh migration, existing customized schema-13 upgrade,
  interrupted migration rollback, repeated startup, and first edit without setup.
  Seeded definitions are compared against the Rust policy defaults.
- **78 frontend tests passed** across 18 files, including two new editor cases.
  Svelte check: zero errors/warnings; ESLint and Vite production build passed.
- Strict Clippy passed. The desktop fixture initially exceeded the function-length
  lint after adding the baseline check; extracting its verification helper fixed
  that diagnostic. No lint suppression was added.
- Fresh Windows x86_64 portable package 0.1.0 built. Using a separate fixture vault,
  the app unlocked, authenticated over verified TLS, and displayed all three
  built-ins at revision zero. Create role was enabled; no initialization control
  appeared. The read-only fixture verified three persisted roles and zero user
  policy changes, passing in 85.49 seconds. This database check supplements the
  observed UI; it is not a substitute for it.

ZIP SHA-256:
`3ab4a88275471a27d45694661d9ee89853e6bede4eb35155561539d168f5ca24`

Packaged executable SHA-256:
`539cfa0aa9c8c32d24e5a34960fdcea6132fbaee738f66a437d8c54e0dc6726a`

Evidence logs:

- `D:/codex-build/cords-role-defaults-20261014.log`
- `D:/codex-build/cords-role-defaults-build-20261014.log`
- `D:/codex-build/cords-role-defaults-gui-20261014.log`

Fixtures `cords-rbac-5d66d7ed10fa` and `cords-rbac-8b67689bcb75` were removed;
their absence was verified. The disposable desktop app closed normally. Personal
accounts and deployed services were untouched. Concurrent avatar work was preserved;
the package contains the workspace state at build time, not a separate avatar release.

This change is local, not deployed or pushed. It needs the updated server migration
and client to reach existing installations. No new macOS/Linux packaged GUI evidence
is claimed. Channel/category enforcement and the broader provisioning milestone
remain incomplete and inactive.
