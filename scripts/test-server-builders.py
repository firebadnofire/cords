#!/usr/bin/env python3
"""Exercise server packaging with a fake compiler; does not prove native builds."""
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipIf(os.name == "nt", "Requires Unix shell and packaging tools")
class ServerBuilders(unittest.TestCase):
    def test_custom_target_directory_and_repeat_packaging(self):
        for host, script, platform in (
            ("Linux", "linux-server-tar.sh", "linux"),
            ("Darwin", "mac-server-tar.sh", "macos"),
        ):
            with self.subTest(host=host), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                for directory in ("build-scripts", "migrations/postgres", "assets/avatar-codec/licenses"):
                    shutil.copytree(ROOT / directory, root / directory)
                for filename in ("Cargo.toml", "README.md", "LICENSE"):
                    shutil.copy2(ROOT / filename, root / filename)
                tools = root / "tools"
                tools.mkdir()
                commands = {
                    "uname": f'if [ "$1" = -s ]; then echo {host}; else echo x86_64; fi',
                    "xcrun": "exit 0",
                    "cargo": '''test "$1" = build && test "$2" = --locked && test "$3" = --release
test "$4" = --target-dir
mkdir -p "$5/release"
printf '#!/bin/sh\\nexit 0\\n' > "$5/release/cords-server"
''',
                }
                for name, body in commands.items():
                    path = tools / name
                    path.write_text("#!/bin/sh\nset -eu\n" + body + "\n")
                    path.chmod(0o755)
                env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                           CARGO_TARGET_DIR=str(root / "custom cargo"),
                           CORDS_DIST_DIR=str(root / "dist"))
                for _ in range(2):
                    subprocess.run(["bash", str(root / "build-scripts" / script)],
                                   cwd=root, env=env, check=True, capture_output=True, text=True)
                    archive, = (root / "dist").glob("*.tar.gz")
                    subprocess.run(["sha256sum", "--check", archive.name + ".sha256"],
                                   cwd=archive.parent, check=True, capture_output=True)
                    with tarfile.open(archive) as package:
                        names = package.getnames()
                        prefix = f"cords-server-{platform}-x86_64/"
                        self.assertIn(prefix + "cords-server", names)
                        self.assertTrue(any(name.startswith(prefix + "migrations/postgres/") for name in names))


if __name__ == "__main__":
    unittest.main()
