import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('structure', Path(__file__).with_name('engine-structure-check.py'))
structure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(structure)


class StructureTests(unittest.TestCase):
    def test_decision_validators_are_independent_of_descriptive_reason(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            directory = root / structure.CRATES / 'core/src/engine/api/decision'
            directory.mkdir(parents=True)
            source = directory / 'check.rs'
            for expression in ('match decision.reason { _ => true }',
                               'matches!(reason, Some(_))',
                               'matches!(&decision.reason, Reason::Discard)',
                               'use super::Reason as Context;'):
                with self.subTest(expression=expression):
                    source.write_text(expression)
                    self.assertEqual(len(structure.decision_validator_errors(root)), 1)
            source.write_text('// Reason::Discard\nmatch shape { _ => "decision.reason" }')
            (directory / 'project.rs').write_text('match decision.reason { _ => true }')
            self.assertEqual(structure.decision_validator_errors(root), [])

    def test_generated_and_binary_outputs_but_not_large_source_fixtures(self):
        self.assertIsNotNone(structure.artifact_reason('generated/run.json', 4, b'{}'))
        self.assertIsNotNone(structure.artifact_reason('saved/engine', 100, b'\x7fELF'))
        self.assertIsNotNone(structure.artifact_reason('generated/corpus.ndjson', 2**22, b'{}'))
        self.assertIsNone(structure.artifact_reason('tests/fixtures/source.json', 2**25, b'{}'))
        self.assertIsNone(structure.artifact_reason('src/engine.rs', 2**22, b'// source'))

    def test_qualified_sdk_deployment_outputs(self):
        for path in ('packages/game-server/sdk/dist/browser.js',
                     'packages/game-server/sdk/dist/types/generated/validators.d.ts',
                     'packages/game-server/sdk/generated/validators.js',
                     'packages/game/generated/sdk-identity.ts'):
            with self.subTest(path=path):
                self.assertIsNone(structure.artifact_reason(path, 10, b'export {};'))

    def test_sdk_exception_preserves_directory_and_binary_guards(self):
        for path in ('packages/game/generated/other.json',
                     'packages/game/generated/sdk-identity.ts/other.json',
                     'generated/foo.json', 'packages/multiverse/dist/app.js',
                     'packages/game-server/sdk/dist-extra/generated/app.js'):
            with self.subTest(path=path):
                self.assertEqual('generated/cache directory', structure.artifact_reason(path, 2, b'{}'))

        for directory in ('packages/game-server/sdk/dist', 'packages/game-server/sdk/generated'):
            for nested in ('target', 'node_modules', '.scryfall-cache', '__pycache__'):
                path = f'{directory}/{nested}/app.js'
                with self.subTest(path=path):
                    self.assertEqual('generated/cache directory', structure.artifact_reason(path, 2, b'{}'))

        for path in ('packages/game-server/sdk/dist/browser.js',
                     'packages/game-server/sdk/generated/validators.js',
                     'packages/game/generated/sdk-identity.ts'):
            for prefix in (b'\x7fELF', b'\x00asm', b'SQLite format 3\x00'):
                with self.subTest(path=path, prefix=prefix):
                    self.assertEqual('native/Wasm executable or SQLite database',
                                     structure.artifact_reason(path, len(prefix), prefix))

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

    def test_nested_public_witnesses_cannot_reach_private_modules_or_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            core = Path(tmp)
            src = core / 'src'
            src.mkdir()
            (src / 'game.rs').write_text(
                'testkit_fields! {\n pub struct Game {\n  pub(crate) state: State,\n'
                '  prepared: Prepared,\n  pub visible: bool,\n }\n}\n'
                'testkit_fields! { pub struct Receipt { state_token: Token } }')
            for module in ('rules/combat_adversarial', 'recovery/nested', 'support/nested',
                           'rules/rules_kernel/nested_adversarial'):
                path = core / 'tests' / module / 'case.rs'
                path.parent.mkdir(parents=True)
                for forbidden in ('use mtg_core::{testkit as hidden};',
                                  'use crate::rules_kernel::helper;',
                                  'use crate :: state_recovery as hidden;',
                                  'use super::{private, support};', 'use crate::{support, private};',
                                  'fn bad(g: Game) { let _ = g\n .prepared; }',
                                  'fn bad(g: Game) { let _ = g.state.rules; }',
                                  'fn bad(r: Receipt) { let _ = r.state_token; }'):
                    with self.subTest(module=module, forbidden=forbidden):
                        path.write_text(forbidden)
                        self.assertEqual(len(structure.public_witness_errors(core)), 1)
                path.write_text('use crate::support::helper; // g.state\n'
                                'fn ok(g: Game) { g.state(); let _ = g.visible; }')
                self.assertEqual(structure.public_witness_errors(core), [])

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
