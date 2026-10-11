#!/usr/bin/env python3
"""Supplementary offline contracts; fake assets are not package build evidence."""

import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("contract", Path(__file__).with_name("release-contract.py"))
contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(contract)


class ReleaseContracts(unittest.TestCase):
    def test_semver(self):
        for tag in ("v0.1.0", "V1.2.3-beta.1", "v1.0.0-0", "v1.0.0-alpha-01", "v1.0.0+001.build"):
            with self.subTest(tag=tag):
                self.assertEqual(contract.version(tag), tag[1:])
        for tag in ("v01.0.0", "v1.02.0", "v1.0.00", "v1.0.0-01", "v1.0.0-beta.01",
                    "v1.0.0-", "v1.0.0+", "v1.0.0-a..b", "v1.0", "1.0.0", "build-abc",
                    "v1.0.0\n", "v1.0.0/other", "v1.0.0-ä"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                contract.version(tag)

    def test_inventory_and_checksum(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            package = directory / "client-1.0.0+build.pkg"
            package.write_bytes(b"fixture")
            (directory / f"{package.name}.sha256").write_bytes(contract.checksum(package))
            expected = contract.manifest(directory)
            (directory / "SHA256SUMS").write_bytes(expected)
            for name in (package.name, f"{package.name}.sha256", "SHA256SUMS"):
                (directory / f"{name}.asc").write_bytes(b"cryptographic verification is separate")
            contract.verify_files(directory)
            self.assertEqual(contract.manifest(directory), expected)
            package.write_bytes(b"changed")
            with self.assertRaises(ValueError):
                contract.verify_files(directory)

    def test_no_inventory_omission(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "client.zip").write_bytes(b"fixture")
            (directory / "SHA256SUMS").write_bytes(b"")
            with self.assertRaises(ValueError):
                contract.verify_files(directory)

    def test_reject_unsafe_or_unexpected_inputs(self):
        for name in ("SHA256SUMS.sig", "private.gpg", "filename with space.zip", "orphan.zip.asc"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as root:
                directory = Path(root)
                (directory / "good.zip").write_bytes(b"fixture")
                (directory / name).write_bytes(b"unexpected")
                with self.assertRaises(ValueError):
                    contract.inputs(directory)

    def test_reject_checksum_path_escape(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "good.zip").write_bytes(b"fixture")
            (directory / "good.zip.sha256").write_bytes(b"0" * 64 + b"  ../good.zip\n")
            with self.assertRaises(ValueError):
                contract.inputs(directory)

    def test_reject_empty_and_directories(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            package = directory / "empty.zip"
            package.touch()
            with self.assertRaises(ValueError):
                contract.inputs(directory)
            package.unlink()
            package.mkdir()
            with self.assertRaises(ValueError):
                contract.inputs(directory)

    def test_signature_status(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "status"
            primary = "A" * 40
            path.write_text(f"[GNUPG:] VALIDSIG {'B' * 40} date 0 0 4 0 22 8 00 {primary}\n")
            contract.signature_status(primary, path)
            with self.assertRaises(ValueError):
                contract.signature_status("C" * 40, path)
            for rejected in ("BADSIG", "EXPKEYSIG", "REVKEYSIG", "EXPSIG", "KEYEXPIRED", "SIGEXPIRED"):
                with self.subTest(status=rejected):
                    path.write_text(f"[GNUPG:] VALIDSIG {primary}\n[GNUPG:] {rejected} {primary}\n")
                    with self.assertRaises(ValueError):
                        contract.signature_status(primary, path)


if __name__ == "__main__":
    unittest.main()
