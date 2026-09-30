# Cords release workflow

`.forgejo/workflows/release.yml` is the privileged Cords publication path. It
runs for `main` pushes, `v*` and `V*` tags, and manual dispatches. Pull requests never
receive release or registry credentials.

Forgejo remains canonical. A successful run also mirrors the source revision,
release assets, and server container to `firebadnofire/cords` on GitHub.

## Runner assignment

| Runner label | Native work | Cross-compiled work |
|---|---|---|
| `ubuntu-22.04` | Linux x86_64 client and linux/amd64 server image | Windows x86_64 client |
| `arm-ubuntu-22.04` | Linux aarch64 client and linux/arm64 server image | Windows ARM64 client |
| `macos-latest` | macOS aarch64 client | None |

The offline `windows-latest` runner is intentionally not part of this path.
Windows clients are built with pinned `cargo-xwin 0.23.1` and Rust 1.97.1.

### Action resolution and Docker access

Actions use explicit HTTPS repository URLs rather than the instance's default
action host. Artifact transfer uses Forgejo's patched `upload-artifact@v4` and
`download-artifact@v4`, required for Forgejo artifact compatibility.
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

## Client release assets

Each release contains:

- Linux x86_64 and aarch64 client `.tar.gz` archives;
- Windows x86_64 and ARM64 client `.zip` archives;
- a macOS aarch64 client `.pkg` installer;
- a basename-only `.sha256` file for every client package;
- `cosign.pub` for container verification;
- an armored detached `.asc` signature for every package, per-package checksum,
  and `cosign.pub`;
- `SHA256SUMS` covering those packages, checksum files, and `cosign.pub`; and
- `SHA256SUMS.asc`.

All detached signatures and checksums are verified before upload. Reruns replace
assets with the same name rather than silently accumulating duplicates.

## Server container

The server image is built natively for `linux/amd64` and `linux/arm64`, then
assembled as one multi-architecture manifest. The workflow publishes identical
digests to:

```text
pubcode.archuser.org/firebadnofire/cords
ghcr.io/firebadnofire/cords
```

Both registries receive a traceability tag, the release tag, and `latest`.
Cosign signs and verifies the digest independently in each registry. The GitHub
package's public/private visibility remains a GitHub repository setting; the
workflow updates the package but does not change that setting.

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
- A `main` push or manual branch dispatch uses `build-<full-commit-sha>`.
- Every container also receives `sha-<full-commit-sha>`.
- Releases are regular releases, not prereleases.

## Validation boundary

Local YAML parsing, shell parsing, packaging tests, and event simulations verify
the repository contract. They do not prove runner availability, secret scopes,
registry permissions, GitHub package visibility, hosted cross-compilation, or
live publication. Those require a successful Forgejo Actions run followed by
independent release, signature, and container-pull checks.
