# Reviewed shared release verification identities

These are the existing public keys explicitly authorized by the user for Cords
on 2026-10-11. They are shared intentionally; no replacement release key was
generated. Private signing material is supplied only to authorized Linux jobs.

| Public source | Checked-in file | Identity |
| --- | --- | --- |
| [GPG public certificate](https://archuser.org/gpg.key), HTTPS redirect to [william.asc](https://archuser.org/gpg/william.asc) | `gpg.pub` | Primary `7D6EF134D851C8DA0862D97494F31AF374E2EE3C` |
| [Cosign public key](https://raw.githubusercontent.com/firebadnofire/bazzite-firebadnofire/refs/heads/main/cosign.pub) | `cosign.pub` | ECDSA P-256; exact SHA-256 `fa232091c35997060654dfbcb58c911adc5432756f5234e8846a51dba3ee2d01` |

The GPG source returns **binary OpenPGP**, despite the redirected `.asc` name.
`gpg.pub` preserves its exact published bytes, SHA-256
`f1d3b7895d0ddb6f5638c92ebf80434152bf9db797d56473e9d1a793c403e8e9`.
The live signing subkey fingerprint is
`F66F1B747BC6EE547BF0ACE442B0D0D2CA34F814`; the full primary fingerprint remains
the authority. GPG self-signatures and key validity are checked with GnuPG.

`identity.json` pins both files, primary fingerprint and source provenance. Scripts
never retrieve replacement keys during signing/verification. A changed URL response,
certificate/subkey update, or Cosign rotation requires an explicit reviewed commit
updating the public file and configuration, with independent authority verification.
Do not update pins automatically from a URL, private import, or release artifact.

## Verification

Use a reviewed checkout containing these pins. On Linux, with Python 3.11+,
GnuPG, OpenSSL and Bash 4+:

```sh
python3 scripts/release-identity.py
bash scripts/verify-release-artifacts.sh /path/to/downloaded-release
```

The verifier checks exact inventory/checksums and every `.asc`, including
`SHA256SUMS.asc`, in an isolated public keyring. It accepts no fingerprint/public
certificate override that differs from the pins. Independent consumers can inspect
the primary fingerprint with:

```sh
gpg --show-keys --with-fingerprint --with-subkey-fingerprint .forgejo/release-keys/gpg.pub
```

For OCI, use immutable references from the verified release metadata, and verify
both destinations independently with the pinned public key:

```sh
digest='sha256:PASTE_VERIFIED_RELEASE_DIGEST'
cosign verify --key .forgejo/release-keys/cosign.pub \
  "pubcode.archuser.org/firebadnofire/cords@${digest}"
cosign verify --key .forgejo/release-keys/cosign.pub \
  "ghcr.io/firebadnofire/cords@${digest}"
```

Do not replace the digest with `latest` as integrity evidence. Public verification
does not require `CI_KEY_B64`, `CI_KEY_PASSPHRASE`, `COSIGN_B64` or `COSIGN_PASSWORD`.
The signer matches imported GPG authority and verifies signatures using this public
certificate, including its known subkeys. OCI signing compares the private key's
derived SubjectPublicKeyInfo with the pin before publication and verifies with this
exact file in each registry. Private keys are never sent to Mac-mini.

Current evidence covers public-key validation and local signing/rejection fixtures.
Actual secret correspondence and registry signatures remain hosted activation gates;
the full revised release workflow is not yet activated. Ad hoc Apple bundle sealing,
Developer ID/notarization, Authenticode and these GPG/Cosign identities are distinct.
