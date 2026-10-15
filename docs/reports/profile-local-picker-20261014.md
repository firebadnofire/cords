# Local profile-picture picker and URL removal

## Plan

Apply the user's 2026-10-14 amendment: profile pictures use local files, with no
URL entry, publication or fetching. Preserve the separate Jewel, account vaults,
picker privacy and concurrent role/account changes. Continue the animated lossless
JXL plan; do not publish a private PNG preview as an alternate canonical avatar.

## Implementation

- My Account and Profile image now expose a native local-file input. Selection
  feeds the existing bounded still-image importer and interactive crop controls.
  Selection cancellation or decoding failure leaves the existing preview intact.
  The input resets so the same file can be selected again.
- Until the full JXL processing workflow is connected, these selections save only
  as encrypted private previews. GIF/JXL/APNG/animated WebP fail explicitly through
  the existing animation guard; no animation is flattened. The editor and settings
  explain the current local-only behavior.
- New self-contact cards contain no URL avatar or private preview. Existing
  URL-only contact pictures render initials with an unavailable explanation;
  viewing messages or admission requests makes no image-host request. The obsolete
  remote-avatar cache/loader and its lock hook were removed. Device signatures,
  authorization and nickname rendering remain unchanged.
- Jewel URL loading remains a separate local feature. Its hint now correctly says
  the image is private and not published in user cards.
- ADR 0009 and the implementation plan supersede the earlier URL-source requirements.
  The signed embedded-JXL baseline and server-side rejection of that baseline's URL
  representations remain part of the unfinished protocol gate.

## Validation

- All 18 UI test files / 80 tests pass, including real component selection/crop,
  explicit animation failure preserving the previous preview, and URL-contact
  rendering without remote loading. File-dialog interaction itself was not manually
  exercised in the packaged WebView.
- All 15 client-core library tests pass, including proof that removed URLs and
  private preview bytes do not enter signed self-contact cards.
- Svelte/TypeScript check: zero errors or warnings. Changed UI lint, client-core
  formatting, whitespace checks and production frontend build pass.
- The final Windows release executable was rebuilt after the final frontend build
  and packaged with SQLite migrations, README and license at
  `D:/codex-build/cords-local-avatar-20261014/cords-client-windows-x86_64`.
  ZIP: `D:/codex-build/cords-local-avatar-20261014/cords-client-windows-x86_64-0.1.0.zip`.
- The old running `dist` package was not overwritten or terminated. The new package
  retains the application identifier and default application-data location; no
  account database or identity files were moved or modified by packaging.

No shared animated-JXL upload, cross-server acceptance or packaged manual desktop
gate is claimed. Continue with the real codec worker/editor connection and signed
profile publication described in the main implementation report.
