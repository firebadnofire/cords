#!/usr/bin/env python3
"""Build both pinned local codec modules from verified/locked sources.

Run in Linux/WSL with Python, Git, CMake 3.31.6 and Ninja 1.11.1. The SDK and
verified source checkouts live in a caller-selected cache, outside tracked source.
All subprocess failures stop the build. Existing checkouts are never reset.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parent
LOCK = json.loads((ROOT / "dependencies.json").read_text(encoding="utf-8"))
WORKSPACE = ROOT.parents[1]


def run(*arguments, cwd=None, env=None, capture=False):
    print("+", " ".join(map(str, arguments)), flush=True)
    result = subprocess.run(
        [str(argument) for argument in arguments],
        cwd=cwd,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else None,
    )
    return result.stdout.strip() if capture else None


def checkout(cache, name):
    dependency = LOCK[name]
    source = cache / name
    if not source.exists():
        run("git", "init", source)
        run("git", "remote", "add", "origin", dependency["url"], cwd=source)
        run("git", "fetch", "--depth=1", "origin", dependency["revision"], cwd=source)
        run("git", "checkout", "--detach", "FETCH_HEAD", cwd=source)
    actual = run("git", "rev-parse", "HEAD", cwd=source, capture=True)
    if actual != dependency["revision"]:
        raise RuntimeError(f"{source}: expected {dependency['revision']}, found {actual}; use a fresh cache")
    run("git", "diff", "--exit-code", "HEAD", cwd=source)
    for path, revision in dependency.get("submodules", {}).items():
        run("git", "submodule", "update", "--init", "--depth=1", "--", path, cwd=source)
        actual = run("git", "rev-parse", "HEAD", cwd=source / path, capture=True)
        if actual != revision:
            raise RuntimeError(f"{source / path}: submodule revision mismatch")
        run("git", "diff", "--exit-code", "HEAD", cwd=source / path)
    return source


def build_source(cache, output):
    # The workspace toolchain is pinned separately; never silently select another
    # compiler or update the application lockfile while building codec assets.
    rust = LOCK["rust"]
    run("rustup", "run", rust, "rustc", "--version")
    run("rustup", "target", "add", "--toolchain", rust, "wasm32-unknown-unknown")
    cli = cache / "rust-tools" / "bin" / "wasm-bindgen"
    if not cli.exists():
        run("rustup", "run", rust, "cargo", "install", "wasm-bindgen-cli", "--locked",
            "--version", LOCK["wasm-bindgen"], "--root", cache / "rust-tools")
    actual = run(cli, "--version", capture=True)
    if actual != f"wasm-bindgen {LOCK['wasm-bindgen']}":
        raise RuntimeError(f"Unexpected binding generator: {actual}; use a fresh cache")
    environment = dict(os.environ)
    target = cache / "source-target"
    environment["CARGO_TARGET_DIR"] = str(target)
    environment["SOURCE_DATE_EPOCH"] = "1751400000"
    run("rustup", "run", rust, "cargo", "rustc", "-p", "cords-avatar-source", "--locked",
        "--release", "--target", "wasm32-unknown-unknown", "--",
        f"--remap-path-prefix={WORKSPACE}=/cords",
        f"--remap-path-prefix={cache}=/codec-cache",
        "-C", "link-arg=--max-memory=134217728",
        "-C", "link-arg=--initial-memory=16777216", cwd=WORKSPACE, env=environment)
    destination = output / "source"
    destination.mkdir(parents=True, exist_ok=True)
    run(cli, target / "wasm32-unknown-unknown/release/cords_avatar_source.wasm",
        "--target", "web", "--out-dir", destination, "--no-typescript")
    (destination / "package.json").write_text('{"type":"module"}\n', encoding="utf-8")


def source_hashes():
    files = [WORKSPACE / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")]
    files += list((WORKSPACE / "crates/cords-avatar-source").rglob("*.rs"))
    files += [WORKSPACE / "crates/cords-avatar-source/Cargo.toml"]
    files += [ROOT / name for name in ("build.py", "bridge.cc", "fixtures.cc", "CMakeLists.txt", "coalescing.cmake", "dependencies.json")]
    return {str(path.relative_to(WORKSPACE)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(files)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT.parents[1] / "target" / "avatar-codec")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--fixtures", action="store_true", help="Build the validation-only layer fixture generator")
    options = parser.parse_args()
    if options.jobs < 1 or options.jobs > 16:
        parser.error("--jobs must be between 1 and 16")
    if sys.platform == "win32":
        parser.error("Run this source build in Linux/WSL; the emitted module is platform-independent")
    for program, version in (("cmake", LOCK["cmake"]), ("ninja", LOCK["ninja"])):
        if not shutil.which(program):
            raise RuntimeError(f"Install {program} {version} in an isolated build-tools environment")
        actual = run(program, "--version", capture=True).splitlines()[0]
        if version not in actual:
            raise RuntimeError(f"Expected {program} {version}, found {actual}")
    cache = options.cache.resolve()
    output = options.output.resolve()
    if output == ROOT or output in ROOT.parents or output == cache:
        raise RuntimeError("Output must be a dedicated build directory")
    cache.mkdir(parents=True, exist_ok=True)
    inputs = source_hashes()
    sdk = checkout(cache, "emsdk")
    run(sys.executable, sdk / "emsdk.py", "install", LOCK["emsdk"]["version"], cwd=sdk)
    run(sys.executable, sdk / "emsdk.py", "activate", LOCK["emsdk"]["version"], cwd=sdk)
    source = checkout(cache, "libjxl")
    # emsdk's env shell is trusted only after its commit and tracked files pass checks.
    environment_json = run(
        "bash", "-c", 'source "$1/emsdk_env.sh" >/dev/null; "$2" -c "import json,os; print(json.dumps(dict(os.environ)))"',
        "cords-sdk-env", sdk, sys.executable, capture=True,
    )
    environment = json.loads(environment_json)
    environment["SOURCE_DATE_EPOCH"] = "1751400000"
    compiler = run("emcc", "--version", env=environment, capture=True)
    if LOCK["emsdk"]["version"] not in compiler.splitlines()[0]:
        raise RuntimeError("The activated Emscripten compiler does not match the lock")
    output.mkdir(parents=True, exist_ok=True)
    run("emcmake", "cmake", "-S", ROOT, "-B", output, "-G", "Ninja",
        "-DCMAKE_BUILD_TYPE=Release", f"-DCORDS_LIBJXL_SOURCE={source}", env=environment)
    run("cmake", "--build", output, "--target", "cords-jxl", "--parallel", options.jobs, env=environment)
    if options.fixtures:
        run("cmake", "--build", output, "--target", "cords-fixtures", "--parallel", options.jobs, env=environment)
    build_source(cache, output)
    if source_hashes() != inputs:
        raise RuntimeError("Codec inputs changed during the build; rerun to produce an accurate manifest")
    assets = {}
    for name in ("cords-jxl.mjs", "cords-jxl.wasm", "source/cords_avatar_source.js",
                 "source/cords_avatar_source_bg.wasm", "source/package.json"):
        payload = (output / name).read_bytes()
        assets[name] = {"bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
    manifest = {"dependencies": LOCK, "sources": inputs, "assets": assets}
    (output / "cords-jxl.manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(assets, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Avatar codec build failed: {error}", file=sys.stderr)
        sys.exit(1)
