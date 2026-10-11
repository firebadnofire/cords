#!/usr/bin/env python3
"""Hostile candidate fixtures, not authenticated builder/service acceptance."""

import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("request_validator", Path(__file__).with_name("validate-release-request.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name) / "assets"
        self.directory.mkdir()
        policy = CHECK.load(CHECK.ROOT / ".forgejo/release-policy.json")
        self.assertEqual(policy["activation"], "disabled")
        self.assertEqual(policy["credential_scope"], "external-executor-only")
        commit = "a" * 40

        def evidence(names):
            return [{"id": name, "run_id": 1, "job_id": index, "attempt": 1,
                     "commit": commit, "evidence_sha256": "b" * 64}
                    for index, name in enumerate(names, 1)]

        version = CHECK.tomllib.loads((CHECK.ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        for template in policy["packages"].values():
            path = self.directory / template.format(version=version)
            path.write_bytes(b"fake fixture; NOT a built package")
            path.with_name(path.name + ".sha256").write_bytes(CHECK.CONTRACT.checksum(path))
        (self.directory / "cosign.pub").write_bytes((CHECK.ROOT / ".forgejo/release-keys/cosign.pub").read_bytes())
        (self.directory / "release-provenance.json").write_text('{"fixture":"unverified evidence"}\n')
        self.request = {
            "schema": 1, "origin": policy["canonical_origin"], "repository": policy["repository"],
            "tag": "v" + version, "commit": commit,
            "policy_sha256": CHECK.digest(CHECK.ROOT / ".forgejo/release-policy.json"),
            "signing_identity_sha256": CHECK.digest(CHECK.ROOT / ".forgejo/release-keys/identity.json"),
            "tests": evidence(policy["test_checks"]),
            "builds": evidence(list(policy["packages"]) + ["oci-amd64", "oci-arm64"]),
            "files": [{"name": path.name, "size": path.stat().st_size, "sha256": CHECK.digest(path)}
                      for path in sorted(self.directory.iterdir())],
            "oci": [{"image": image, "digest": "sha256:" + "c" * 64, "platforms": policy["oci_platforms"]}
                    for image in policy["oci_destinations"]],
        }

    def rejects(self, mutate):
        candidate = copy.deepcopy(self.request)
        mutate(candidate)
        with self.assertRaises(ValueError):
            CHECK.validate(candidate, self.directory)

    def test_repeat_is_read_only_and_uppercase_tag(self):
        before = {path.name: path.read_bytes() for path in self.directory.iterdir()}
        CHECK.validate(self.request, self.directory)
        self.request["tag"] = "V" + self.request["tag"][1:]
        CHECK.validate(self.request, self.directory)
        self.assertEqual(before, {path.name: path.read_bytes() for path in self.directory.iterdir()})

    def test_no_self_authorization_or_commands(self):
        for key in ("authorized", "success", "command", "download_url"):
            with self.subTest(key=key):
                self.rejects(lambda row: row.update({key: True}))

    def test_source_and_policy_substitution(self):
        for key, value in (("origin", "https://evil.example"), ("repository", "firebadnofire/cords-publication"),
                           ("commit", "main"), ("tag", "v99.1.0"), ("tag", "build-abc"),
                           ("policy_sha256", "d" * 64), ("signing_identity_sha256", "d" * 64)):
            with self.subTest(key=key):
                self.rejects(lambda row: row.update({key: value}))

    def test_evidence_missing_duplicate_stale_and_invalid_reference(self):
        for field in ("tests", "builds"):
            self.rejects(lambda row: row[field].pop())
            self.rejects(lambda row: row[field].__setitem__(0, row[field][1]))
            self.rejects(lambda row: row[field][0].update(commit="e" * 40))
            self.rejects(lambda row: row[field][0].update(job_id=True))
            self.rejects(lambda row: row[field][0].update(authorized=True))

    def test_files_missing_duplicate_path_and_tamper(self):
        self.rejects(lambda row: row["files"].pop())
        self.rejects(lambda row: row["files"].__setitem__(0, row["files"][1]))
        self.rejects(lambda row: row["files"][0].update(name="../secret"))
        path = next(self.directory.glob("*.pkg"))
        path.write_bytes(b"tampered")
        with self.assertRaises(ValueError):
            CHECK.validate(self.request, self.directory)

    def test_extra_entry_and_directory(self):
        (self.directory / "secret").mkdir()
        with self.assertRaises(ValueError):
            CHECK.validate(self.request, self.directory)

    @unittest.skipIf(sys.platform == "win32", "real symlink fixture requires Linux host; Windows privilege not assumed")
    def test_symlink_entry_rejected(self):
        path = next(self.directory.glob("*.pkg"))
        original = path.read_bytes()
        target = Path(self.temp.name) / "outside.pkg"
        target.write_bytes(original)
        path.unlink()
        path.symlink_to(target)
        with self.assertRaises(ValueError):
            CHECK.validate(self.request, self.directory)

    def test_checksum_and_key_substitution_even_with_updated_digest(self):
        for name in (next(self.directory.glob("*.sha256")).name, "cosign.pub"):
            path = self.directory / name
            original = path.read_bytes()
            path.write_bytes(b"substituted bytes")
            candidate = copy.deepcopy(self.request)
            item = next(row for row in candidate["files"] if row["name"] == name)
            item.update(size=path.stat().st_size, sha256=CHECK.digest(path))
            with self.subTest(name=name), self.assertRaises(ValueError):
                CHECK.validate(candidate, self.directory)
            path.write_bytes(original)

    def test_oci_missing_destination_mutable_tag_and_platform(self):
        self.rejects(lambda row: row["oci"].pop())
        self.rejects(lambda row: row["oci"][0].update(image="evil.example/cords"))
        self.rejects(lambda row: row["oci"][0].update(digest="latest"))
        self.rejects(lambda row: row["oci"][0].update(digest="sha256:" + "e" * 64))
        self.rejects(lambda row: row["oci"][0].update(platforms=["linux/amd64"]))

    def test_bounded_json_and_duplicate_fields(self):
        path = Path(self.temp.name) / "request.json"
        for contents in ('{"schema":1,"schema":1}', " " * (CHECK.MAX_JSON + 1)):
            path.write_text(contents)
            with self.assertRaises(ValueError):
                CHECK.load(path)

    def test_cli_never_claims_authorization_and_does_not_leak_bad_values(self):
        path = Path(self.temp.name) / "request.json"
        command = [sys.executable, str(Path(CHECK.__file__)), "--request", str(path),
                   "--artifacts", str(self.directory)]
        path.write_text(json.dumps(self.request))
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("UNVERIFIED; publication DISABLED", result.stdout)
        path.write_text('{"DO_NOT_LOG_SECRET_MARKER":')
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("DO_NOT_LOG_SECRET_MARKER", result.stderr)


if __name__ == "__main__":
    unittest.main()
