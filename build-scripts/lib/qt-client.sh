#!/usr/bin/env bash
# Source after common.sh. No host packages or existing installations are changed.
qt_client_requirements() {
  for command_name in cargo rustc python3 cmake; do require_command "${command_name}"; done
  CORDS_QT_PREFIX="${CORDS_QT_PREFIX:-${QT_ROOT_DIR:-}}"
  if [[ -z "${CORDS_QT_PREFIX}" ]] && command -v qmake6 >/dev/null 2>&1; then
    CORDS_QT_PREFIX="$(qmake6 -query QT_INSTALL_PREFIX)"
  fi
  [[ -d "${CORDS_QT_PREFIX}" ]] || die 'Qt 6.10.3 SDK is required; set CORDS_QT_PREFIX to its gcc_64, clang_64 or MSVC SDK directory. See build-scripts/README.md.'
  QT_BUILD_ROOT="${CORDS_QT_BUILD_DIR:-${REPO_ROOT}/target/qt}"
  QT_CARGO_ROOT="${CARGO_TARGET_DIR:-${QT_BUILD_ROOT}/cargo}"
  QT_PROFILE="${CORDS_QT_PROFILE:-release}"
  [[ "${QT_PROFILE}" == release || "${QT_PROFILE}" == dev ]] || die 'CORDS_QT_PROFILE must be release or dev'
}

build_qt_client() {
  local build_name="$1" package="$2"
  shift 2
  log "Building native Qt 6.10.3 Cords (${QT_PROFILE}, ${build_name})"
  python3 "${REPO_ROOT}/scripts/build-client-qt.py" \
    --qt-prefix "${CORDS_QT_PREFIX}" --build-dir "${QT_BUILD_ROOT}/${build_name}" \
    --cargo-target "${QT_CARGO_ROOT}" --package-dir "${package}" \
    --profile "${QT_PROFILE}" --jobs "${CORDS_BUILD_JOBS:-4}" "$@"
}

sign_qt_app() {
  local application="$1" identity="${APPLE_SIGNING_IDENTITY:--}"
  [[ -f "${application}/Contents/MacOS/Cords" ]] || die "Qt application is missing: ${application}"
  while IFS= read -r -d '' library; do
    codesign --force --sign "${identity}" "${library}"
  done < <(find "${application}/Contents" -type f -name '*.dylib' -print0)
  if [[ -d "${application}/Contents/Frameworks" ]]; then
    while IFS= read -r -d '' framework; do
      codesign --force --sign "${identity}" "${framework}"
    done < <(find "${application}/Contents/Frameworks" -type d -name '*.framework' -print0)
  fi
  codesign --force --sign "${identity}" "${application}"
  codesign --verify --deep --strict "${application}"
}
