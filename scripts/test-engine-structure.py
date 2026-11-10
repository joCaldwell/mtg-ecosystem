import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('structure', Path(__file__).with_name('engine-structure-check.py'))
structure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(structure)


class StructureTests(unittest.TestCase):
    def test_generated_and_binary_outputs_but_not_large_source_fixtures(self):
        self.assertIsNotNone(structure.artifact_reason('generated/run.json', 4, b'{}'))
        self.assertIsNotNone(structure.artifact_reason('saved/engine', 100, b'\x7fELF'))
        self.assertIsNotNone(structure.artifact_reason('generated/corpus.ndjson', 2**22, b'{}'))
        self.assertIsNone(structure.artifact_reason('tests/fixtures/source.json', 2**25, b'{}'))
        self.assertIsNone(structure.artifact_reason('src/engine.rs', 2**22, b'// source'))

    def test_index_reads_staged_bytes_not_worktree(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            subprocess.run(['git', 'init', '-q', tmp], check=True)
            (root / 'hidden').write_bytes(b'\x7fELFtest')
            subprocess.run(['git', '-C', tmp, 'add', 'hidden'], check=True)
            (root / 'hidden').write_text('now innocent')
            self.assertIn('executable', structure.artifact_errors(root)[0])

    def test_target_alias_cannot_hide_upper_layer(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'Cargo.toml').write_text('[workspace.dependencies]\n')
            for name in ['contracts', 'compiler', 'core']:
                path = root / structure.CRATES / name
                path.mkdir(parents=True)
                (path / 'Cargo.toml').write_text(f'[package]\nname="mtg-{name}"\n')
            path = root / structure.CRATES / 'core/Cargo.toml'
            path.write_text(path.read_text() + '\n[target.\'cfg(unix)\'.dependencies]\nconcealed = { package="mtg-compiler", path="../compiler" }\n')
            self.assertIn('mtg-compiler', structure.dependency_errors(root)[0])
            path.write_text('[package]\nname="mtg-core"\n[dev-dependencies]\nmtg-compiler = {path="../compiler"}\n')
            self.assertEqual([], structure.dependency_errors(root))


if __name__ == '__main__':
    unittest.main()
