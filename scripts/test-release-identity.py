#!/usr/bin/env python3
"""Validate public pins and reject altered configuration using real GPG/OpenSSL."""

import importlib.util
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("identity", Path(__file__).with_name("release-identity.py"))
identity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(identity)


class PublicIdentity(unittest.TestCase):
    def test_authoritative_identity(self):
        fingerprint, _, _ = identity.validate()
        self.assertEqual(fingerprint, "7D6EF134D851C8DA0862D97494F31AF374E2EE3C")

    def test_wrong_fingerprint(self):
        with tempfile.TemporaryDirectory() as root:
            keys = Path(root) / "keys"
            shutil.copytree(identity.KEYS, keys)
            config = json.loads((keys / "identity.json").read_text())
            config["gpg_primary_fingerprint"] = "A" * 40
            (keys / "identity.json").write_text(json.dumps(config))
            with self.assertRaises(ValueError):
                identity.validate(keys)

    def test_altered_key_bytes(self):
        for name in ("gpg.pub", "cosign.pub"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as root:
                keys = Path(root) / "keys"
                shutil.copytree(identity.KEYS, keys)
                with (keys / name).open("ab") as key:
                    key.write(b"changed")
                with self.assertRaises(ValueError):
                    identity.validate(keys)

    def test_bad_public_key_format(self):
        import hashlib

        with tempfile.TemporaryDirectory() as root:
            keys = Path(root) / "keys"
            shutil.copytree(identity.KEYS, keys)
            (keys / "cosign.pub").write_bytes(b"-----BEGIN PUBLIC KEY-----\ninvalid\n-----END PUBLIC KEY-----\n")
            config = json.loads((keys / "identity.json").read_text())
            config["cosign_sha256"] = hashlib.sha256((keys / "cosign.pub").read_bytes()).hexdigest()
            (keys / "identity.json").write_text(json.dumps(config))
            with self.assertRaises(subprocess.CalledProcessError):
                identity.validate(keys)

    def test_missing_public_configuration(self):
        with tempfile.TemporaryDirectory() as root, self.assertRaises(OSError):
            identity.validate(Path(root))


if __name__ == "__main__":
    unittest.main()
