#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
appdir="$repo_dir/target/appimage/Cords.AppDir"
appimagetool="${APPIMAGETOOL:-appimagetool}"
stage_only=false

if [ "${1:-}" = "--stage-only" ]; then
  stage_only=true
elif [ "$#" -ne 0 ]; then
  echo "Usage: $0 [--stage-only]" >&2
  exit 2
fi

cargo build --release --manifest-path "$repo_dir/Cargo.toml"
rm -rf "$appdir"
install -Dm755 "$repo_dir/target/release/cords" "$appdir/usr/bin/cords"
install -Dm644 "$repo_dir/packaging/cords.desktop" "$appdir/cords.desktop"
install -Dm644 "$repo_dir/packaging/cords.svg" "$appdir/cords.svg"
install -Dm644 "$repo_dir/packaging/cords.svg" "$appdir/usr/share/icons/hicolor/scalable/apps/cords.svg"
ln -s usr/bin/cords "$appdir/AppRun"

if [ "$stage_only" = true ]; then
  echo "Staged $appdir"
  exit 0
fi

if ! command -v "$appimagetool" >/dev/null 2>&1; then
  echo "appimagetool was not found; set APPIMAGETOOL or use --stage-only" >&2
  exit 1
fi

ARCH="${ARCH:-$(uname -m)}" "$appimagetool" "$appdir" "$repo_dir/target/Cords.AppImage"
echo "Created target/Cords.AppImage"
