#!/usr/bin/env python3
"""Exercise Unix builder failures without compiling or installing dependencies."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipIf(os.name == "nt", "Requires Unix Bash")
class BuildRequirements(unittest.TestCase):
    def test_cross_builder_missing_cmake_reports_full_setup(self):
        with tempfile.TemporaryDirectory() as temporary:
            tools = Path(temporary)
            (tools / "dirname").symlink_to(shutil.which("dirname"))
            for name, body in (("uname", "echo Linux"), ("cargo", "exit 99"),
                               ("cargo-xwin", "exit 99"), ("python3", "exit 99")):
                path = tools / name
                path.write_text("#!/bin/sh\n" + body + "\n")
                path.chmod(0o755)
            script = ROOT / "build-scripts/linux-windows-client-crosscomp.sh"
            result = subprocess.run([shutil.which("bash"), str(script)],
                                    env=dict(os.environ, PATH=str(tools)),
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn("required command 'cmake' is unavailable", result.stderr)
            self.assertIn("Requirements for linux-windows-client-crosscomp.sh:", result.stderr)
            self.assertIn("pip install cmake ninja cargo-xwin==0.23.1", result.stderr)
            self.assertIn("CORDS_QT_WINDOWS_PREFIX", result.stderr)

    def test_every_unix_entry_point_reports_its_requirements(self):
        scripts = sorted((ROOT / "build-scripts").glob("*.sh"))
        self.assertEqual(len(scripts), 7)
        for script in scripts:
            with self.subTest(script=script.name), tempfile.TemporaryDirectory() as temporary:
                tools = Path(temporary)
                for name in ("dirname", "awk"):
                    (tools / name).symlink_to(shutil.which(name))
                uname = tools / "uname"
                host = "Darwin" if script.name.startswith("mac-") else "Linux"
                uname.write_text(f"#!/bin/sh\necho {host}\n")
                uname.chmod(0o755)
                env = dict(os.environ, PATH=str(tools), CORDS_DIST_DIR=str(tools / "dist"))
                result = subprocess.run([shutil.which("bash"), str(script)], env=env,
                                        capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                requirements = ROOT / "build-scripts/requirements" / (script.name + ".txt")
                self.assertIn(f"Requirements for {script.name}:", result.stderr)
                self.assertIn(requirements.read_text().strip(), result.stderr)
                self.assertIn("Full setup and controls:", result.stderr)
                self.assertFalse((tools / "dist").exists())

    def test_external_failure_preserves_status_and_exit_cleanup(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shutil.copytree(ROOT / "build-scripts", root / "build-scripts")
            script = root / "build-scripts/linux-client-tar.sh"
            marker = root / "cleanup"
            script.write_text('''#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/lib/common.sh"
trap 'printf cleaned > "$MARKER"' EXIT
bash -c 'exit 37'
''')
            result = subprocess.run(["bash", str(script)], env=dict(os.environ, MARKER=str(marker)),
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 37)
            self.assertEqual(marker.read_text(), "cleaned")
            self.assertEqual(result.stderr.count("Requirements for linux-client-tar.sh:"), 1)


if __name__ == "__main__":
    unittest.main()
