#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/lib/common.sh"
require_linux
for tool in cargo cargo-xwin python3 cmake ninja clang-cl lld-link llvm-lib llvm-rc llvm-mt; do
  require_command "${tool}"
done
[[ -n "${CORDS_QT_HOST_PREFIX:-}" ]] || die 'set CORDS_QT_HOST_PREFIX to Linux Qt 6.10.3'
[[ -n "${CORDS_QT_WINDOWS_PREFIX:-}" ]] || die 'set CORDS_QT_WINDOWS_PREFIX to Windows x64 MSVC Qt 6.10.3'
# cargo-xwin provides both the Rust compiler environment and the matching CMake
# toolchain. Its SDK/CRT cache is user-local; no system packages are installed.
xwin_environment="$(cargo xwin env --target x86_64-pc-windows-msvc)"
eval "${xwin_environment}"
prepare_dist
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-${CORDS_BUILD_JOBS:-2}}"
log 'Cross-compiling the native Windows x64 Qt client (runtime tests require Windows)'
python3 "${REPO_ROOT}/scripts/build-client-qt-cross.py" \
  --host-qt "${CORDS_QT_HOST_PREFIX}" --windows-qt "${CORDS_QT_WINDOWS_PREFIX}" \
  --build-dir "${CORDS_QT_BUILD_DIR:-${REPO_ROOT}/target/qt-windows-cross}" \
  --cargo-target "${CARGO_TARGET_DIR:-${REPO_ROOT}/target/qt-windows-cross/cargo}" \
  --dist-dir "${DIST_DIR}" --version "$(project_version)" \
  --profile "${CORDS_QT_PROFILE:-release}" --jobs "${CORDS_BUILD_JOBS:-2}"
write_sha256 "${DIST_DIR}/cords-client-windows-x86_64-$(project_version).zip"
