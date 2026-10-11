# Inactive workflow definitions

`release-builds.yml` prepares the explicit tests-before-build graph. Its mandatory
aggregate accepts only `success` for identity, Rust/frontend/security/PostgreSQL,
Compose and independent packaged runtime acceptance. Runtime acceptance is deliberately
unavailable, so it blocks every build. ARM and native Mac definitions are additionally
inactive pending enforced execution paths. OCI builds export unsigned layouts;
there is no registry login/push or publication job. The external executor independently
checks authorization, test order and provenance; the graph grants no signing authority.
Action/image pinning and hosted Forgejo scheduling acceptance remain unmet.

Windows ARM packaging requires a matching ARM Linux host. Discovered CI retains its
usable AMD64 cross check; ARM definitions remain in this graph and preserved legacy
definition. Do not route ARM to AMD64 by assumption or weaken the packaging guard.
Validate an isolated ARM execution path on opensuse-server before activation.

These files are outside `.forgejo/workflows` and are not discovered as Actions.
Do not copy them into discovery until the [monorepo activation gates](../../docs/ci-monorepo-plan.md)
pass. `legacy-ci.yml` and `legacy-release.yml` preserve the prior definitions for
audit, including old secret references and unsafe publication/host assumptions;
they are not runnable security-approved replacements.

`ci.yml` prepares every branch push, PR and v/V tag check on Linux, retains Rust,
frontend, disposable PostgreSQL, Compose, secret scanning and Windows cross checks,
pins cargo-deny 0.20.2 and verifies the immutable event checkout. It still needs
action/image digest pinning, credential-free job payload evidence, isolated Docker/
Compose execution and hosted acceptance. No passing fixture establishes those gates.

Mandatory release graph (all definitions remain in Cords):

`exact protected tag/commit -> full mandatory tests -> all package/OCI builds ->
candidate manifest -> independently verified external executor -> signatures ->
verified drafts and OCI digests in both registries -> finalization and mirror`.

Missing, skipped, cancelled or failed required checks block builds/publication.
Linux ARM builds must execute on opensuse-server with a validated architecture path;
native macOS universal DMG, ARM64 PKG and server must execute on Mac-mini through an
enforced executor or independently approved native procedure. The legacy Mac host
label is not that executor. Preserve Windows AMD64/ARM64 cross builds and OCI
AMD64/ARM64. Do not introduce extra repositories/runners to bypass the blocked gate.

The current discovered `release.yml` contains only a skipped activation gate: no
build, secret expression, signing, registry operation or publication executes.
This is an intentional local stop, not a complete release implementation. The
skip condition is not evidence of runner task-payload or inherited-secret isolation.
No hosted dispatch has validated its scheduling behavior. The
existing discovered CI retains Linux checks and Windows cross checks; direct Mac
jobs have been removed pending isolation. The required native targets remain in the
plan, package scripts and preserved definitions. Expanded all-branch testing stays
inactive pending inherited-secret and runner reviews. These changes are local until
published; they cannot restrict a malicious workflow targeting the Global Mac runner.
