#!/usr/bin/env bash
# Fixture-based native Qt interaction under a real X server in an owned guest.
set -Eeuo pipefail
build_dir=${1:?Usage: run-linux-ui-tests.sh BUILD_DIRECTORY SOURCE_DIRECTORY}
source_dir=${2:?Source directory required}
source /etc/os-release
[[ "$ID" == ubuntu ]] || { echo 'Owned Ubuntu guest required' >&2; exit 2; }
export QT_QPA_PLATFORM=xcb QT_QUICK_BACKEND=software LIBGL_ALWAYS_SOFTWARE=1
export LD_LIBRARY_PATH="$HOME/Qt/gcc_64/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
results="$HOME/artifacts/linux-xvfb"
mkdir -p "$results"
if [[ ${CORDS_XVFB_SESSION:-} != 1 ]]; then
    exec xvfb-run -a -s '-screen 0 1280x800x24' env CORDS_XVFB_SESSION=1 bash "$0" "$@"
fi
# Native focus and Escape assertions require a window manager, not just Xvfb.
openbox --sm-disable > "$results/window-manager.log" 2>&1 &
manager=$!
trap 'kill "$manager"; wait "$manager" || true' EXIT
ready=0
for attempt in {1..100}; do
    if xprop -root _NET_SUPPORTING_WM_CHECK | grep -q 'window id'; then ready=1; break; fi
    sleep 0.1
done
[[ "$ready" == 1 ]] || { echo 'Xvfb window manager did not become ready' >&2; exit 2; }
run_test() {
    local name=$1
    shift
    timeout 120 "$@" -o "$results/$name.xml,xml" -o -,txt
    [[ -s "$results/$name.xml" ]] || { echo "Missing Qt result: $name" >&2; exit 2; }
}
run_test qml-controls "$HOME/Qt/gcc_64/bin/qmltestrunner" -input "$source_dir/bins/cords-client/qt/tests" -import "$source_dir/bins/cords-client/qt/qml"
run_test native-media "$build_dir/bin/CordsMediaTests"
run_test native-controller "$build_dir/bin/CordsControllerTests"
run_test row-filter "$build_dir/bin/CordsRowFilterTests"
shopt -s nullglob
captures=("$build_dir"/*.png)
(( ${#captures[@]} > 0 )) || { echo 'Native Qt tests produced no required screenshots' >&2; exit 2; }
mkdir -p "$results/screenshots"
cp -- "${captures[@]}" "$results/screenshots/"
echo 'All four Xvfb software-rendered fixture/native groups passed; live-server GUI acceptance is separate.'
