#!/usr/bin/env python3
"""Validate checked-in public pins. Never fetch keys or read private credentials."""

import argparse
import hashlib
import json
import re
import subprocess
import tempfile
import time
from pathlib import Path

KEYS = Path(__file__).resolve().parent.parent / ".forgejo" / "release-keys"


def validate(keys=KEYS):
    config = json.loads((keys / "identity.json").read_text())
    fingerprint = config["gpg_primary_fingerprint"]
    if config["version"] != 1 or not re.fullmatch(r"(?:[0-9A-F]{40}|[0-9A-F]{64})", fingerprint):
        raise ValueError("invalid release identity configuration")
    paths = [keys / "gpg.pub", keys / "cosign.pub"]
    for path, field in zip(paths, ("gpg_sha256", "cosign_sha256"), strict=True):
        expected = config[field]
        if not re.fullmatch(r"[0-9a-f]{64}", expected):
            raise ValueError(f"invalid {field} pin")
        if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError(f"{path.name} differs from checked-in byte pin; reviewed rotation required")
    for field in ("gpg_source_url", "gpg_resolved_url", "cosign_source_url"):
        if not config[field].startswith("https://"):
            raise ValueError(f"invalid provenance URL: {field}")
    if not paths[1].read_bytes().startswith(b"-----BEGIN PUBLIC KEY-----"):
        raise ValueError("Cosign pin must be a PEM public key")
    subprocess.run(["openssl", "pkey", "-pubin", "-in", str(paths[1]), "-pubcheck", "-noout"],
                   check=True, capture_output=True)
    with tempfile.TemporaryDirectory(prefix="cords-public-identity-") as home:
        command = ["gpg", "--no-options", "--homedir", home, "--batch", "--no-auto-key-retrieve"]
        packets = subprocess.run(command + ["--list-packets", str(paths[0])], check=True,
                                 capture_output=True, text=True).stdout
        if ":secret key packet:" in packets or ":secret sub key packet:" in packets:
            raise ValueError("public GPG pin contains private-key packets")
        subprocess.run(command + ["--import", str(paths[0])], check=True, capture_output=True)
        listed = subprocess.run(command + ["--with-colons", "--list-keys"], check=True,
                                capture_output=True, text=True).stdout
        rows = [line.split(":") for line in listed.splitlines()]
        primary = [row for row in rows if row[0] == "pub"]
        fingerprints = [row[9] for i, row in enumerate(rows)
                        if row[0] == "fpr" and i > 0 and rows[i - 1][0] == "pub"]
        if len(primary) != 1 or fingerprints != [fingerprint] or primary[0][1] in ("r", "e", "d"):
            raise ValueError("GPG public primary identity does not match the configured live fingerprint")
        now = int(time.time())
        if primary[0][6] and int(primary[0][6]) <= now:
            raise ValueError("GPG public primary is expired")
        signing = [row for row in rows if row[0] in ("pub", "sub") and "s" in row[11]
                   and row[1] not in ("r", "e", "d") and (not row[6] or int(row[6]) > now)]
        if not signing:
            raise ValueError("GPG certificate has no live signing key")
        checked = subprocess.run(command + ["--with-colons", "--check-sigs", fingerprint],
                                 check=True, capture_output=True, text=True).stdout
        signatures = [line.split(":") for line in checked.splitlines() if line.startswith("sig:")]
        self_signatures = [row for row in signatures if row[12] == fingerprint]
        if not self_signatures or any(row[1] != "!" for row in self_signatures):
            raise ValueError("GPG certificate self-signatures failed verification")
    return fingerprint, *paths


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fields", action="store_true", help="emit fingerprint and public paths for trusted scripts")
    args = parser.parse_args()
    try:
        fingerprint, gpg, cosign = validate()
        if args.fields:
            print(f"{fingerprint}\n{gpg}\n{cosign}")
        else:
            print(f"Verified checked-in GPG identity {fingerprint} and exact Cosign public pin")
    except (KeyError, ValueError, OSError, subprocess.CalledProcessError) as error:
        # Do not dump subprocess buffers or credential environments on errors.
        parser.exit(1, f"error: public release identity validation failed: {error}\n")


if __name__ == "__main__":
    main()
