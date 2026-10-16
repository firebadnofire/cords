# Cords release workflow

> **Publication disabled in this checkout.** The discovered `release.yml` now has
> only a deliberately skipped gate. Prior definitions are retained outside Actions
> discovery in `workflow-proposals/legacy-release.yml`. The canonical architecture
> is one Cords monorepo plus an independently protected external executor; see
> [the current plan](../../docs/ci-monorepo-plan.md) and
> [executor contract](../../docs/external-release-executor.md).
> The revised signing scripts now use checked-in, user-authorized public pins,
> sign every asset with `.asc`, and reject old `.sig`
> staging directories. No privileged executor has been installed or activated.
> The remainder of this document describes the previous workflow until the validated
> replacement is activated; it is not evidence that the requested system is complete.

## Revised file-signing contract (locally validated)

On the Linux signing executor, set `CI_KEY_B64` and `CI_KEY_PASSPHRASE`. The public
identity comes from [reviewed checked-in pins](../release-keys/README.md).
The first private key imported is never treated as the trust anchor. Exactly one
primary private key must match the configured fingerprint; signing subkeys remain
bound to that primary. Expired/revoked signatures are rejected.

```sh
bash scripts/sign-release-artifacts.sh release
bash scripts/verify-release-artifacts.sh release
```

Signing/verification require Bash 4+, Python 3.11+, GnuPG, OpenSSL and core utilities on
Linux. No release keys are needed on Mac-mini. The signer uses a temporary private
keyring, passes the passphrase by file descriptor, verifies every signature, and
cleans up its keyring/agent. It creates `.asc` for each package, per-package checksum,
`cosign.pub`, optional provenance and the exact `SHA256SUMS` manifest. Signatures
are excluded from that manifest, preventing circular checksums. Valid existing
signature bytes are preserved on retry; incompatible manifests/signatures fail.

Consumer verification needs only the trusted public key and fingerprint, not any
private signing credentials. Obtain the public identity independently of a release
download; the user-authorized sources, fingerprint, exact key bytes and rotation
policy are documented in the linked key README. No live URL change is trusted
automatically. Override variables cannot replace the configured public authority.
The verifier rejects extra/missing inputs, unsigned inputs, symlinks, checksum path
escapes and altered bytes. These scripts do not establish package architecture,
complete platform inventory, tag authorization or remote publication by themselves.

Reproduce the offline supplementary fixtures (no production publication):

```sh
python3 scripts/test-release-contract.py
python3 scripts/test-release-identity.py
bash scripts/test-release-contract.sh
shellcheck scripts/{sign-release-artifacts,verify-release-artifacts,test-release-contract}.sh
```

Fixtures use intentionally fake package bytes and an ephemeral test key. Their
passing results are signing/byte-contract evidence, not platform package evidence.

## Previous workflow (not the revised release contract)

The archived `.forgejo/workflow-proposals/legacy-release.yml` was the previous
privileged Cords publication path. It
runs for `v*` and `V*` tag pushes, including the planned `v0.1.0`, and explicit
manual dispatches. Its intended separation by triggers did not prevent applicable
owner/repository secrets reaching ordinary jobs under Forgejo v14. The earlier
claim that branch/PR jobs never receive publication credentials was incorrect.
Do not use the archived definition as a secure or activated release system.

Forgejo remains canonical. The GitHub mirror job depends on successful Pubcode
container publication and successful Forgejo release publication, so it cannot
mutate GitHub first. It then mirrors the source revision, the exact signed
release payload, and the exact container digest to `firebadnofire/cords`.

## Current Qt desktop checks

The active `ci.yml` now invokes the native Qt builders on `ubuntu-22.04`,
`windows-latest` and `macos-latest`, with Qt 6.10.3, aqtinstall 3.3.0, CMake
3.31.6 and Rust 1.97.1. Windows requires a native x64 MSVC 2022 C++ host;
macOS requires Xcode command-line tools. These are host requirements, not a
claim that all runners are online. The previously offline Windows runner must
be provisioned before its job can validate anything. No native Windows/macOS
hosted result has been established by local Windows/Linux checks.

The existing Rust/Svelte job remains temporarily to check the legacy reference
until native parity and removal. Publication stays disabled. The archived
release proposals below still describe Tauri-era cross-builds and must not be
activated without migration and the existing executor/security gates.

## Previous runner assignment

| Runner label       | Native work                                                        | Cross-compiled work                |
| ------------------ | ------------------------------------------------------------------ | ---------------------------------- |
| `ubuntu-22.04`     | Linux x86_64 client, server tarball, and linux/amd64 server image  | Windows x86_64 client              |
| `arm-ubuntu-22.04` | Linux aarch64 client, server tarball, and linux/arm64 server image | Windows ARM64 client               |
| `macos-latest`     | Universal macOS client DMG                                         | Intel half of the universal binary |

The offline `windows-latest` runner is intentionally not part of this path.
Windows clients are built with pinned `cargo-xwin 0.23.1` and Rust 1.97.1.

### Action resolution and Docker access

Checkout and Node setup use the mirrors in
`https://pubcode.archuser.org/actions`. That organization does not currently
mirror the artifact actions, so artifact transfer uses Forgejo's patched
`upload-artifact@v4` and `download-artifact@v4`, required for Forgejo artifact
compatibility.
See [Forgejo artifact documentation](https://forgejo.org/docs/v15.0/user/actions/advanced-features/#artifacts).

Docker jobs explicitly select `ghcr.io/catthehacker/ubuntu:act-22.04` to provide
Node, Docker CLI, Buildx, and Compose instead of a bare Ubuntu image. The image
does not itself provide a Docker daemon. Both architecture runners must expose
a working daemon to the job container. The build checks `docker version` before
registry authentication and fails with runner configuration context if access
is missing.

The September 30 saved run showed no `/var/run/docker.sock` in the ARM64 job.
On an isolated trusted runner using a local UNIX daemon socket, an administrator
can configure `container.docker_host: "automount"` in the runner configuration.
For a remote daemon, use a TLS-authenticated connection with certificate
validation. Do not expose an unauthenticated TCP daemon. See
[Forgejo Docker access](https://forgejo.org/docs/latest/admin/actions/docker-access/).
Daemon access grants jobs control over that daemon's containers; use a dedicated
CI daemon, separated from Forgejo and production workloads. Do not grant
untrusted pull-request jobs access to a shared privileged host daemon.

The saved macOS log also showed an action-cache clone collision (`info/exclude:
File exists`). Explicit action URLs avoid the failing local repository lookup,
but a persistent clone collision requires runner-side diagnosis with active
jobs stopped before targeted cache repair. This workflow does not delete runner
caches or alter daemon access.

## Required secrets

- `CI_KEY_B64`: the OpenPGP private key, base64-encoded exactly once.
- `CI_KEY_PASSPHRASE`: passphrase for that OpenPGP key.
- `COSIGN_B64`: the password-protected Cosign private key, base64-encoded
  exactly once.
- `COSIGN_PASSWORD`: passphrase for the Cosign private key.
- `GH_KEY`: a classic GitHub PAT able to push repository contents, publish
  releases, and write packages for `firebadnofire/cords`.

The workflow decodes only the secrets explicitly named `*_B64`. Encoded and
decoded key material is never printed. Forgejo checkout, releases, and package
publication use the built-in `${{ forgejo.token }}` with least-privilege
repository scope.

## File release assets

Each release contains:

- Linux x86_64 and aarch64 client `.tar.gz` archives;
- Windows x86_64 and ARM64 client `.zip` archives;
- a universal macOS client `.dmg`;
- Linux x86_64 and aarch64 server `.tar.gz` archives;
- a basename-only `.sha256` file for every client and server package;
- `cosign.pub` for container verification;
- `SHA256SUMS` covering every package, per-package checksum, and `cosign.pub`;
  and
- the detached OpenPGP signature `SHA256SUMS.sig`.

The checksum manifest signature and all checksums are verified before upload.
Reruns replace assets with the same name rather than silently accumulating
duplicates.

## Server container

The server image is built natively for `linux/amd64` and `linux/arm64`, then
assembled as one multi-architecture manifest. The workflow publishes identical
digests to:

```text
pubcode.archuser.org/firebadnofire/cords
ghcr.io/firebadnofire/cords
```

Both registries receive a traceability tag, the release tag, and `latest`.
Pubcode receives and verifies the multi-architecture manifest and its Cosign
signature first. After the Forgejo file release succeeds, the mirror job copies
that manifest with preserved digests, rejects any digest difference, and signs
and verifies the GHCR location. The GitHub package's public/private visibility
remains a GitHub repository setting; the workflow updates the package but does
not change that setting.

Examples:

```sh
docker pull ghcr.io/firebadnofire/cords
docker pull ghcr.io/firebadnofire/cords:latest
cosign verify --key cosign.pub ghcr.io/firebadnofire/cords@sha256:<digest>
```

## Release identities

- A `v*` or `V*` tag uses that tag for releases and container tags. Release tags
  must have a suffix containing only ASCII letters, digits, dots, underscores,
  or hyphens; unsafe names fail before publication.
- A manual branch dispatch uses `build-<full-commit-sha>`.
- Every container also receives `sha-<full-commit-sha>`.
- Releases are regular releases, not prereleases.

## Validation boundary

Local YAML parsing, shell parsing, packaging tests, and event simulations verify
the repository contract. They do not prove runner availability, secret scopes,
registry permissions, GitHub package visibility, hosted cross-compilation, or
live publication. Those require a successful Forgejo Actions run followed by
independent release, signature, and container-pull checks.
