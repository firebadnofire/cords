#!/usr/bin/env python3
"""Static stored-workflow gates; not enforcement against hostile live jobs."""

import json
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]


def publication_stopped(workflow):
    return bool(workflow.get("jobs")) and all(
        job.get("if") == "${{ false }}" and not job.get("uses")
        for job in workflow["jobs"].values())


class MonorepoGates(unittest.TestCase):
    def test_publication_disabled_and_tag_triggers_preserved(self):
        source = (ROOT / ".forgejo/workflows/release.yml").read_text()
        workflow = yaml.safe_load(source)
        self.assertEqual(workflow["on"]["push"]["tags"], ["v*", "V*"])
        self.assertTrue(publication_stopped(workflow))
        self.assertNotIn("secrets.", source)
        # A newly added active publisher must fail this static acceptance gate.
        workflow["jobs"]["injected-publisher"] = {"runs-on": "ubuntu-22.04"}
        self.assertFalse(publication_stopped(workflow))

    def test_no_direct_mac_dispatch_in_discovered_workflows(self):
        for path in (ROOT / ".forgejo/workflows").glob("*.yml"):
            source = path.read_text()
            self.assertNotIn("macos-latest", source)
            self.assertNotIn("arm-ubuntu-22.04", source)

    def test_expanded_ci_is_prepared_outside_discovery(self):
        path = ROOT / ".forgejo/workflow-proposals/ci.yml"
        source = path.read_text()
        workflow = yaml.safe_load(source)
        self.assertEqual(workflow["on"]["push"]["branches"], ["**"])
        self.assertIn("pull_request", workflow["on"])
        self.assertIn('test "$(git rev-parse HEAD)" = "${GITHUB_SHA}"', source)
        self.assertNotIn('${FORGEJO_REF:-refs/heads/main}', source)
        self.assertIn('cargo-deny --locked --version 0.20.2', source)
        self.assertTrue({"rust-and-frontend", "containers", "client-platform-check", "windows-cross-check"}
                        <= workflow["jobs"].keys())

    def test_single_repo_and_external_credential_policy(self):
        policy = json.loads((ROOT / ".forgejo/release-policy.json").read_text())
        self.assertEqual(policy["repository"], "firebadnofire/cords")
        self.assertEqual(policy["activation"], "disabled")
        self.assertEqual(policy["credential_scope"], "external-executor-only")
        self.assertEqual(len(policy["packages"]), 9)
        self.assertEqual(len(policy["oci_destinations"]), 2)


if __name__ == "__main__":
    unittest.main()
