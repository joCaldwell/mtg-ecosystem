import pathlib
import tempfile
import unittest

from audit import compare, sha


class ComparisonTest(unittest.TestCase):
    def test_transitions_diagnostics_and_incomplete_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = pathlib.Path(directory) / "manifest.json"
            raw = b'{"cards":[{"oracle_id":"b"},{"oracle_id":"a"}]}'
            manifest.write_bytes(raw)
            row = dict(status="unsupported", path="p", reason="r", exit_code=1)
            before = dict(format_version=1, source_commit="1" * 40,
                          target_manifest_sha256=sha(raw),
                          records=[dict(row, oracle_id=k) for k in ("b", "a")])
            after = dict(before, source_commit="2" * 40, records=[
                dict(row, oracle_id="a", reason="changed"),
                dict(oracle_id="b", status="catalog_error", exit_code=2)])
            changes = compare(before, after, manifest)["changes"]
            self.assertEqual([r["oracle_id"] for r in changes], ["a", "b"])
            self.assertEqual(changes[0]["after"]["reason"], "changed")
            self.assertEqual(changes[1]["after"]["status"], "catalog_error")
            for report in (before, after):
                report["records"] = [r for r in report["records"] if r["oracle_id"] == "a"]
            with self.assertRaisesRegex(ValueError, "inventory"):
                compare(before, after, manifest)
