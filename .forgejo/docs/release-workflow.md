# Cords release workflow

`.forgejo/workflows/release.yml` runs for version tags matching `v*`. It builds
the native Rust binary, stages a Linux AppDir, packages `Cords.AppImage`, signs
it, verifies the signature, and publishes both files to the tag's Forgejo
release.

## Required secrets

- `CI_KEY`: an ASCII-armored private OpenPGP key or its base64 encoding.
- `CI_KEY_PASSPHRASE`: the private key passphrase.

The workflow uses Forgejo's built-in workflow token for authenticated checkout
and release publication. That token must have repository content and release
write permission.

## Release assets

- `Cords.AppImage`
- `Cords.AppImage.asc` (armored detached OpenPGP signature)

Verify a downloaded release with:

```sh
gpg --verify Cords.AppImage.asc Cords.AppImage
```

## Triggering a release

After local validation, create and push a version tag:

```sh
git tag -s v0.1.0
git push origin v0.1.0
```

The local tag signature and the detached AppImage signature are separate. The
workflow creates the latter from `CI_KEY`.

## Validation boundary

Local `cargo test` validates the Rust project. A successful packaging script
validates AppDir staging and AppImage creation on that machine. The Forgejo
workflow itself still requires a configured `ubuntu-22.04` runner with outbound
HTTPS access and sufficient token permissions; those infrastructure details are
not proven by repository-local checks.
