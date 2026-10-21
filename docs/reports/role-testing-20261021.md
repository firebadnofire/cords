# Role testing — 2026-10-21

## Plan

Run the existing three-client role acceptance scenario on `.54`, then build and
execute the current native server role/authorization tests with disposable
PostgreSQL. Keep staged enforcement separate from the deployed catalog path.
Collect JSON/JUnit/logs, verify resource cleanup, and preserve unrelated services.

## Live deployment validation

Source HEAD: `a88af31`. Working-tree snapshot SHA-256:
`c1a7ad9a6ec5cb45743b91da1857c4dcb265e80f66f4c29c8209acf57acd868c`.
Run: `/mnt/1tb-disk/cords-e2e/runs/cords-e2e-20261021-080554-905fe2`.
Local report: `target/e2e/20261021-080548/results.json`.

Executed through `test-e2e.ps1 scenario permissions` with a fresh native build,
a disposable server, and three independent client VMs. Six assertion groups
passed: exact source, build gate, isolated TLS readiness, independent guests,
identity/admission, and role CRUD/assignment/revocation with owner-only denial.
Active role/channel enforcement was explicitly unavailable, so the command
returned exit 3. The disposable server, PostgreSQL, containers, volumes, network,
client guests, overlays and NoCloud media were cleaned successfully.

The supported deployment still uses preparatory roles. A successful catalog
assignment does not establish an active permission grant.

## Native staged enforcement validation

Completed: **27 passed, zero failed, zero ignored**, exit 0. This selection
contains nine policy-evaluator unit tests and 18 PostgreSQL/service integration
tests. Three additional passing harness records cover compilation, database
readiness and cleanup; they are not counted as application tests.

Native test source and binary identity, exact selected test names,
compiler output and per-test results are recorded by the isolated run:
`/mnt/1tb-disk/cords-e2e/runs/cords-e2e-20261021-080840-73f781`.

Test binary SHA-256:
`31d49caa351c699cec11b0ef60df5723c55518f38facb92d4edec1291db97438`.
Local JSON/JUnit, test transcript and selection are under
`target/e2e/role-service-20261021/`.

Validated coverage:

- Migration rollback, repeatability, preservation of definitions/exclusions and
  archival of preparatory authority; invalid identity evidence supplies no grant.
- Admin/moderator inheritance, deny precedence, category boundaries, owner/device
  separation, lockdown and mention-permission rules.
- Delegated assignment, distinct definition authority, self/owner/higher-role
  escalation denial, idempotent retries and immediate stale-session denial after
  revocation.
- Current-policy enforcement for application sends, history, contacts,
  KeyPackages, discovery, channel snapshots, Welcomes and real TLS notifications.
  Denied sends leave route-event counts unchanged.
- Signed, persistent preparatory catalogs and bounded, owner-authorized audit pages.

The staged tests explicitly apply the candidate migration in disposable schemas;
they do not activate production RBAC. Some service-boundary tests use opaque
relay fixtures and prove authorization, not MLS cryptographic continuity.
Independent-client messaging evidence remains a separate acceptance boundary.

No production application source was changed for these tests.

## Cleanup and limitations

The disposable PostgreSQL container used a dedicated 1 TB disk directory, pinned
image and dynamically assigned loopback-only port. Plain PostgreSQL transport was
confined to that host's loopback; live network-client tests retained TLS validation.
Database dump/logs were collected before container removal. Data files and evidence
remain under the manifest-owned run directory on the 1 TB disk.

The final ownership audit found no test containers, volumes, networks or proxies.
The three intentionally stopped failure-evidence guests from prior work remain.
SELinux is enforcing. Default `cords-server` / `cords-postgres` container IDs and
start timestamps were identical before and after the native test run.

Active RBAC remains unavailable through the supported production client/deployment.
Passing staged service tests does not satisfy that activation gate. No role GUI
acceptance was executed in this run.

Reproduce the deployed-client selection from the checkout:

```powershell
.\test-e2e.ps1 scenario permissions
```

The exact native-run wrapper is preserved as `runner.py` inside its remote run.
On `.54`, rerun it with the retained exact source (it creates fresh owned resources):

```sh
python3 /mnt/1tb-disk/cords-e2e/runs/cords-e2e-20261021-080840-73f781/runner.py \
  /mnt/1tb-disk/cords-e2e/runs/cords-e2e-20261021-080554-905fe2/source
```
