# Avatar display and picker shape follow-up

## Plan

1. Correct picker shape persistence through sealed preferences, native metadata
   mirroring, the existing device-local registry and the picker tile. Preserve
   genericization and nickname privacy behavior.
2. Validate persistence after registry reopen, idempotent upgrades, invalid-shape
   rejection, UI presentation and privacy before proceeding.
3. Continue the codec gate with the incremental TypeScript JXL reader against the
   actual pinned WASM. Keep full upload, signed-profile and playback acceptance
   explicitly incomplete.

## Implementation

The registry previously retained image/crop metadata but omitted shape, and picker
tiles always used a circular CSS mask. It now stores `avatar_shape`, defaults old
registries to `circle`, validates `circle`/`square`, and mirrors the saved shape
after unlock/settings saves. Visible square entries use a square mask. Genericized
entries expose neither the picture nor its shape. Existing accounts acquire their
saved shape when unlocked; the picker never opens a locked vault to recover it.

The JXL display failure remains an incomplete feature path. `Client::self_contact`
currently publishes `avatar: None`, and the local editor stores a private PNG
preview. The shared renderer has no connected JXL worker/player. Publishing that
preview would violate the canonical lossless JXL requirement. No substitute profile
format or compatibility reader was added.

`avatarJxlReader.ts` provides the next codec unit: a narrow incremental adapter to
the existing pinned libjxl ABI. It uses one reused WASM canvas, preserves exact
zero/nonzero duration ticks and rational timebase/loops, enforces format/dimension/
frame/pixel/duration limits, and frees resources on failure/disposal. It requires
an exclusively owned, externally reserved WASM module and watchdog. It is not yet
connected to a worker command loop or visible avatar component. Its heap check
does not establish the full aggregate upload-memory gate.

## Validation

- Five client-core account tests pass, including square/crop persistence after
  database reopen, invalid-shape rejection and repeated old-registry upgrades.
- Two native conversation tests pass, including picker shape privacy redaction.
- All 81 UI tests in 18 files pass. Svelte/TypeScript and ESLint pass; the Vite
  production build passes.
- The real-WASM reader harness passes 25 tests (15 adapter cases plus ten existing
  core cases) on Windows Node 24.18.0 and independently on `.54` Node 24.18.1.
  Cases include all six compositing/orientation/color fixtures at both precisions,
  exact timing/loops, every frame's pixels, a reused canvas, static transparency,
  truncated streams and allocation cleanup. The adapter cases explicitly supply
  a 128 MiB imported decoder memory. Remote checks use an isolated cache directory;
  running Cords/PostgreSQL services were not modified.
- Windows release build passes. A separate package is available at
  `D:\codex-build\cords-avatar-picker-shape-20261014\cords-client-windows-x86_64\Cords.exe`.
  The ZIP's SHA-256 is
  `b0fe77008c6b05f81bd67acc816809922b734e8015d4445fb6ba7da9f20b9de4`.
  The running user application was not replaced or restarted. Packaged visual
  confirmation remains unperformed; unlock an existing account once to mirror
  its sealed shape before checking its locked picker tile.

Additional Rust lint validation encountered two infrastructure failures: Windows
reported an externally altered F: volume (Btrfs), and C: ran out of space writing
release lint metadata. The 311 newly created check-metadata files (170,271,616 bytes)
were preserved under the package directory's `clippy-metadata` sibling, with a
`relocation.csv` source manifest. No source, prior artifact or running application
was deleted. The retry uses `D:\codex-build\cords-avatar-clippy-target` with
temporary files on D:. It reaches the source checks but desktop Clippy fails on
the existing unchanged `conversation::apply` dispatcher's `too_many_lines`
warning (105/100). No lint exception or unrelated dispatcher refactor was added.
The separate client-core all-target Clippy run on D: passes with warnings denied.

Full worker accounting/command-loop integration, lossless upload/verification,
signed revisions and rollback protection, bounded server validation/publication,
cross-server distribution and canvas playback remain outstanding. Packaged manual
upload/remote-playback, real PostgreSQL/two-client/two-server and macOS/Linux desktop
acceptance have not been performed. No acceptance gate is marked complete.
