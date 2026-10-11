# C0 acceptance record

Status: **C0 acceptance incomplete; C1 and later checkpoints inactive.**
Dependency remediation and fresh Windows portable-package acceptance passed on
2026-10-11. Fresh native Linux packaged GUI acceptance also passed in the
documented Debian 12 runtime on the openSUSE host. Native macOS packaged acceptance
is pending; SSH access through the Linux jump host is now verified. Missing macOS
evidence is not waived by Windows/Linux results.

Source baseline: `2c4febab7594b252214c1179c81a5f6775b70d3d`, plus the C0 working
changes below. Initial contracts/tests: 2026-10-10. Remediation and fresh packaged
execution: 2026-10-11. No C1 changes were made.

## Plan

The [implementation plan](../rework-plan.md) retains all eight work areas and every
lettered checkpoint. [ADR 0007](../adr/0007-rework-security-contracts.md) records:

- Root reconciliation cannot establish the legitimate holder of a compromised root.
  Terminal burning remains the compromise response, with distribution limitations.
- A compromised/cloned report authority can equivocate. Detect conflicting evidence
  and fail closed; committed revocation does not establish global awareness.
- Archive deletion removes application-accessible content/material without a
  forensic-erasure guarantee across backups, snapshots, journals or physical storage.

ADR 0005's exceptional burned-root readmission is superseded at C2's terminal-burn
activation gate. Succession timing, bootstrap closure and operator recovery remain.
Current runtime still follows ADR 0005; C0 does not activate C2.

Remediation sequence: investigate provenance → select compatible maintained upstream
→ rerun security/regressions → build fresh official Windows package → execute
independent packaged clients over verified TLS → validate restart/archive/migration
→ record evidence and limits. Failures were diagnosed before dependent work continued.

## Implementation

### Contracts and executable baseline

The [contract audit](../protocol/rework-baseline.md) distinguishes current behavior,
future obligations and compatibility fences. The 27
[semantic scenarios](../../test-vectors/rework-contracts.json) cover root concurrency,
withheld/omitted branches, burning/compromise, report finality/handoff/equivocation,
archives, compatibility and hardware separation. They are specification-only;
future root/report/DM implementations and new byte encodings are not claimed.

Executable additions verify all six existing identity signing vectors, substitution,
version tampering, cross-domain rejection and hash continuity. SQLite fixtures cover
schema 2 → 3, failure after table replacement, transactional rollback, retry/reopening,
opaque bytes/ID preservation and rejection of incompatible older migration sets.
Injected SQL failure is transaction-interruption evidence, not power-loss proof.
Existing client-core regressions separately exercise real sealed state, MLS/cache
atomicity, account migration, archive conflicts and restart.

### RUSTSEC-2026-0173

[RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173.html) identifies
`proc-macro-error2` as **unmaintained**, not a known exploitable vulnerability, and
lists no patched release of that crate. Before remediation:

```text
cords client/server/core → cords-crypto → openmls_rust_crypto 0.6.0
→ hpke-rs 0.7.0 → libcrux-sha3 0.0.10
→ hax-lib 0.3.7 → hax-lib-macros 0.3.7
→ proc-macro-error2 2.0.1 → proc-macro-error-attr2 2.0.0
```

The hax dependency is conditional on `cfg(hax)` in the libcrux chain; the all-target
dependency-policy audit includes it. The normal Windows package does not enable
that verification configuration.

Maintained upstream [hax PR 2039](https://github.com/cryspen/hax/pull/2039), merged
2026-06-09, removes the dependency using `syn::Error`/compile-error diagnostics.
At its reviewed commit the compatible macros still have version 0.3.7. That fix is
not available as a compatible crates.io 0.3.x release. The latest hax release
inspected was 0.4.2; changing the MLS/provider chain to accept it is a larger upgrade.

`Cargo.toml` patches only `hax-lib-macros` to maintained upstream, full commit
`d9c15b6e31cd62e781aa02c360c2007cc0f4f40e`. Its path dependency also resolves
`hax-lib-macros-types` 0.3.7 from that commit. Both proc-macro-error2 packages are
removed from `Cargo.lock`. Registry versus pinned `src/lib.rs`/`src/dummy.rs` bytes
match; the ordinary macro path remains unchanged.

Compared source hashes: `src/lib.rs`
`20E4ABDA602341CE6A7C2487F613168A9D87F761A7169BD6EAFC448C3F751343`;
`src/dummy.rs`
`6F1D02497342310F83935A8079AE7AA15A5E39E5B8723C320F175E3832BFB5E1`.

`deny.toml` allows this specific upstream Git source and requires `rev` pins.
Unknown Git/registry sources remain denied. **No advisory ignore was added.**
Existing unrelated Tauri/Linux exceptions remain unchanged. Remove this temporary
upstream pin when a compatible released chain contains the fix, after equivalent
validation. Builds now require the pinned Git source or a local/vendor cache; that
availability requirement is an explicit supply-chain tradeoff.

OpenMLS 0.9.0, its RustCrypto provider 0.6.0, hpke-rs 0.7.0, hax-lib 0.3.7 and
libcrux-sha3 0.0.10 versions remain unchanged. The aggregate C0 lockfile also records
test-vector dependencies and resolves tempfile 3.27.0 to already-present getrandom
0.3.4 within its declared `>=0.3.0, <0.5` range. No runtime command, wire format,
signing domain, cipher suite, production migration or feature activation changed.

## Validation

### Fresh post-change checks

| Check | 2026-10-11 result / boundary |
| --- | --- |
| Contract links / JSON scenarios | Prior C0 pass: 27 unique specification-only scenarios; no future implementation claim |
| Rust format | Pass: `cargo fmt --all -- --check` |
| Clippy, all targets/features, warnings denied | Pass; 14.54 seconds |
| Workspace tests, all features, locked | Pass: 37 passed, 0 failed; 21 database tests ignored in this invocation |
| Real disposable PostgreSQL explicit tests | Pass: 21 passed, 0 failed, 0 ignored; 256.68 seconds |
| Frontend format/check/lint | Pass; Svelte 0 errors, 0 warnings |
| Frontend tests | Pass: 11 files, 36 tests; 13.99 seconds; component/jsdom evidence |
| Production frontend build | Pass: CSS 37.14 kB / 8.62 kB gzip; JS 189.01 kB / 62.04 kB gzip |
| npm high-severity audit | Pass: 0 vulnerabilities reported |
| cargo-deny | Pass: advisories, bans, licenses, sources; existing multiple-version/unmatched-exception warnings remain |
| Fresh official Windows portable ZIP | Pass: build/package; release compilation 42.35 seconds |
| Native server release build | Pass: 36.73 seconds |
| Two independent packaged Windows clients | Pass for the cases below; real native UI/commands, TLS and PostgreSQL |
| Packaged migration / known archive history / reopening | Pass for the isolated fixtures below |
| Linux/macOS packaged desktop | **Not executed; incomplete platform gate** |
| Native compatible-token lifecycle | Unvalidated; no hardware support claim; C0 changes no hardware path |

Initial workspace/frontend attempts exhausted drive C: MSVC `LNK1318`/PDB limit,
LLVM I/O no space and frontend `ENOSPC`. Diagnosis found approximately 21.8 GiB of
generated debug artifacts. `cargo clean -p cords-client-core -p cords-server-core
-p cords-client` reclaimed 9.5 GiB. Reruns used `CARGO_INCREMENTAL=0` and passed.
No source, vault, identity or user data was removed. Initial infrastructure failures
are retained in logs and are not counted as passing attempts.

### Environment and artifacts

Windows 11 Pro for Workstations 10.0.26200 x64; rustc 1.97.1 `8bab26f4f`
(2026-07-14); cargo 1.97.1 `c980f4866` (2026-06-30); cargo-deny 0.20.2;
Node 24.18.0; npm 11.16.0. Tauri Rust 2.11.5, JS API 2.11.1, CLI 2.12.0;
Svelte 5.57.1, TypeScript 5.9.2, Vite 7.3.6, Vitest 5.0.2. Installed WebView2
version directories: 154.0.4258.53/.62. PostgreSQL **17.10** (`postgres:17-bookworm`);
Caddy **2.10.2** (`caddy:2.10-alpine`). Disposable services ran on
`william@192.168.86.54`, with loopback-only listeners/tunnels.

Evidence root outside Git:
`C:/Users/william/AppData/Local/Temp/cords-c0-remediation-81fc93d9c7d0478ba1fe26bf95a1709d`.
It contains disposable vaults/credentials as well as logs. Do not publish
`private-state.json`, bootstrap code, server-private log or vault databases.

| Fresh artifact | SHA-256 |
| --- | --- |
| `artifacts/cords-client-windows-x86_64-0.1.0.zip` | `D63DA46D77E50954AC9D6DAD9981BD18A82F75ECF0A017602E1F2173B27F6493` |
| Both extracted `Cords.exe` binaries | `1C31BA21A6B08ABE71E83D774F10DA56930E57A67E88CE165E24FB1DC15BBA1A` |
| Dedicated test CA PEM file (file hash) | `CE62C0CB19B4B5364E7B8F9FF42258C3D3F85C9BA9154735E55DE8921DBB9187` |

The official ZIP from `build-scripts/windows-client.ps1` includes embedded production
UI/native migrations. Separate extractions in `installation-A`/`installation-B`
used separate `vault-A`/`vault-B`; both accounts/roots/devices were created through
the real packaged UI. Neither client used Vite, mocks, a CLI message bridge or a
migration-directory override. They are independent installations on one Windows
host, not separate physical machines. MSI/NSIS were built on 2026-10-10 but were
**not** rebuilt/executed here. No installer, signing, publication or cross-platform
claim follows from the ZIP. Historical smoke reports are not fresh evidence.

### TLS and public identifiers

Origin: `https://localhost:58202`. Caddy required TLS 1.3 on remote
`127.0.0.1:58202`; native server `127.0.0.1:58201` was reached over loopback plus
an encrypted SSH reverse tunnel. PostgreSQL remote `127.0.0.1:32769` forwarded to
Windows `127.0.0.1:58200`. The dedicated test CA was supplied through
`CORDS_CLIENT_CA`; no global trust store or certificate-validation bypass was used.
Node HTTPS readiness: `TLS=TLSv1.3; authorized=true; HTTP=204`.

Schannel curl initially rejected readiness because revocation status for the private
test CA was unavailable. Validation continued with certificate-validating Node HTTPS
and native Rust clients. Bravo without the supplied CA explicitly refused the peer:
`invalid peer certificate: UnknownIssuer`.

| Identity / route | Value |
| --- | --- |
| Server pin | `LaV9GVxm54IRH7PYe3a6iCBVuBtizORGWmsoust6Sic` |
| Alpha account | `6dnYcVPW-6bv6D4x24leXdlZXTfe0HvvPy6tIZCyDVI` |
| Alpha device | `01a12b07-199d-7282-8a72-af40a1a7813a` |
| Bravo account | `zW2p6fF1T5PVpisiEciRE6rw91CdKHUKuMWvlyNKH_0` |
| Bravo device | `01a12b08-6229-7650-a094-8d74128ed3e3` |
| Encrypted channel | `01a12b0a-15ac-7450-9d96-0644d0ccbeae`, `c0-encrypted`, MLS epoch 1 |
| Public channel | `01a12b0d-d452-75f1-9b42-c245395a3e5d`, `c0-public`, no MLS epoch |

### Packaged interaction results

| Action / expected result | Observed result |
| --- | --- |
| Account creation, signed discovery trust and authentication | Pass; distinct account/device IDs above, same server pin; Alpha claimed initial owner with the disposable server-generated bootstrap code |
| Non-owner admission / least privilege | Pass; Bravo authenticated under explicit public admission; channel creation disabled for Bravo |
| Real KeyPackage, add device and Welcome | Pass; Bravo initially saw encrypted-channel metadata without access, published its KeyPackage, Alpha added its device; both reached epoch 1 |
| Bidirectional encrypted messaging | Pass; `C0 encrypted Alpha 2026-10-11 #1` and `C0 encrypted Bravo reply 2026-10-11 #2` displayed on both |
| Explicit true-public creation / signed plaintext | Pass; permanent-public warning displayed; `C0 public Alpha 2026-10-11 #3` and `C0 public Bravo reply 2026-10-11 #4` displayed on both |
| Offline catch-up in both modes | Pass; while Bravo was closed/disconnected Alpha sent `C0 public while B offline 2026-10-11 #5` and `C0 encrypted while B offline 2026-10-11 #6`; both appeared after Bravo reopened |
| Locked privacy / wrong password | Pass; restart showed local picker only; wrong password rejected protected-data authentication and did not unlock history |
| Certificate failure | Pass; Bravo without fixture CA unlocked locally but refused network access with UnknownIssuer; restoring CA allowed authenticated catch-up |
| Both restarts retain identity/trust/history | Pass; original devices, server pin, route identities and received bodies retained |
| Persisted MLS after restart | Pass; `C0 encrypted Alpha after restart 2026-10-11 #7` received by restarted Bravo at epoch 1 |
| Existing authenticated retirement | Pass; Alpha retired the disposable encrypted channel in administration; Bravo observed retirement, retained four bodies and sending was disabled |
| Archive after restart with server/TLS stopped | Pass; Bravo locally unlocked; Archives opened all four bodies and original identities read-only without server authentication |

PostgreSQL found eight events with eight unique IDs: one `mls.commit`, four
`mls.application`, three `public.signed`. Displayed catch-up histories contained each
expected body once. This is observed restart/catch-up evidence, not exhaustive
duplicate injection; regressions separately exercise replay.

Transcripts: `desktop-B-encrypted-received.txt`, `desktop-A-encrypted-reply.txt`,
`desktop-A-public-roundtrip.txt`, `desktop-B-public-restart-catchup.txt`,
`desktop-B-encrypted-restart-catchup.txt`, `desktop-B-invalid-unlock.txt`,
`desktop-B-unknown-issuer.txt`, `desktop-A-restarted-authenticated.txt`,
`desktop-B-encrypted-post-restart-delivery.txt`, `desktop-B-retired-channel.txt`,
`desktop-B-relay-unavailable.txt`, `desktop-B-offline-archive.txt`.
Public database evidence: `database-event-encodings.txt`, `postgres-version-final.txt`.

### Packaged migration

First, `cords-dev` prepared a disposable single-vault fixture (preparation only).
Packaged migration copied/verified identity, changed its storage password and
authenticated with its original device/pin. Its CLI history returned an empty cache;
that first fixture is **not** evidence of history preservation.

A second offline fixture copied Alpha's closed, consistent `client.db` into
`legacy-history-vault/client.db`, already containing four bodies and the encrypted
archive. No WAL/SHM files were present when copied. Source/copy hashes before opening:
`8D2BA6A77504E6B2A9AC2A42F8E828710EA97671A2DF78F2788054CF7D0E0482`.
Server/TLS stayed stopped throughout this migration; the copied keys transmitted
nothing. This isolated copy is not counted as another independent messaging identity.

**Migrate existing installation → Copy and verify identity** created local picker
label `C0 History Migrated`, preserving Alpha's public profile/device, pin, channel
IDs/confidentiality and all four encrypted archive bodies. Repeated packaged startup
unlocked with the new password. The original legacy file remains present; opening
it can update routine database state, so its post-migration file hash is not claimed
unchanged. Evidence: `desktop-migration-history-identity.txt`,
`desktop-migration-known-history.txt`, `desktop-migration-restart-picker.txt`,
`desktop-migration-restart-unlocked.txt`.

This exercises existing legacy single-vault → account-registry migration with current
schema/state. Schema 2 → 3 interruption/rollback/incompatible-version rejection
remain executable fixture evidence, not packaged process-kill/power-loss proof.

## Exact commands and fixture configuration

Workspace: `C:/Users/william/Desktop/git/cords`; `$taskRoot` is the evidence root above.
Ephemeral credentials were kept outside Git. Native command failures were checked.

```powershell
cargo tree --target all -i proc-macro-error2 --workspace --depth 9
# Add reviewed Cargo.toml patch / deny.toml source rule, then:
cargo update -p hax-lib-macros
cargo tree --locked --target all -i hax-lib-macros --workspace --depth 8
cargo deny check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
# After diagnosing ENOSPC, generated package outputs only:
cargo clean -p cords-client-core -p cords-server-core -p cords-client
$env:CARGO_INCREMENTAL = '0'
cargo test --workspace --all-features --locked
$fixture = Get-Content "$taskRoot/private-state.json" | ConvertFrom-Json
$env:CORDS_TEST_DATABASE_URL = $fixture.databaseUrl
cargo test -p cords-server-core --locked -- --ignored --test-threads=1
# In bins/cords-client/ui:
npm.cmd run format:check
npm.cmd run check
npm.cmd run lint
npm.cmd run test
npm.cmd run build
npm.cmd audit --audit-level=high
# Back at workspace root:
$env:CORDS_DIST_DIR = Join-Path $taskRoot 'artifacts'
& ./build-scripts/windows-client.ps1
cargo build --locked --release -p cords-server
cargo build --locked --release -p cords-dev
```

Container prefix: `cords-c0-81fc93d9c7d0478ba1fe26bf95a1709d`.
PostgreSQL database/user `cords`; random 32-byte disposable password provided by
the private fixture environment. Remote setup command (ephemeral substitution redacted):

```sh
docker run -d --name <prefix>-pg --label cords.acceptance=c0 \
  -e POSTGRES_USER=cords -e POSTGRES_DB=cords -e POSTGRES_PASSWORD=<ephemeral> \
  -p 127.0.0.1::5432 postgres:17-bookworm
```

Assigned PostgreSQL port was 32769. No production database/container was used.

```powershell
ssh.exe -N -o BatchMode=yes -o ExitOnForwardFailure=yes `
  -L127.0.0.1:58200:127.0.0.1:32769 `
  -L127.0.0.1:58202:127.0.0.1:58202 `
  -R127.0.0.1:58201:127.0.0.1:58201 william@192.168.86.54
$env:CORDS_DATABASE__URL = $fixture.databaseUrl
./target/release/cords-server.exe --config "$taskRoot/server.toml" serve --migrate
```

Server config:

```toml
[server]
listen = "127.0.0.1:58201"
public_origin = "https://localhost:58202"
name = "C0 Packaged Acceptance"
join_policy = "public"
data_dir = "C:/Users/william/AppData/Local/Temp/cords-c0-remediation-81fc93d9c7d0478ba1fe26bf95a1709d/server-data"
[database]
url = "provided-by-environment"
migrations_dir = "C:/Users/william/Desktop/git/cords/migrations/postgres"
```

The first Caddy bind-mounted config failed: permission denied reading Caddyfile.
No host security policy/permissions were weakened. Retry used `docker create --name
<prefix>-tls-copy --label cords.acceptance=c0 --network host caddy:2.10-alpine`,
`docker cp <config> <container>:/etc/caddy/Caddyfile`, then `docker start <container>`.
Configuration:

```caddyfile
https://localhost:58202 {
  bind 127.0.0.1
  tls internal {
    protocols tls1.3
  }
  reverse_proxy 127.0.0.1:58201
}
```

CA copied from `/data/caddy/pki/authorities/local/root.crt` to `tls-ca.pem`.
Certificate-validating readiness command:

```powershell
$env:CORDS_C0_EVIDENCE = $taskRoot
node -e 'const fs=require("fs"),https=require("https"),p=process.env.CORDS_C0_EVIDENCE,s=JSON.parse(fs.readFileSync(p+"/private-state.json")); https.get({hostname:"localhost",port:s.tlsPort,path:"/health/ready",ca:fs.readFileSync(p+"/tls-ca.pem"),minVersion:"TLSv1.3",maxVersion:"TLSv1.3"},r=>{console.log("TLS="+r.socket.getProtocol()+"; authorized="+r.socket.authorized+"; HTTP="+r.statusCode);r.resume();if(r.statusCode!==204)process.exitCode=1}).on("error",e=>{console.error(e.message);process.exitCode=1});'
```

Each independent client was launched separately (`A`, then `B`) and its account
created through the UI. Never copy one messaging client's vault into the other:

```powershell
$clientLabel = 'A' # Repeat with B.
$zip = Join-Path $taskRoot 'artifacts/cords-client-windows-x86_64-0.1.0.zip'
Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $taskRoot "installation-$clientLabel")
$env:CORDS_CLIENT_STATE = Join-Path $taskRoot "vault-$clientLabel"
$env:CORDS_CLIENT_CA = Join-Path $taskRoot 'tls-ca.pem'
Remove-Item Env:CORDS_CLIENT_MIGRATIONS -ErrorAction SilentlyContinue
Start-Process -FilePath (Join-Path $taskRoot "installation-$clientLabel/cords-client-windows-x86_64/Cords.exe") -PassThru
# Negative trust: close B, preserve vault, unset CA and reopen/unlock/select channel.
Remove-Item Env:CORDS_CLIENT_CA -ErrorAction SilentlyContinue
# Close B, restore CORDS_CLIENT_CA, reopen, verify catch-up.
```

GUI steps are in the tables/transcripts above, exercised against native packaged
windows using the computer-use skill. Passwords belonged only to disposable fixtures;
no personal Cords vault was opened or modified.

For the known-history migration, close Alpha and verify process exit first. The
actual source was its sole `accounts/<local-id>/client.db`; the copy was opened
only while the server/TLS endpoint were stopped:

```powershell
$sourceDbs = @(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'vault-A/accounts') -Recurse -File -Filter 'client.db')
if ($sourceDbs.Count -ne 1) { throw 'Expected one fixture vault database' }
$migrationRoot = Join-Path $taskRoot 'legacy-history-vault'
if (Test-Path -LiteralPath $migrationRoot) { throw 'Preserve existing migration fixture' }
New-Item -ItemType Directory -Path $migrationRoot | Out-Null
Copy-Item -LiteralPath $sourceDbs[0].FullName -Destination (Join-Path $migrationRoot 'client.db')
Get-FileHash -Algorithm SHA256 $sourceDbs[0].FullName,(Join-Path $migrationRoot 'client.db')
$env:CORDS_CLIENT_STATE = $migrationRoot
Remove-Item Env:CORDS_CLIENT_CA -ErrorAction SilentlyContinue
Remove-Item Env:CORDS_CLIENT_MIGRATIONS -ErrorAction SilentlyContinue
Start-Process -FilePath (Join-Path $taskRoot 'installation-migration/cords-client-windows-x86_64/Cords.exe') -PassThru
# Migrate in UI; verify identity/pin/archive; close, reopen with new password.
```

The source-hash comparison used `Get-FileHash -Algorithm SHA256` against registry
`C:/Users/william/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/hax-lib-macros-0.3.7`
and pinned checkout
`C:/Users/william/.cargo/git/checkouts/hax-580ebeee043cdea1/d9c15b6/hax-lib/macros`,
for each of `src/lib.rs` and `src/dummy.rs`. A mismatch would fail this check.

Read-only final PostgreSQL evidence was collected with `docker exec -i <prefix>-pg
psql -U cords -d cords -v ON_ERROR_STOP=1` over SSH, using:

```sql
select version();
select account_id,device_id from device_contacts order by device_id;
select id,name,epoch,next_sequence,confidentiality_mode from channels order by id;
select count(*) as events,count(distinct event_id) as unique_events from route_events;
select route_id,sequence,event_id,envelope::jsonb->>'content_encoding' as encoding
from route_events order by route_id,sequence;
```

An initial diagnostic query used nonexistent table `server_devices`; it failed and
was corrected to the migration-defined `device_contacts` before proceeding. This
read-only query failure did not alter fixture state or acceptance results.

Logs include `cargo-deny-final.log`, `clippy.log`, `workspace-tests-retry.log`,
`postgres-tests.log`, `frontend-retry-*.log`, `windows-package.log`, `server-build.log`.

## Remaining limitations / activation decision

- Native macOS packaged evidence remains pending. C0 stays incomplete; C1 inactive.
- Offline Archives reading works. The dialog has no search field. Selecting active
  or retired channels through the ordinary sidebar still requests server metadata
  and fails disconnected, even while Archives can read retained content. Recorded
  in `desktop-B-offline-sidebar-limitation.txt`; C0 introduces no new UI behavior.
- Current packaged core paths above were exercised, not every placeholder or future
  feature. Succession/recovery, departure/burn, public acknowledgement and permission
  failure paths have real independent-client/TLS/PostgreSQL regression evidence;
  this is not fresh packaged UI proof of every such workflow.
- No packaged process-kill during migration, power-loss, hardware lifecycle,
  MSI/NSIS installation, release signing or separate-physical-machine proof.
- Localhost uses an explicitly trusted private test CA. Production-domain/public
  Web PKI deployment acceptance is separate. Future scenarios retain their own gates.

## Migration, rollback and fixture retention

C0 adds no runtime schema or production security format. The build dependency and
documentation/tests can be reverted against current durable formats; reverting the
pin reintroduces the advisory/dependency-gate failure. Never rewind live MLS keys,
heads, counters, outboxes or cursors to roll back code.

Migration used an isolated disposable copy with original and migrated vault retained
separately. No old snapshot was restored into a live authority/messaging session.
Ordinary package restarts reused current vaults/pins. Future migration gates must
record matching backup sets, compatible binaries, fences and repair-forward procedures.

Test vaults, package and logs remain outside Git. Test clients, server, SSH tunnel
and isolated Docker fixtures are stopped after evidence capture; existing user Cords
and remote production services remain untouched. Retained fixtures are disposable
artifacts, not deployment or long-term backup guarantees.

## Fresh remote Linux acceptance — 2026-10-11

### Plan and inspection

Inspect the authorized host and deployment script → add isolated instance support
→ validate guards → use official packaging and real native desktop interaction
→ validate lifecycle, migration and negative protocols → stop and retain fixtures.
No C1 behavior or messaging/cryptographic format was introduced.

`william@192.168.86.54` runs openSUSE Tumbleweed 20261009, x86_64, kernel
7.2.6-1-default, Docker 29.7.2-ce. Its `~/git/cords` was on `main` at the same
baseline commit above. Seven build scripts had executable-bit changes only;
`cords-up.sh` was untracked. Those changes and that original script were preserved.
Original script SHA-256, unchanged after testing:
`d89029b4b1c5a6f0399aace42fccc2132ca482440415c71c5a8a9616c5fab028`.
Existing healthy `cords-server` on port 4849 and `cords-postgres`, their network,
volumes/configuration, and unrelated services were not stopped or changed.
Their container IDs, start times and final running states match the initial snapshot.

No Linux package or active graphical session was available on that host. The
existing Windows archive was not used. The host lacked `rg`, so inspection used
`grep`; the host's missing Ayatana development module was not installed globally.
Host tools: Rust/Cargo 1.98.1, Node 24.18.1, npm 11.16.0, Python 3.13.15.
No host package, runner configuration or production deployment was modified.

Fixture root: `/home/william/cords-c0-linux-20261011-4c6fb2d3` (private directory).
Source was cloned into its `source` subdirectory, then overlaid with the local C0
working tree. It is the baseline **plus uncommitted C0 changes**, not a claimed
clean release commit. Source archive SHA-256:
`0c9aa037219da77cd4ee223dbe6aa19e4a370dff94ff5117ab01e8425ec1e8f0`.
Final isolated deployment script was uploaded separately and passed `bash -n` and
`shellcheck`; SHA-256:
`ad1620d18370ff034b748f738fb10c6f2976b5de78555b11451fb997283baa7a`.
Tested lockfile SHA-256:
`3cdb276e86d3bd9c2b82fe752253bf521b5acef4206abeaeddde6a7b4b307ca4`.
CRLF normalization was limited to shell scripts in that isolated source copy.

### Implementation and deployment

The new repository-root `cords-up.sh` is a backward-compatible copy of the inspected
remote script. Defaults retain its existing resource names, origin, admission
policy and port. Named instances add isolated names/labels, explicit configuration,
origin/admission/publish options, image-name isolation and ownership checks.
`start`, `restart` and `stop` retain database, server key, configuration and volumes.
`up` refuses existing resources. `down` still means **permanent purge**, requiring
confirmation; it was not used during acceptance. Restart is not purge.

The fixture used the existing script's official OCI build/migration/start path with:

```bash
export CORDS_INSTANCE=cords-c0-linux-20261011-4c6fb2d3
export CORDS_CONF_DIR=/home/william/cords-c0-linux-20261011-4c6fb2d3/config
export CORDS_PUBLISH=127.0.0.1:58301:4848
export CORDS_PUBLIC_ORIGIN=https://localhost:58302
export CORDS_JOIN_POLICY=public
export CORDS_SERVER_NAME='C0 Linux Acceptance'
bash cords-up.sh up
bash cords-up.sh restart
bash cords-up.sh stop
bash cords-up.sh stop
bash cords-up.sh start
bash cords-up.sh start
bash cords-up.sh up  # expected refusal; existing instance is not overwritten
bash cords-up.sh stop
bash cords-up.sh status
```

Fresh random credentials were stored only in private fixture files. Container,
network, volume and image names use the fixture prefix. PostgreSQL 17.10 has
separate `cords` desktop-acceptance and `cords_test` regression databases; neither
is a production database. Server image ID:
`sha256:47b71c99a66a99ae7b185922c5edce44818ab8d6842da4d5e1e325ad6afed358`.

Caddy 2.10.2 provided a separate private test CA and loopback TLS endpoint. It used
`bind 127.0.0.1`, `tls internal { protocols tls1.3 tls1.3 }`, HSTS, and
`reverse_proxy 127.0.0.1:58301`. CA trust was explicit via `CORDS_CLIENT_CA`;
normal certificate and signed server identity checks remained enabled. Readiness
and discovery were checked with `curl --cacert gui/ca.pem --fail`, and:

```bash
openssl s_client -connect 127.0.0.1:58302 -servername localhost \
  -CAfile gui/ca.pem -verify_return_error -tls1_3 </dev/null
openssl s_client -connect 127.0.0.1:58302 -servername localhost \
  -CAfile gui/ca.pem -tls1_2 </dev/null  # expected rejection
```

TLS 1.3 negotiated `TLS_AES_128_GCM_SHA256`, verification code 0; TLS 1.2 was refused.
This private-CA localhost evidence does not establish public-domain Web PKI rollout.

### Package, runtime and reproducible commands

Official `build-scripts/linux-client-tar.sh` produced a fresh 0.1.0 x86_64 archive
using existing builder image
`vpn-appliance-manager-linux-builder:node24.18.0-rust1.97.1`:

```bash
root=/home/william/cords-c0-linux-20261011-4c6fb2d3
docker run --rm --name cords-c0-linux-20261011-4c6fb2d3-build \
  --label chat.cords.instance=cords-c0-linux-20261011-4c6fb2d3 \
  --memory=8g --cpus=2 --entrypoint /bin/bash \
  -e CARGO_BUILD_JOBS=2 -e CARGO_INCREMENTAL=0 \
  -v "$root/source:/source:z" -w /source \
  vpn-appliance-manager-linux-builder:node24.18.0-rust1.97.1 \
  build-scripts/linux-client-tar.sh
cd "$root/source/dist"
sha256sum -c cords-client-linux-x86_64-0.1.0.tar.gz.sha256
for who in A B; do
  tar -xzf cords-client-linux-x86_64-0.1.0.tar.gz -C "$root/gui/installation-$who"
done
```

| Artifact | SHA-256 |
| --- | --- |
| `cords-client-linux-x86_64-0.1.0.tar.gz` | `870bcbcf16ea383820d7f9e74c477cbf3f6ebec5a93b38623dcdd8db4530bb24` |
| `Cords.AppImage`, independently installed A/B copies | `7e2feb4bfc130bdbe444e10af6d640dce05b32cda1c928de2761e26a80c6fbbe` |

Native graphical execution used a dedicated Debian 12 container on the real Linux
kernel, Xvfb `:90` at 1400×1000, Openbox and D-Bus/AT-SPI. Its unprivileged UID 1000
had Docker's normal seccomp profile and `--cap-drop ALL`. WebKit sandboxing was not
disabled. This is native Linux package execution, not a browser mock or CLI-only
acceptance. AppImages were extracted with their supported `--appimage-extract`
operation and launched through the packaged `AppRun`, with all bundled resources;
FUSE integration and system desktop registration were not tested.

Runtime versions: Rust/Cargo 1.97.1, Node 24.18.0, npm 11.16.0, Python 3.11.2;
GTK 3.24.38-2~deb12u3, WebKitGTK 2.50.6-1~deb12u2, Ayatana 0.5.92-1,
librsvg 2.54.7+dfsg-1~deb12u1, Xvfb 21.1.7-3+deb12u13, Openbox 3.6.1-10,
pyatspi 2.46.0-2, xdotool 3.20160805.1-5, Pillow 9.4.0-1.1+deb12u1,
D-Bus 1.14.10-1~deb12u1. GUI image ID:
`sha256:871a9dd20bf22d65dce4e7e8c10c3b5d055b5d7e1e01e0180d133f2fc4ffe3d3`.

The retained `gui/start.sh`, `gui/launch.sh` and `gui/desktop.py` reproduce the
graphical setup and interactions. No test-only Tauri commands or DOM injection
were added. Controls were identified from the actual AT-SPI tree; controls without
accessible actions used mouse input, and text used ordinary keyboard input.
The harness raises the correct process's window and scrolls controls into view.
Example commands used:

```bash
desktop=cords-c0-linux-20261011-4c6fb2d3-desktop
docker exec -w /test/installation-A/cords-client-linux-x86_64 \
  "$desktop" ./Cords.AppImage --appimage-extract
docker exec "$desktop" /test/launch.sh A no-ca
docker exec "$desktop" /test/launch.sh A
docker exec "$desktop" /test/launch.sh B
ui() { docker exec -e DISPLAY=:90 \
  -e DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/cords-dbus \
  "$desktop" python3 /test/desktop.py "$@"; }
ui click 0 'push button' 'Add Account'
ui set 0 entry Nickname 'Linux C0 Alice'
ui secret 0 'password text' 'New local storage password' /test/password-A
ui secret 0 'password text' 'Confirm password' /test/password-A
ui click 0 'push button' 'Create Account'
ui set 0 entry 'HTTPS origin https://your-server.example:4848' https://localhost:58302
ui click 0 'push button' 'Trust and continue'
ui secret 0 entry 'One-time claim code' /test/claim-code
ui click 0 'push button' 'Claim server'
ui set 0 entry Message 'C0 Linux encrypted A01 independent Alice'
ui click 0 'push button' 'Send message'
ui click 1 'push button' 'Archives · locally retained history'
ui click 1 'push button' '#c0-linux-encrypted · encrypted · 01a12b67-6b5e-7111-8487-fb7773aa97bd'
ui dump 1
ui shot /test/evidence/offline-restarted-archive.png
```

Each operation was run only at its corresponding UI state. Passwords and the
bootstrap code were supplied from mode-0600 private files, not logged or placed
in command arguments. A/B use separate extracted installations and `vault-A` /
`vault-B`; `CORDS_CLIENT_MIGRATIONS` was unset. Neither received copied identity,
vault, device keys or MLS state from the other.

### Validation results

| Required Linux gate | Result and observed evidence |
| --- | --- |
| Package installation/startup | PASS: two matching official package copies extracted and actual native windows opened. |
| Independent accounts | PASS: accounts `g00FBvyQJOTL1dl5MSnrvGeG1QXjLs4jQAexZO4feYY` and `sfKBh_u_2eCtO44TdKik6ELjjUj00_-r9sHE-pzPp_s`; devices `01a12b65-50c2-7850-8fee-c39eb2d53252` and `01a12b67-8061-7b90-a070-a3e8e7355c68`. |
| Vault lock/unlock | PASS: wrong password refused; correct password reopened; Switch Account removed messages/server details from locked UI. |
| Authentication/trust | PASS: owner claim and second-account admission through the GUI; stable pin `EPKBxB7SuslpyRngEiXqyoA4Bvck_o8ID6MdEWhM03c`. |
| Verified TLS | PASS: trusted private CA accepted; omitted CA produced `UnknownIssuer` in the packaged UI. |
| Public exchange | PASS: A03/B04 bidirectional `public.signed` messages, A07 offline catch-up and B09 after server restart. |
| Encrypted MLS exchange | PASS: Bob published its own KeyPackage; Alice added Bob through the member selector, producing epoch 1; A01/B02 exchanged and decrypted on both clients. |
| Synchronization/order | PASS: Bob closed during A05/A06 encrypted and A07 public sends, then caught up with preserved ordering. |
| Client restart | PASS: same vault/device/pin, old and new messages retained; channel selection is manual after restart. |
| Server restart/persistence | PASS: `cords-up.sh restart` restarted server and PostgreSQL; before/after identity/channel/event snapshots matched; A08 encrypted and B09 public exchange worked afterward. |
| Offline archives | PASS: both channels retired through owner UI, retained on both clients; encrypted/public history readable with server and TLS proxy stopped, including after offline client restart/unlock. |
| Stored-state/migration | PASS: closed Bob database copied into isolated legacy-installation fixture, hash matched; packaged migration rewrapped with new password; identity, device, pin and both archives preserved after restart. Source vault retained. |
| Invalid/incompatible protocols | PASS: packaged UI refused correctly signed protocol 2-only discovery (`no common protocol`) and separately tampered discovery (`server metadata signature is invalid`). Fixtures generated and verified using the production `cords-protocol` encoder and Ed25519 implementation. |
| Existing regressions | PASS: 37 Rust workspace tests and 21 real disposable PostgreSQL tests on Linux; 36 frontend tests, diagnostics, formatting, Clippy and dependency audit rerun as below. |

Final PostgreSQL evidence: 10 unique route events (one MLS commit, five encrypted
application events, four public signed events), encrypted epoch 1, both original
channel IDs retained and retired. No rollback of live keys, counters, cursors or
accepted records was performed. The migration copy stayed offline; it did not
resume messaging with duplicated device credentials.

Linux checks used the same builder, source mount and job limits as the package,
on the isolated instance network. A private `regression.env` supplied only its
dedicated `cords_test` database URL. Exact commands within the test container:

```bash
cargo fmt --all -- --check
cargo test --release --workspace --all-features --locked
export CORDS_TEST_DATABASE_URL="$CORDS_DATABASE__URL"
cargo test --release -p cords-server-core --locked -- --ignored --test-threads=1
```

Fresh Windows-side shared-code/frontend checks after the deployment script addition:

```powershell
cargo deny check
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
# In bins/cords-client/ui:
npm run check
npm test -- --run
npm audit --audit-level=low
```

All passed; Svelte diagnostics 0 errors/0 warnings, 36 frontend tests, npm 0
vulnerabilities. `cargo deny` reports advisories/bans/licenses/sources OK; existing
warning-level duplicate/unused-exception notices remain, with no suppression added.

Guard validation rejected an invalid instance, `/etc` as a config target, a
production image name for a test instance, a deliberately mismatched volume label,
and an attempt to run `up` against production. Repeated stop/start passed; repeated
fresh `up` correctly refused overwrite. Purge was never used as a persistence test.

Failures diagnosed before retry: builder image's inherited entrypoint; SELinux
mount label; existing UID 1000 in the GUI base image; ShellCheck diagnostics;
JSON rather than text bootstrap logging; unsupported accessibility text editing;
window focus/scrolling and a changing accessibility tree; expected labels differing
from harness assumptions; TLS-proxy startup readiness; and a read-only SQL query
using nonexistent `lifecycle_head_hash`, corrected to migration-defined `retired`.
These required harness/environment corrections, not protocol or UI changes.
One early archive screenshot preceded final rendering; the later
`offline-restarted-archive.png` was inspected and shows the complete retained text.

Evidence remains in fixture `logs`, `evidence`, `gui/evidence`, and the public
`protocol-fixture` helper. Main harness SHA-256:
`b5e1155bb752b2f28a2c324a8986a1aadf5295f9aa7999cfab649cc830688672`.
Production resource snapshot comparison passed; original checkout remains dirty
only as inspected. All named test services and clients are stopped; database,
vaults, packages, logs and volumes are retained. Desktop supervisor stop used
Docker's eventual kill after both application processes were terminated; this is
not application crash-recovery evidence.

Linux PASS applies to this native Debian 12 graphical runtime, not every Linux
distribution or desktop. No FUSE/system installation, Apple signing, hardware,
physical-machine separation or whole-machine power-loss claims are made. Existing
offline ordinary-sidebar/search limitations remain; the Archives path was exercised.

### Native macOS gate — initial jump-host inspection

The user subsequently authorized `192.168.86.42` via `.54`. Direct local ProxyJump
with strict checking first refused an unknown target host key. Its Ed25519 key
was obtained from `.54`'s already-established known-hosts record, over verified SSH:
`SHA256:GS4oG+L5xSj75vGJZtwGAuswPnhTc9hby8/NoRtLVcY`.
With that explicit known-hosts file, target identity verification passed but local
authentication as `william` was rejected. Existing authenticated SSH from `.54`
to `.42`, with `BatchMode=yes`, `StrictHostKeyChecking=yes`, `ForwardAgent=no`,
succeeded as `william`. No private key was copied or agent forwarded.

Verified native host: arm64, macOS 26.6.2 build 25G83, Xcode 26.6 build 17F113,
SDK 26.5, Rust/Cargo 1.97.1 and both Apple Darwin targets installed. Homebrew's
`/opt/homebrew/bin` is absent from noninteractive SSH PATH; explicitly adding it
exposes existing Node 24.15.0/npm 11.12.1. Python is 3.14.7. Registered runner's
public fields identify `Mac-mini`, ID 6, with `macos:host`, `mac-mini:host`,
`macos-latest:host`, and `arm-ubuntu-22.04:docker://ghcr.io/catthehacker/ubuntu:act-22.04`.
Thus the ARM Ubuntu label is a Linux container, not native macOS execution.

Only 1.5 GiB free on its 228 GiB APFS data volume and no additional writable build
volume were found. `/dev/console` is owned by root; existing `who` records do not
establish a currently accessible unlocked graphical session. The System Events
inspection did not return and its specific probe process was stopped. The user
was asked to provide build capacity and log in/unlock as `william`; no files,
runner settings, TCC settings or release credentials were changed. Native package
build/runtime acceptance is **pending**, not a connectivity/build/test pass or
failure. C0 remains incomplete and C1 remains inactive.

The later CI/CD request is tracked separately in `../ci-rework-plan.md` and
`ci-rework.md`; its local fixture signing tests do not satisfy this native macOS
gate. On a repeated inspection the Mac still had 1.5 GiB free and root owned
`/dev/console`. At that initial inspection no macOS package had been built or
executed. The subsequent native build/remediation evidence follows below.

Reproduce the verified jump-host inspection without agent forwarding or altered
host-key checking (the existing jump-host target authentication is used):

```powershell
ssh.exe -o BatchMode=yes -o StrictHostKeyChecking=yes william@192.168.86.54 `
  'ssh -n -o BatchMode=yes -o StrictHostKeyChecking=yes -o ForwardAgent=no william@192.168.86.42 "id -un; uname -sm; sw_vers; xcodebuild -version; xcode-select -p; rustc --version; cargo --version; rustup target list --installed; df -h /System/Volumes/Data; stat -f %Su /dev/console"'
```

Use `-n` for nested inspection commands so inner SSH does not consume the rest
of an outer script's standard input. Public runner fields were inspected without
printing `.runner` tokens. Existing runner label routing must be preserved:
native jobs belong on `Mac-mini`'s host macOS labels, never its ARM Ubuntu label.

### Native macOS remediation — packaged integrity and startup

After the user's instruction to continue, `df` reported **23 GiB free**, sufficient
for bounded native builds. No files were deleted to obtain that space. Although
`/dev/console` remains owned by root, `scutil`'s console-user record identifies
william UID 501, `OnConsole=TRUE`, `LoginDone=TRUE`. Root ownership alone does not
prove a login screen. GUI access remains unavailable: bounded System Events
inspection times out after 12 seconds, `screencapture` fails with
`could not create image from display`, and native Swift `AXIsProcessTrusted()`
returns **false**. No TCC, Gatekeeper, quarantine or authentication policy changed.

Isolated native fixture: `/Users/william/cords-c0-macos-20261011` on `.42`.
The validated Linux C0 source snapshot, excluding build outputs and private fixture
state, was transferred over verified SSH via `.54`. Source tar SHA-256:
`671c11d4901f362bff8276da1d79eda13cf02a6f97e13a5838d06849cb606f26`.
An isolated Git snapshot `69b090575fb2b940740f20f206433e2b4c271ba9` supplies build
metadata; this is a fixture commit, not the original Cords baseline commit.
The three subsequent packaging changes below are separate from that snapshot.
No GPG/Cosign private key, SSH private key or release credential was copied to Mac.

Native environment: Apple ARM64_T8103, 8 GiB RAM; macOS 26.6.2/25G83;
Xcode 26.6/17F113 and SDK 26.5; Rust/Cargo 1.97.1; Node 24.15.0/npm 11.12.1;
Python 3.14.7. Tauri CLI 2.12.0 and library 2.11.5 remain locked. Both Apple
Darwin Rust targets were already installed; no host toolchain replaced.

Initial native ARM PKG and universal DMG builds exited successfully. The initial
PKG hash is `07dc832f95675fd15ccb9cd8c4c4d69aaab353a44afbe0135bb7b0cf19ff56be`.
**Initial PKG integrity acceptance failed:** strict `codesign` rejected both original
and extracted apps: `code has no resources but signature indicates they must be present`.
The linker-created signature had `Info.plist=not bound` and `Sealed Resources=none`.
The failure occurred before application launch. The failed package/installation and
logs remain retained separately, not overwritten as a purported accepted fixture.

Plan → implement → validate remediation:

- Set supported Tauri `bundle.macOS.signingIdentity` to `"-"`, preserving explicit
  `APPLE_SIGNING_IDENTITY` configuration. This seals the whole app with ad hoc
  signing. It is **not Developer ID signing, notarization or installer signing**.
- Require strict bundle verification in the built app and fully extracted PKG,
  compare packaged executable bytes, and require strict verification plus both
  architectures in the read-only mounted DMG before emitting its checksum.
- Two DMG validation-script mistakes failed closed and were diagnosed: Tauri removes
  the intermediate app when only `dmg` is requested, and Apple's `lipo` requires
  the input filename before `-verify_arch`. Corrected actual-payload checks passed
  a complete native rerun, including verified unmount. No assertion was removed
  from the distributed-app integrity gate.

Validated delta hashes (local and Mac copies match):

```text
7bc63cfa6e4b0dab2f87160b6e56c51581cfe8d9d686e6a68960624ce7cf0097  bins/cords-client/src-tauri/tauri.conf.json
5e18fb40992b5f4b45ef815d9e20bd6e63a213b53346fd2f634ea28b92f2ca1c  build-scripts/mac-client-pkg.sh
b8764cb88a592727e1510d0ff13c8e9218620d50b899a81fc21a78011d6643b1  build-scripts/mac-client-dmg.sh
```

Exact native commands, invoked via the established strict `.54` → `.42` SSH route:

```sh
cd /Users/william/cords-c0-macos-20261011/source
export PATH=/opt/homebrew/bin:/Users/william/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export CORDS_DIST_DIR=/Users/william/cords-c0-macos-20261011/artifacts-fixed
bash build-scripts/mac-client-pkg.sh
CI=true bash build-scripts/mac-client-dmg.sh
bash build-scripts/mac-server-tar.sh
```

Logs: `build-client-pkg-fixed.log`, `build-client-dmg-fixed-3.log`, `build-server.log`.
Failed DMG verification attempts are retained as `build-client-dmg-fixed.log` and
`build-client-dmg-fixed-2.log`. The scripts themselves perform the PKG extraction,
strict codesign checks, `hdiutil verify`, read-only attach, architecture verification
and detach. These commands build natively; they are not a hosted runner dispatch.

| Artifact | Native package gate | SHA-256 |
|---|---|---|
| `cords-client-macos-aarch64-0.1.0.pkg` | PASS: strict built/extracted app, resource seal, executable comparison | `5b8b330306dc426c9f4cbda52387f763032330881191b9cd50f757670b19fa8b` |
| `cords-client-macos-universal-0.1.0.dmg` | PASS: image integrity, mounted app signature and arm64/x86_64 slices | `55e87ff5d778027f4711a54dd8ba2d9f6ffcaaac6724b8f34a0036fba99d9a50` |
| `cords-server-macos-aarch64-0.1.0.tar.gz` | PASS: native release build; extracted arm64 Mach-O executes `--help` | `45d342baf4bade3a457d8967dada0d0615f288483fdb6f28d6613385697ae69b` |

The native server binary was extracted to the isolated `extracted-server` directory;
it was not started against production databases. Client packages are unsigned
installers/ad hoc sealed apps. x86_64 code was built and inspected, not executed
on an Intel Mac. No reproducible byte-identical rebuild, Apple distribution trust,
notarization, GPG release signature or published artifact is claimed for this fixture.

PKG startup reproduction:

```sh
pkgutil --expand-full /Users/william/cords-c0-macos-20261011/artifacts-fixed/cords-client-macos-aarch64-0.1.0.pkg /Users/william/cords-c0-macos-20261011/extracted-pkg-fixed
/opt/homebrew/bin/python3 /Users/william/cords-c0-macos-20261011/mac-startup-fixed.py
# Restart the same independent installations/state:
/opt/homebrew/bin/python3 /Users/william/cords-c0-macos-20261011/mac-startup-fixed.py
```

The retained harness copies the extracted app with `ditto` into
`installation-fixed-A` and `installation-fixed-B`, verifies each strict signature,
then starts the actual packaged executable for eight seconds with separate
`CORDS_CLIENT_STATE` paths `vault-fixed-A` and `vault-fixed-B`. Neither process exits
prematurely; both are stopped by the harness without requiring a forced kill.
This is controlled process termination, not a tested GUI quit or crash recovery.
Each creates its own
`accounts.db`, WAL and SHM, with independent database inodes 291099105 and 291099228.
Repeat startup passes using those same paths. No migration-directory override is
used: bundled resources work. The app identifier is `chat.cords.desktop`, Info.plist
is bound, and sealed resources report version 2, 13 rules, 4 files.

This is a **startup/restart smoke**, not two completed account installations:
no account has been created/unlocked through the Mac GUI. TLS trust/authentication,
encrypted/public messaging, synchronization, message/vault restart persistence,
channel lifecycle and offline archives remain untested on Mac. Accessibility and
display access are pending user-provided GUI access; no hidden test-only invoke hook
or CLI fixture substitutes for the packaged GUI gate.

Native frontend reproduction (installed from the committed npm lock by packaging):

```sh
cd /Users/william/cords-c0-macos-20261011/source/bins/cords-client/ui
export PATH=/opt/homebrew/bin:/Users/william/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin
npm audit --audit-level=high
npm run format:check
npm run check
npm run lint
npm test
```

All PASS: npm reports zero vulnerabilities, diagnostics zero errors/warnings,
format/lint pass, **36 tests in 11 files pass**. An initial nested shell quoting
mistake failed before these native checks and accidentally attempted read-only npm
commands from the Windows repository root, which rejected absent package/lockfiles.
The corrected retained `mac-frontend-checks.sh` runs all checks natively and fails
immediately on errors. This harness failure was not represented as a product failure.

Native Rust checks completed successfully:

```sh
cd /Users/william/cords-c0-macos-20261011/source
export PATH=/opt/homebrew/bin:/Users/william/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
cargo fmt --all -- --check
cargo clippy --release --workspace --all-targets --all-features --locked -- -D warnings
cargo test --release --workspace --all-features --locked
```

Results: format PASS; native Clippy PASS (1m13s); native Rust **37 passed, 0 failed,
21 ignored** (3m37s build). The 21 real database tests were not run on Mac; the
previous Linux disposable PostgreSQL evidence remains separately recorded, not
transferred into a Mac passing claim. Full output is `rust-checks.log`.

Retained harness SHA-256:

```text
cb5717e25538ed0cfae4d3e3ecb267fe14709c2670cd5b68fff9955bcbc7484b  mac-startup-fixed.py
7863661adbc82e3f16a126bc48cdedd7574b290b4e729218edce2a204d0e2d97  mac-frontend-checks.sh
924f0fa3ef291fdebfdee8c8653b4c38128bf9ae38de8c6462899f54b5c9ed4a  mac-rust-checks.sh
```

Mac Cargo.lock matches the tested Linux/Windows hash
`3cdb276e86d3bd9c2b82fe752253bf521b5acef4206abeaeddde6a7b4b307ca4`.
Windows post-change
`cargo fmt --all -- --check` and `cargo deny check` pass again; existing warning-level
duplicate/unused-exception notices remain (cargo-deny 0.20.2).
No RUSTSEC-2026-0173 suppression added.

Secure supplemental transfer of the three native packages and their checksum files
to `/home/william/cords-ci-review-20261011/mac-artifacts` on `.54` passes
`sha256sum -c` for all three. Verified SSH host checking remains enabled. This proves
byte-preserving transfer to an isolated review fixture, not provenance-secure hosted
artifact collection into a protected signing job. No private keys were involved.

C0 remains **incomplete**. The Mac GUI and hosted CI security/dispatch gates are
not passing merely because native builds, startup and automated frontend tests pass.
C1 and later product checkpoints remain inactive.
