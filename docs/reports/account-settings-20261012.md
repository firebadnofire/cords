# Account settings and roster diagnostics — 2026-10-12

## Plan

See [the focused implementation plan](../plans/account-settings-and-roles.md).
C0 is complete; this user-requested work does not activate the full rework milestone
or privileged publication. Roles remain a separately gated follow-up. Automatic
role-based encrypted access was explicitly approved: verified devices are queued,
not given encryption keys by changing an ACL.

## Implementation

- Settings → Security changes the local unlock password. Native code checks the
  expected active account, current password and existing master key; uses the
  existing Argon2id parameters and authenticated wrapping context with a fresh salt;
  and conditionally updates the single installation row atomically. Identity,
  encrypted content, MLS state, trust and pending operations are untouched.
- The UI checks confirmation, keeps the existing explicit weak-password warning,
  shows native failures, and clears password fields after a completed attempt.
  Rust owns temporary passwords in zeroizing buffers. Passwords are not preferences.
- Retained channel archives are reached through Settings → Local history. Archived
  server bookmarks are described separately; no complete remote history is promised.
- Connection errors wrap rather than disappear behind an ellipsis. Background
  errors preserve their full chain, including HTTP status and recognized error code.

No server/protocol schema change, personal-vault mutation, deployed-service change,
server purge, owner-secret change or publication activation was performed.

## Validation

Environment: Windows x64; rustc 1.97.1 (8bab26f4f, 2026-07-14), Node 24.18.0,
npm 11.16.0. Existing locked package versions are preserved. `subtle` 2.6.1 was
already locked and is now a direct client-core dependency for constant-time
comparison of the authenticated unwrapped key with the loaded vault key.

Commands from the repository root unless marked UI:

```powershell
cargo fmt --all
cargo fmt --all --check
cargo test --release -p cords-client-core password_change_is_atomic_and_preserves_account_state -- --nocapture
cargo test --release -p cords-client-core --locked
cargo test --release --workspace --all-features --locked
cargo check --release -p cords-client --locked
cargo clippy --release -p cords-client-core -p cords-client --locked -- -D warnings
cargo deny check
git diff --check

# bins/cords-client/ui
npx prettier --write src/App.svelte src/components/Settings.svelte src/components/SettingsAccount.test.ts src/styles.css
npm run check
npm run lint
npm test
npm run build

# Root; isolated artifact destination, no overwrite of the running personal package
$env:CORDS_DIST_DIR = 'D:/codex-build/cords-settings-20261012'
& ./build-scripts/windows-client.ps1
```

| Gate | Result |
| --- | --- |
| Password storage regression | PASS: incorrect old password, short new password, injected SQL write failure, repeated rewrap, old-password rejection on restart, unchanged storage key/account/device/trust/history/MLS/preferences/outbox, unaffected independent vault |
| Client core | PASS: 13 tests |
| Workspace, all features | PASS: 38 tests; 21 PostgreSQL tests explicitly ignored, **not** counted as passing |
| Frontend | PASS: 39 tests in 12 files; confirmation, failed native command and secret-field clearing, archive navigation and bookmark distinction |
| Diagnostics/lint/build | PASS: zero Svelte errors/warnings, ESLint, production Vite build, native Rust check and warning-free Clippy |
| Dependency security | PASS: advisories/bans/licenses/sources; existing duplicate and unmatched-advisory warnings remain |
| Portable Windows build/startup | PASS: official ZIP built and separately extracted; native empty picker observed, no Vite/mocks or personal state |
| Packaged password change, relock and new-password unlock | PASS: disposable account, wrong-current rejection, successful change, old-password rejection and new-password unlock after lock and process restart; unchanged public account/device IDs |
| Packaged archive navigation | PASS: Settings → Local history → native archive browser opens offline before and after restart; empty fixture only, no retained-body acceptance claim |
| Other native platforms / installer acceptance | NOT RUN for this change |

Initial failed checks were diagnosed before proceeding: the new UI fixture used
`departure_confirmed` instead of the actual `remote_drop_confirmed` field; fixed.
The first UI run produced three jsdom `showModal is not a function` errors despite
passing assertions; this run is **not** a pass. A test-only dialog presentation stub
resolved the missing jsdom API; the subsequent full run had no unhandled errors.
Native dialog behavior still requires the packaged check.

Final native package (rebuilt after adding expected-account binding):

| Artifact | SHA-256 |
| --- | --- |
| `D:/codex-build/cords-settings-20261012/cords-client-windows-x86_64-0.1.0.zip` | `3C842A1851D19E26743C4E8F0B457DFCC700C21CEC0CF91C0A820080CDDC3506` |
| Extracted `Cords.exe` | `834B7C2FA0A4DCCFABE48B48B1E242A989EA5EF8BD46EC945425F4F51FFE709E` |

Separate installation:
`D:/codex-build/cords-settings-20261012/installation-430275602e6b48f5ae02bb6cc312cd6a/cords-client-windows-x86_64`;
isolated `CORDS_CLIENT_STATE` is the sibling `test-vault` directory. The first
empty test window was closed normally before replacing its test-only payload;
the final native package was tested with explicitly authorized disposable credentials
and closed normally after acceptance.
No private material is included in this report or the ZIP. The original app stayed open.

### Packaged Windows acceptance

The user explicitly authorized disposable test credentials for this isolated
fixture. Neither personal-account authentication nor the GPG prompt was automated.
Interactions used the actual packaged WebView and native commands through Windows
computer control, without Vite, injected UI code or mocked native commands.

1. Created and unlocked `Settings test` in the empty account picker. Closed the
   connection dialog without trusting or authenticating to any server.
2. Opened Settings → Local history. Observed separate retained-channel and archived
   server-bookmark descriptions. The archive browser opened offline and displayed
   `No retired channel history retained.` Closing it returned to Settings.
3. Submitted a deliberately incorrect current password in Settings → Security.
   The native command rejected it with `current password was not accepted: local
   protected data could not be authenticated or unlocked`. All three fields cleared.
4. Submitted the correct current password and matching replacement/confirmation.
   Observed the success notice and three cleared fields.
5. Used Lock Now. The previous password failed with `Account could not be opened`
   and `local protected data could not be authenticated or unlocked`; the replacement
   unlocked the fixture.
6. Closed the package normally, then relaunched the same executable with the same
   isolated state directory. Repeated old-password rejection and replacement-password
   unlock. Settings showed the same account fingerprint and device identity:
   `1GWXP2_ZKEuPWvycygxhdeB7srdyFVOzgpi65zB9hkA` and
   `01a131cd-a0af-7631-b2c6-c4cef9306588` (generation 1).
7. Reopened the archive browser offline from Settings after restart, observed its
   empty state, and closed the fixture normally. Its vault is retained outside Git.

Restart command (from repository root; temporary environment restored afterward):

```powershell
$settingsInstall = 'D:\codex-build\cords-settings-20261012\installation-430275602e6b48f5ae02bb6cc312cd6a'
$settingsExe = Join-Path $settingsInstall 'cords-client-windows-x86_64\Cords.exe'
$settingsVault = Join-Path $settingsInstall 'test-vault'
$settingsPreviousState = $env:CORDS_CLIENT_STATE
try {
    $env:CORDS_CLIENT_STATE = $settingsVault
    Start-Process -FilePath $settingsExe -WorkingDirectory (Split-Path $settingsExe)
} finally {
    $env:CORDS_CLIENT_STATE = $settingsPreviousState
}
```

This fixture had no server membership, received history or attachment objects.
It verifies native password rewrapping, unlock persistence, identity preservation
and archive navigation/empty state. It does not establish fresh two-client TLS
messaging, populated offline-archive viewing or other-platform acceptance.

The 21 PostgreSQL regressions were not rerun for this local-only unit; server and
PostgreSQL code/schema are unchanged. Roles/server units require those fresh gates.
No controlled power-loss test was performed. Changing a password does not revoke
copies of the storage key or alter old backups, snapshots or journal copies.

## Live roster observation

The original running personal package is under `dist/cords-client-windows-x86_64-0.1.0`.
After the user unlocked FireBADnoFire, native accessibility inspection showed a
connected account, the public channel `testp`, and encrypted `teste` disabled with
“Visible server channel; waiting to be added to its encrypted conversation.” There
was no failure banner at that observation. This establishes current non-enrollment,
not the cause of the earlier request failure.

Source audit finds multiple distinct refusals on `roster-requests`: unauthorized
manager/route access, an already enrolled target, and unavailable unreserved
KeyPackages. The latter two currently share `CORDS_STATE_CONFLICT`; idempotency
conflicts also use that code. An ambiguous pending operation therefore must not be
blindly discarded. No retry-state or personal-vault cleanup was applied on a guessed
diagnosis. The full earlier status/code or a controlled reproduction is still needed.

## Rollback and next gates

No new storage format is introduced. Rewrap forward with the current password to
change protection again; do not restore an old live cryptographic snapshot.
Navigation/diagnostic changes can revert independently. Keep role implementation
and deployment inactive until the documented acceptance gates pass. Publication
remains disabled. The test installation is retained outside Git; it is not a release.

Git delivery: changes are staged on local `main`. The required GPG-signed commit
attempt reached local pinentry and then failed with `gpg: signing failed: Timeout`;
no commit was created and no push was performed. The user subsequently deferred
signing; no retry or unsigned commit was attempted. Signing was not disabled or
bypassed. Local `main` contains the staged implementation; upstream is unchanged.
