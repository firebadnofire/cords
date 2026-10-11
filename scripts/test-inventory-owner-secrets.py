#!/usr/bin/env python3
"""Supplementary static scanner fixtures; no production credentials."""

import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("inventory", Path(__file__).with_name("inventory-owner-secrets.py"))
inventory = importlib.util.module_from_spec(spec)
spec.loader.exec_module(inventory)


class InventoryTests(unittest.TestCase):
    def test_property_and_bracket(self):
        result = inventory.references("${{ secrets.CI_KEY_B64 }}\n${{ secrets['GH_KEY'] }}")
        self.assertEqual(result["direct"], [{"name": "CI_KEY_B64", "line": 1}, {"name": "GH_KEY", "line": 2}])
        self.assertFalse(result["dynamic_context_lines"])

    def test_dynamic_and_inheritance(self):
        result = inventory.references("${{ secrets[inputs.name] }}\nsecrets: inherit\n${{ toJSON(secrets) }}")
        self.assertEqual(result["dynamic_context_lines"], [1, 2, 3])
        self.assertTrue(result["inherits_all"])

    def test_no_secret_values_in_output(self):
        result = inventory.references('value: DO_NOT_PRINT_THIS_SECRET\nkey: ${{ secrets.COSIGN_B64 }}')
        self.assertNotIn("DO_NOT_PRINT_THIS_SECRET", str(result))

    def test_action_credentials_redacted(self):
        result = inventory.references('uses: https://user:DO_NOT_PRINT@host/action@main')
        self.assertEqual(result["uses"], ["UNRESOLVED_OR_REDACTED_REFERENCE"])

    def test_comment_conservative(self):
        result = inventory.references('# old ${{ secrets.CI_KEY }}\nuses: ./local-action')
        self.assertEqual(result["direct"][0]["name"], "CI_KEY")
        self.assertEqual(result["uses"], ["./local-action"])


if __name__ == "__main__":
    unittest.main()
