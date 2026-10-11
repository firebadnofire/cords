#!/usr/bin/env python3
"""Validate untrusted candidate shape/bytes only. NEVER authorizes signing."""

import argparse
import hashlib
import importlib.util
import json
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("release_contract", ROOT / "scripts/release-contract.py")
CONTRACT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTRACT)
MAX_JSON = 1024 * 1024
SHA = re.compile(r"[0-9a-f]{64}")
COMMIT = re.compile(r"[0-9a-f]{40}")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def object_keys(value, names, context):
    require(isinstance(value, dict) and set(value) == set(names), f"unexpected {context} fields")


def unique_object(pairs):
    result = {}
    for name, value in pairs:
        require(name not in result, "duplicate JSON field")
        result[name] = value
    return result


def load(path):
    with path.open("rb") as source:
        data = source.read(MAX_JSON + 1)
    require(len(data) <= MAX_JSON, "JSON exceeds intake bound")
    return json.loads(data, object_pairs_hook=unique_object)


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def positive_integer(value):
    return type(value) is int and 0 < value <= 2**63 - 1


def sha(value):
    return isinstance(value, str) and SHA.fullmatch(value) is not None


def evidence(rows, expected, commit):
    require(isinstance(rows, list) and len(rows) == len(expected), "incomplete evidence inventory")
    seen = set()
    for row in rows:
        object_keys(row, ("id", "run_id", "job_id", "attempt", "commit", "evidence_sha256"), "evidence")
        require(isinstance(row["id"], str) and row["id"] in expected and row["id"] not in seen,
                "duplicate or unexpected evidence identifier")
        seen.add(row["id"])
        require(row["commit"] == commit, "evidence commit differs")
        require(all(positive_integer(row[key]) for key in ("run_id", "job_id", "attempt")),
                "invalid evidence reference")
        require(sha(row["evidence_sha256"]), "invalid evidence digest")


def redirected(path):
    return path.is_symlink() or (hasattr(path, "is_junction") and path.is_junction())


def validate(request, directory):
    policy_path = ROOT / ".forgejo/release-policy.json"
    identity_path = ROOT / ".forgejo/release-keys/identity.json"
    policy = load(policy_path)
    pins = load(identity_path)
    object_keys(request, ("schema", "origin", "repository", "tag", "commit", "policy_sha256",
                          "signing_identity_sha256", "tests", "builds", "files", "oci"), "request")
    require(type(request["schema"]) is int and request["schema"] == 1, "unsupported request schema")
    require(request["origin"] == policy["canonical_origin"] and request["repository"] == policy["repository"],
            "noncanonical source identity")
    require(isinstance(request["tag"], str), "invalid tag type")
    version = CONTRACT.version(request["tag"])
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    require(version == workspace, "tag differs from workspace version")
    require(isinstance(request["commit"], str) and COMMIT.fullmatch(request["commit"]), "invalid exact commit")
    require(request["policy_sha256"] == digest(policy_path), "candidate policy differs")
    require(request["signing_identity_sha256"] == digest(identity_path), "candidate signing pins differ")
    for name, key in (("gpg.pub", "gpg_sha256"), ("cosign.pub", "cosign_sha256")):
        require(digest(ROOT / ".forgejo/release-keys" / name) == pins[key], "checked-in public key differs")
    evidence(request["tests"], policy["test_checks"], request["commit"])
    evidence(request["builds"], list(policy["packages"]) + ["oci-amd64", "oci-arm64"], request["commit"])

    packages = {name.format(version=version) for name in policy["packages"].values()}
    expected = packages | {name + ".sha256" for name in packages} | {"cosign.pub", "release-provenance.json"}
    rows = request["files"]
    require(isinstance(rows, list) and len(rows) == len(expected), "incomplete file inventory")
    require(directory.is_dir() and not redirected(directory), "unsafe intake directory")
    entries = list(directory.iterdir())
    require(len(entries) == len(expected) and {path.name for path in entries} == expected,
            "unexpected or missing intake entries")
    require(all(path.is_file() and not redirected(path) for path in entries), "unsafe intake entry")
    seen = set()
    total = 0
    for row in rows:
        object_keys(row, ("name", "size", "sha256"), "file")
        name = row["name"]
        require(isinstance(name, str) and name in expected and name not in seen, "duplicate or unsafe file name")
        seen.add(name)
        require(positive_integer(row["size"]) and row["size"] <= 8 * 1024**3 and sha(row["sha256"]),
                "invalid file size/digest")
        total += row["size"]
        require(total <= 32 * 1024**3, "candidate exceeds intake bound")
        path = directory / name
        require(path.stat().st_size == row["size"] and digest(path) == row["sha256"], "file bytes differ")
    for name in packages:
        checksum = directory / (name + ".sha256")
        require(checksum.stat().st_size < 256 and checksum.read_bytes() == CONTRACT.checksum(directory / name),
                "invalid package checksum")
    require(digest(directory / "cosign.pub") == pins["cosign_sha256"], "substituted Cosign public key")

    rows = request["oci"]
    require(isinstance(rows, list) and len(rows) == len(policy["oci_destinations"]), "incomplete OCI destinations")
    seen = set()
    digests = set()
    for row in rows:
        object_keys(row, ("image", "digest", "platforms"), "OCI")
        require(isinstance(row["image"], str) and row["image"] in policy["oci_destinations"]
                and row["image"] not in seen, "duplicate or unexpected OCI destination")
        seen.add(row["image"])
        require(isinstance(row["digest"], str) and row["digest"].startswith("sha256:")
                and sha(row["digest"][7:]), "invalid OCI digest")
        digests.add(row["digest"])
        require(row["platforms"] == policy["oci_platforms"], "incomplete OCI platform inventory")
    require(len(digests) == 1, "OCI mirror manifest digests differ")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
    try:
        validate(load(args.request), args.artifacts)
    except (ValueError, OSError, TypeError, RecursionError) as error:
        # Input contents may be sensitive; don't print their values or decoder context.
        parser.exit(1, f"error: candidate validation failed ({type(error).__name__}); signing unauthorized\n")
    print("CANDIDATE SHAPE/BYTES VALID ONLY: authorization, test results and provenance remain UNVERIFIED; publication DISABLED")


if __name__ == "__main__":
    main()
