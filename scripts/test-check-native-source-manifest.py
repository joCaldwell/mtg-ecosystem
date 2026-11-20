"""Exercise commit selection with a dirty tracked source and a second commit."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class ManifestTest(unittest.TestCase):
    def test_commit_bytes(self):
        checker = Path(__file__).with_name("check-native-source-manifest.py").resolve()
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)

            def git(*args):
                return subprocess.check_output(["git", "-C", repo, *args]).decode().strip()

            def commit():
                git("-c", "user.name=Test", "-c", "user.email=test@example.org",
                    "-c", "commit.gpgsign=false", "commit", "-qam", "fixture")
                return git("rev-parse", "HEAD")

            git("init", "-q")
            source = repo / "source.rs"
            source.write_bytes(b"old")
            git("add", "source.rs")
            first = commit()
            row = dict(path="source.rs", bytes=3, sha256=hashlib.sha256(b"old").hexdigest())

            def run(revision, rows):
                manifest = repo / "manifest.json"
                manifest.write_text(json.dumps(dict(status="compiled-source-closure-verified",
                                                    build_id="fixture", files=rows)))
                return subprocess.run([sys.executable, checker, manifest, "--repo", repo,
                                       "--commit", revision], capture_output=True, text=True)

            source.write_bytes(b"new")
            result = run(first, [row])
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f"{first} matches 1 declared files (3 bytes)", result.stdout)
            second = commit()
            source.write_bytes(b"old")
            for revision, rows, message in [
                (second, [row], "sha256 mismatch"),
                (first, [row, row], "duplicate path"),
                (first, [dict(row, path="../source.rs")], "invalid path"),
                (first, [dict(row, bytes=4)], "byte count mismatch"),
            ]:
                result = run(revision, rows)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)


if __name__ == "__main__":
    unittest.main()
