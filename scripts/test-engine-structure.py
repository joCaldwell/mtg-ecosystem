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
            # Every reviewed layer from the live allowlist, oracle-ast included.
            for crate in structure.ALLOWED:
                name = crate.removeprefix('mtg-')
                path = root / structure.CRATES / name
                path.mkdir(parents=True)
                (path / 'Cargo.toml').write_text(f'[package]\nname="mtg-{name}"\n')
            path = root / structure.CRATES / 'core/Cargo.toml'
            path.write_text(path.read_text() + '\n[target.\'cfg(unix)\'.dependencies]\nconcealed = { package="mtg-compiler", path="../compiler" }\n')
            self.assertIn('mtg-compiler', structure.dependency_errors(root)[0])
            # Compiler-backed witnesses live in mtg-witness, never core dev-dependencies.
            path.write_text('[package]\nname="mtg-core"\n[dev-dependencies]\nmtg-compiler = {path="../compiler"}\n')
            self.assertIn('dev-dependencies mtg-compiler', structure.dependency_errors(root)[0])
            path.write_text('[package]\nname="mtg-core"\n[dev-dependencies]\nmtg-contracts = {path="../contracts"}\n')
            self.assertEqual([], structure.dependency_errors(root))
            # A dev-only validator is reviewed per layer, never as a runtime edge.
            contracts = root / structure.CRATES / 'contracts/Cargo.toml'
            contracts.write_text('[package]\nname="mtg-contracts"\n[dev-dependencies]\njsonschema = "1"\n')
            self.assertEqual([], structure.dependency_errors(root))
            contracts.write_text('[package]\nname="mtg-contracts"\n[dependencies]\njsonschema = "1"\n')
            self.assertIn('jsonschema', structure.dependency_errors(root)[0])
            contracts.write_text('[package]\nname="mtg-contracts"\n')
            # The generated AST layer may not reach upward into contracts either.
            ast = root / structure.CRATES / 'oracle-ast/Cargo.toml'
            ast.write_text('[package]\nname="mtg-oracle-ast"\n[dependencies]\nmtg-contracts = {path="../contracts"}\n')
            self.assertIn('oracle-ast', structure.dependency_errors(root)[0])

    def test_testkit_is_confined_to_test_crates(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            crates = {'core': 'mtg-core', 'sim': 'mtg-sim', 'witness': 'mtg-witness'}
            (root / 'Cargo.toml').write_text('[workspace]\nmembers = [' + ', '.join(
                f'"{structure.CRATES}/{d}"' for d in crates) + ']\n')
            for directory, name in crates.items():
                (root / structure.CRATES / directory).mkdir(parents=True)
            def write(directory, body):
                (root / structure.CRATES / directory / 'Cargo.toml').write_text(
                    f'[package]\nname="{crates[directory]}"\n' + body)
            write('core', '[features]\ntestkit = []\n')
            write('witness', '[dependencies]\nmtg-core = {path="../core", features=["testkit"]}\n')
            write('sim', '[dependencies]\nmtg-core = {path="../core"}\n')
            self.assertEqual([], structure.testkit_errors(root))
            write('sim', '[dev-dependencies]\ncore = {package="mtg-core", path="../core", features=["testkit"]}\n')
            self.assertIn('enables mtg-core/testkit', structure.testkit_errors(root)[0])
            write('sim', "[target.'cfg(unix)'.dependencies]\nmtg-witness = {path=\"../witness\"}\n")
            self.assertIn('dev-only mtg-witness', structure.testkit_errors(root)[0])
            write('sim', '[dependencies]\nmtg-core = {path="../core"}\n[features]\nx = ["mtg-core/testkit"]\n')
            self.assertIn('feature x enables', structure.testkit_errors(root)[0])
            write('sim', '[dependencies]\nmtg-core = {path="../core"}\n')
            write('core', '[features]\ndefault = ["testkit"]\ntestkit = []\n')
            self.assertIn('never be a default feature', structure.testkit_errors(root)[0])

    def test_integration_topology_and_public_authority(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            core = root / structure.CRATES / 'core/tests'
            for path in (core / 'rules', core / 'recovery',
                         root / structure.CRATES / 'witness/tests/cards', root / 'packages/game-server/tests/host'):
                path.mkdir(parents=True)
                (path / 'main.rs').touch()
            sim = root / structure.CRATES / 'sim/src'
            sim.mkdir(parents=True)
            (sim / 'lib.rs').write_text(''.join(f'pub mod {m};\n' for m in ('policy', 'pods', 'analytics', 'play', 'mana_spend', 'search')))
            (sim / 'main.rs').touch()
            self.assertEqual(structure.topology_errors(root), [])
            (core / 'rules/adversarial.rs').write_text('use mtg_core::testkit::*;')
            (core / 'extra.rs').touch()
            (sim / 'lib.rs').write_text((sim / 'lib.rs').read_text().replace('pub mod pods;', 'mod pods;'))
            self.assertEqual(len(structure.topology_errors(root)), 3)

    def test_test_named_modules_must_be_cfg_test(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / structure.CRATES / 'x/src'
            src.mkdir(parents=True)
            (src / 'lib.rs').write_text('mod a_tests;\n#[cfg(test)]\nmod b_tests;\n#[path = "c_tests.rs"]\nmod c;\n'
                                        '#[cfg(all(test, feature = "x"))]\nmod d_tests;\nmod prod;\n'
                                        '#[cfg(feature = "testkit")]\npub mod testkit;\n')
            (src / 'b_tests.rs').write_text('mod nested_tests;\n')
            (src / 'prod.rs').write_text('#[cfg(test)]\nmod testkit;\n')
            errors = structure.test_module_errors(root)
            self.assertEqual(3, len(errors))
            self.assertIn('a_tests.rs', errors[0])
            self.assertIn('c_tests.rs', errors[1])
            self.assertIn('testkit module must be', errors[2])


if __name__ == '__main__':
    unittest.main()
