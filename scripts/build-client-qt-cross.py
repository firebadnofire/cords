#!/usr/bin/env python3
"""Build a Windows x64 MSVC Qt client on Linux using cargo-xwin's environment."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile


def run(args):
    print("+", " ".join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), check=True)


def copy_runtime_tree(source, destination):
    """Preserve QML/plugin layouts, excluding development and debug payloads."""
    def ignore(directory, names):
        return [name for name in names if name.endswith((".pdb", ".lib", ".a"))
                or (name.endswith("d.dll") and name[:-5] + ".dll" in names)]
    shutil.copytree(source, destination, ignore=ignore, dirs_exist_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("host-qt", "windows-qt", "build-dir", "cargo-target", "dist-dir", "version"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--profile", choices=("release", "dev"), default="release")
    parser.add_argument("--jobs", type=int, default=2)
    args = parser.parse_args()
    if sys.platform != "linux" or args.jobs < 1:
        parser.error("a Linux host and positive job count are required")
    root = Path(__file__).resolve().parents[1]
    host = Path(args.host_qt).resolve(strict=True)
    windows = Path(args.windows_qt).resolve(strict=True)
    build = Path(args.build_dir).resolve()
    cargo = Path(args.cargo_target).resolve()
    dist = Path(args.dist_dir).resolve()
    toolchain = Path(os.environ["CMAKE_TOOLCHAIN_FILE_x86_64_pc_windows_msvc"])
    version = subprocess.check_output([host / "bin/qmake", "-query", "QT_VERSION"], text=True).strip()
    config = windows / "lib/cmake/Qt6/Qt6ConfigVersionImpl.cmake"
    if version != "6.10.3" or 'set(PACKAGE_VERSION "6.10.3")' not in config.read_text():
        raise RuntimeError("both Linux and Windows Qt SDKs must be version 6.10.3")
    if not (windows / "lib/Qt6Core.lib").is_file():
        raise RuntimeError("Windows Qt must be the x64 MSVC SDK")
    # Keep all output outside source inputs; CMake itself detects cache/toolchain mismatches.
    if build == root or root.is_relative_to(build):
        raise RuntimeError("build directory must not contain the source tree")
    build.mkdir(parents=True, exist_ok=True)
    dist.mkdir(parents=True, exist_ok=True)
    test_paths = []
    for variable, option in (("CORDS_WINDOWS_TEST_SOURCE_ROOT", "CORDS_TEST_SOURCE_ROOT"),
                             ("CORDS_WINDOWS_TEST_OUTPUT_DIR", "CORDS_TEST_OUTPUT_DIR")):
        if os.environ.get(variable):
            test_paths.append(f"-D{option}={os.environ[variable]}")
    run(["cmake", "-S", root / "bins/cords-client/qt", "-B", build, "-G", "Ninja",
         f"-DCMAKE_TOOLCHAIN_FILE={toolchain}", f"-DCMAKE_PREFIX_PATH={windows}",
         f"-DQT_HOST_PATH={host}", "-DQT_FORCE_FIND_TOOLS=ON",
         "-DQT_NO_QTPATHS_DEPLOYMENT_WARNING=ON",
         "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_RC_COMPILER=llvm-rc", "-DCMAKE_MT=llvm-mt",
         f"-DCORDS_CARGO_TARGET={cargo}", f"-DCORDS_CARGO_PROFILE={args.profile}",
         "-DCORDS_RUST_TARGET=x86_64-pc-windows-msvc", "-DCORDS_QT_DEPLOY=OFF",
         "-DCORDS_ALLOW_SOURCE_MIGRATIONS=OFF", *test_paths])
    run(["cmake", "--build", build, "--parallel", args.jobs])
    run(["cmake", "--build", build, "--target", "Cords_qmllint", "--parallel", args.jobs])
    # windeployqt is a Windows executable. Stage the complete release QML and
    # plugin trees instead, including dynamically selected styles/backends.
    # This intentionally trades archive size for predictable offline deployment.
    with tempfile.TemporaryDirectory(prefix="cords-windows-", dir=build) as temporary:
        package = Path(temporary) / "cords-client-windows-x86_64"
        run(["cmake", "--install", build, "--prefix", package])
        for dll in sorted((windows / "bin").glob("*.dll")):
            if dll.stem.endswith("d") and dll.with_name(dll.stem[:-1] + ".dll").exists():
                continue
            shutil.copy2(dll, package / dll.name)
        copy_runtime_tree(windows / "plugins", package / "plugins")
        copy_runtime_tree(windows / "qml", package / "qml")
        (package / "qt.conf").write_text("[Paths]\nPrefix=.\nPlugins=plugins\nQmlImports=qml\n", encoding="utf-8")
        redist = os.environ.get("CORDS_VC_REDIST")
        if redist:
            shutil.copy2(Path(redist).resolve(strict=True), package / "vc_redist.x64.exe")
        required = ["Cords.exe", "Qt6Core.dll", "Qt6Quick.dll", "plugins/platforms/qwindows.dll",
                    "qml/QtQuick/qmldir", "share/cords/migrations/sqlite"]
        for relative in required:
            if not (package / relative).exists():
                raise RuntimeError(f"deployment is missing {relative}")
        (package / "cords-native-build.json").write_text(json.dumps({
            "platform": "windows", "architecture": "x86_64", "qt_version": version,
            "profile": args.profile, "cross_compiled": True,
            "runtime_tests": "Run the cross-built test executables on Windows; not run on Linux",
        }, indent=2) + "\n", encoding="utf-8")
        (package / "CROSS-BUILD.txt").write_text(
            "Native Qt Windows x64 client cross-compiled on Linux.\n"
            "Requires the Microsoft Visual C++ 2015-2022 x64 runtime.\n"
            "Keep the complete extracted directory together. Run Cords.exe.\n", encoding="utf-8")
        shutil.copy2(root / "README.md", package / "README.md")
        shutil.copy2(root / "LICENSE", package / "LICENSE")
        output = dist / f"cords-client-windows-x86_64-{args.version}.zip"
        pending = output.with_suffix(".zip.tmp")
        try:
            # Qt SDK archives contain epoch-dated files; ZIP starts at 1980.
            with zipfile.ZipFile(pending, "w", zipfile.ZIP_DEFLATED,
                                 strict_timestamps=False) as archive:
                for path in sorted(package.rglob("*")):
                    if path.is_file():
                        archive.write(path, path.relative_to(package.parent))
            pending.replace(output)
        finally:
            pending.unlink(missing_ok=True)
        print(f"Built {output}; Windows execution is a separate validation step.", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Windows cross-build failed: {error}", file=sys.stderr)
        sys.exit(1)
