# Cords

Cords is a native Rust desktop UI mockup for a text-first chat application. It borrows the density and familiar three-pane organization of modern community messengers while keeping the product surface deliberately focused: workspaces, direct conversations, messages, composing, search, and contact details.

The current data is local fixture data. The project is structured as a reusable UI base rather than a finished messaging client:

- `src/app.rs` owns screen composition and interaction state.
- `src/model.rs` contains backend-replaceable view models and fixtures.
- `src/theme.rs` is the central design-token layer.
- `src/widgets.rs` contains reusable visual primitives.
- `packaging/` and `scripts/package-appimage.sh` provide Linux desktop and AppImage staging.

## Run locally

```sh
cargo run
```

The mockup supports conversation selection and filtering, workspace selection, an optional details panel, and locally appending messages through the composer.

## Build

```sh
cargo build --release
```

The release binary is written to `target/release/cords`.

## Package as an AppImage

Install `appimagetool`, then run:

```sh
./scripts/package-appimage.sh
```

Set `APPIMAGETOOL=/path/to/appimagetool` when it is not on `PATH`. The script stages a standard `Cords.AppDir` under `target/appimage/` and writes `target/Cords.AppImage`.

To build and inspect the AppDir without requiring `appimagetool`, run:

```sh
./scripts/package-appimage.sh --stage-only
```

AppImage packaging is Linux-only. The native binary depends on the ordinary graphics/windowing libraries expected by `eframe` (X11 or Wayland); application code and assets do not require a webview or external runtime.
