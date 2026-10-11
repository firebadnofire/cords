#!/usr/bin/env python3
"""Offline release identity and byte contracts; no credentials or network."""

import argparse
import hashlib
import re
import sys
from pathlib import Path

NUMBER = r"(?:0|[1-9][0-9]*)"
PRE = rf"(?:{NUMBER}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
TAG = re.compile(rf"[vV]({NUMBER}\.{NUMBER}\.{NUMBER}(?:-{PRE}(?:\.{PRE})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)")
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*")


def version(tag):
    match = TAG.fullmatch(tag)
    if not match:
        raise ValueError("release tag must be v/V followed by strict SemVer 2.0")
    return match[1]


def checksum(path):
    with path.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    return f"{digest}  {path.name}\n".encode("ascii")


def inputs(directory):
    result = []
    for path in sorted(directory.iterdir(), key=lambda p: p.name):
        if path.is_symlink() or not path.is_file() or not NAME.fullmatch(path.name):
            raise ValueError(f"unsafe release entry: {path.name}")
        if path.name == "SHA256SUMS" or path.name.endswith(".asc"):
            continue
        if not (path.name.endswith((".tar.gz", ".zip", ".dmg", ".pkg", ".sha256"))
                or path.name in ("cosign.pub", "release-provenance.json")):
            raise ValueError(f"unexpected release input: {path.name}")
        if not path.stat().st_size:
            raise ValueError(f"empty release input: {path.name}")
        result.append(path)
    if not result:
        raise ValueError("no release inputs")
    names = {path.name for path in result} | {"SHA256SUMS"}
    for path in directory.glob("*.asc"):
        if path.name[:-4] not in names:
            raise ValueError(f"orphan signature: {path.name}")
    for path in result:
        if path.name.endswith(".sha256"):
            package = directory / path.name[:-7]
            if package.name not in names or path.read_bytes() != checksum(package):
                raise ValueError(f"invalid package checksum: {path.name}")
    return result


def manifest(directory):
    return b"".join(checksum(path) for path in inputs(directory))


def verify_files(directory):
    if (directory / "SHA256SUMS").read_bytes() != manifest(directory):
        raise ValueError("SHA256SUMS does not match exact inventory and bytes")
    for path in inputs(directory) + [directory / "SHA256SUMS"]:
        if not (directory / f"{path.name}.asc").is_file():
            raise ValueError(f"missing signature: {path.name}.asc")


def signature_status(fingerprint, path):
    records = [line.split() for line in path.read_text().splitlines()]
    valid = [row for row in records if len(row) > 2 and row[:2] == ["[GNUPG:]", "VALIDSIG"]]
    rejected = {"BADSIG", "ERRSIG", "EXPSIG", "EXPKEYSIG", "REVKEYSIG", "KEYEXPIRED", "SIGEXPIRED"}
    if (len(valid) != 1 or valid[0][-1].upper() != fingerprint.upper()
            or any(len(row) > 1 and row[1] in rejected for row in records)):
        raise ValueError("signature is not current and valid under pinned primary key")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("tag", "inputs", "manifest", "verify-files", "signature-status"))
    parser.add_argument("value")
    parser.add_argument("status", nargs="?")
    args = parser.parse_args()
    try:
        if args.operation == "tag":
            print(version(args.value))
        elif args.operation == "signature-status":
            if args.status is None:
                raise ValueError("signature status path is required")
            signature_status(args.value, Path(args.status))
        else:
            directory = Path(args.value)
            if args.operation == "inputs":
                print("\n".join(path.name for path in inputs(directory)))
            elif args.operation == "manifest":
                sys.stdout.buffer.write(manifest(directory))
            else:
                verify_files(directory)
    except (ValueError, OSError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
