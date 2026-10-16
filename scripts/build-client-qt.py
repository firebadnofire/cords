#!/usr/bin/env python3
"""Build/test/install the native desktop with an explicitly selected Qt SDK."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
QT_VERSION = "6.10.3"


def run(arguments, env):
    print("Running:", " ".join(map(str, arguments)), flush=True)
    subprocess.run(list(map(str, arguments)), cwd=ROOT, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qt-prefix", type=Path, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    parser.add_argument("--cargo-target", type=Path)
    parser.add_argument("--package-dir", type=Path)
    parser.add_argument("--profile", choices=("release", "dev"), default="release")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--tests", action="store_true")
    parser.add_argument("--cmake", default=shutil.which("cmake"))
    args = parser.parse_args()
    if not args.cmake or not Path(args.cmake).is_file():
        parser.error("CMake >=3.24 must be installed; provide --cmake if it is not on PATH")
    if not 1 <= args.jobs <= 64:
        parser.error("--jobs must be between 1 and 64")
    qt = args.qt_prefix.resolve(strict=True)
    build = args.build_dir.resolve()
    cargo = (args.cargo_target or build / "cargo").resolve()
    if build == ROOT or build in ROOT.parents:
        parser.error("Build directory must be a dedicated output directory")
    qmake = qt / "bin" / ("qmake.exe" if os.name == "nt" else "qmake")
    version = subprocess.check_output([str(qmake), "-query", "QT_VERSION"], text=True).strip()
    if version != QT_VERSION:
        parser.error(f"Expected Qt {QT_VERSION}; selected SDK reports {version}")
    env = os.environ.copy()
    env["PATH"] = str(qt / "bin") + os.pathsep + env["PATH"]
    config = [args.cmake, "-S", ROOT / "bins/cords-client/qt", "-B", build,
        f"-DCMAKE_PREFIX_PATH={qt.as_posix()}", f"-DCORDS_CARGO_TARGET={cargo.as_posix()}",
        f"-DCORDS_CARGO_PROFILE={args.profile}", "-DCMAKE_BUILD_TYPE=Release",
        "-DCORDS_ALLOW_SOURCE_MIGRATIONS=OFF"]
    if os.name == "nt":
        config += ["-G", "Visual Studio 17 2022", "-A", "x64"]
    run(config, env)
    run([args.cmake, "--build", build, "--config", "Release", "--parallel", args.jobs], env)
    if args.tests:
        run([args.cmake, "--build", build, "--config", "Release", "--target", "Cords_qmllint"], env)
        ctest = Path(args.cmake).with_name("ctest.exe" if os.name == "nt" else "ctest")
        run([ctest, "--test-dir", build, "-C", "Release", "--output-on-failure"], env)
    if args.package_dir:
        package = args.package_dir.resolve()
        if package == ROOT or package in ROOT.parents or package == build or package in build.parents:
            parser.error("Package directory must be a dedicated install directory")
        marker = package / "cords-native-build.json"
        if package.exists() and any(package.iterdir()) and not marker.is_file():
            parser.error("Refusing to overlay an unrelated nonempty package directory")
        package.mkdir(parents=True, exist_ok=True)
        manifest = {"qt": QT_VERSION, "profile": args.profile, "platform": sys.platform,
            "build": str(build), "status": "installing"}
        marker.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        run([args.cmake, "--install", build, "--config", "Release", "--prefix", package], env)
        manifest["status"] = "installed"
        marker.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        print(f"Native package installed at {package}. Platform runtime acceptance is a separate gate.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, subprocess.SubprocessError) as error:
        print(f"Native client build failed: {error}", file=sys.stderr)
        sys.exit(1)
