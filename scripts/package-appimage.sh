#!/usr/bin/env bash
set -euo pipefail
echo 'error: The desktop builder now deploys native Qt as a complete directory tarball. Set CORDS_QT_PREFIX and run bash build-scripts/linux-client-tar.sh. No legacy Tauri AppImage was built.' >&2
exit 1
