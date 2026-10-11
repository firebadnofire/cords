#!/usr/bin/env python3
"""Prepared dependency-graph checks; no build, dispatch or publication."""

import copy
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
BUILD_JOBS = ("client-linux", "client-linux-arm64", "client-windows", "client-windows-arm64",
              "server-linux", "server-linux-arm64", "client-macos", "client-macos-pkg", "server-macos", "oci-build")


def gated(jobs):
    return all(jobs.get(name, {}).get("needs") == ["mandatory-tests"] for name in BUILD_JOBS)


class ReleaseGraph(unittest.TestCase):
    def setUp(self):
        self.source = (ROOT / ".forgejo/workflow-proposals/release-builds.yml").read_text()
        self.workflow = yaml.safe_load(self.source)
        self.jobs = self.workflow["jobs"]

    def test_every_build_depends_on_full_gate(self):
        self.assertTrue(gated(self.jobs))
        mutated = copy.deepcopy(self.jobs)
        mutated["client-macos"]["needs"] = ["identity"]
        self.assertFalse(gated(mutated))
        self.assertEqual(set(self.jobs["mandatory-tests"]["needs"]),
                         {"identity", "rust-and-frontend", "containers", "runtime-acceptance"})
        self.assertEqual(self.jobs["mandatory-tests"]["if"], "${{ always() }}")
        body = self.jobs["mandatory-tests"]["steps"][0]["run"]
        for name in self.jobs["mandatory-tests"]["needs"]:
            self.assertIn('test "${GATE_' + name.replace("-", "_").upper() + '}" = success', body)

    def test_missing_runtime_and_executor_evidence_blocks(self):
        self.assertEqual(self.jobs["runtime-acceptance"]["if"], "${{ false }}")
        for name in ("client-macos", "client-macos-pkg", "server-macos",
                     "client-linux-arm64", "client-windows-arm64", "server-linux-arm64"):
            self.assertEqual(self.jobs[name]["if"], "${{ false }}")

    def test_no_privileged_path_and_all_targets_preserved(self):
        self.assertEqual(self.workflow["on"], {"push": {"tags": ["v*", "V*"]}})
        for forbidden in ("secrets.", "docker login", "docker push", "--push", "publish-container.sh",
                          "sign-release-artifacts.sh", "publish-release-assets.sh"):
            self.assertNotIn(forbidden, self.source)
        for script in ("linux-client-tar.sh", "linux-server-tar.sh", "linux-windows-client-crosscomp.sh",
                       "mac-client-dmg.sh", "mac-client-pkg.sh", "mac-server-tar.sh"):
            self.assertIn(script, self.source)
            self.assertTrue((ROOT / "build-scripts" / script).is_file())
        self.assertIn("type=oci", self.source)
        self.assertIn("linux/arm64", self.source)


if __name__ == "__main__":
    unittest.main()
