import importlib.util
from pathlib import Path
import json
import os
import stat
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('runner', Path(__file__).with_name('engine-test-binaries.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class TestBinaryRunnerTests(unittest.TestCase):
    def test_runner_cargo_and_libtest_arguments(self):
        options, cargo, tests = runner.split_arguments(
            ['--max-binaries', '2', '--workers', '8', '-p', 'x', '--test', '*', '--', 'FILTER', '--ignored'])
        self.assertEqual({'max_binaries': 2, 'workers': 8}, options)
        self.assertEqual((['-p', 'x', '--test', '*'], ['FILTER', '--ignored']), (cargo, tests))
        options, cargo, tests = runner.split_arguments(['--lib'])
        self.assertEqual(4, options['max_binaries'])
        self.assertLessEqual(options['workers'], 16)
        self.assertEqual((['--lib'], []), (cargo, tests))

    def test_binaries_times_threads_stay_within_the_worker_budget(self):
        self.assertEqual((4, ['F', '--test-threads=4']), runner.plan({'max_binaries': 4, 'workers': 16}, ['F']))
        self.assertEqual((2, ['--test-threads=1']), runner.plan({'max_binaries': 4, 'workers': 2}, []))
        # A caller's thread count wins and lowers the binary cap instead.
        self.assertEqual((2, ['--test-threads=8']), runner.plan({'max_binaries': 4, 'workers': 16}, ['--test-threads=8']))
        self.assertEqual((1, ['--test-threads', '16']), runner.plan({'max_binaries': 4, 'workers': 16}, ['--test-threads', '16']))
        for workers in range(1, 33):
            for cap in range(1, 9):
                binaries, args = runner.plan({'max_binaries': cap, 'workers': workers}, [])
                threads = int(args[-1].split('=')[1])
                self.assertLessEqual(binaries * threads, workers)
                self.assertLessEqual(binaries, cap)

    def test_zero_executed_tests_fail_unless_a_filter_selects_others(self):
        ok = lambda n: (0, f'running {n} tests\ntest result: ok. {n} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out', 0.1)
        self.assertEqual(3, runner.passed(ok(3)[1]))
        self.assertIsNone(runner.passed('no summary'))
        self.assertEqual(['empty'], runner.empty_selection({'full': ok(2), 'empty': ok(0)}, []))
        self.assertEqual(['empty'], runner.empty_selection({'full': ok(2), 'empty': ok(0)}, ['--test-threads=4']))
        self.assertEqual([], runner.empty_selection({'full': ok(2), 'empty': ok(0)}, ['FILTER']))
        self.assertEqual(['a', 'b'], runner.empty_selection({'a': ok(0), 'b': ok(0)}, ['FILTER']))
        opt_in = (0, 'test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out', 0.1)
        self.assertEqual([], runner.empty_selection({'full': ok(2), 'witness': opt_in}, []))
        # ...but a selection of only ignored opt-in witnesses executes nothing.
        self.assertEqual(['a', 'b'], runner.empty_selection({'a': opt_in, 'b': opt_in}, []))
        self.assertEqual(['a'], runner.empty_selection({'a': opt_in}, ['FILTER']))
        # A failed binary is reported as failed, not as empty.
        self.assertEqual([], runner.empty_selection({'bad': (101, 'panicked', 0.1)}, []))

    def test_only_test_profile_executables_are_selected(self):
        artifact = lambda name, test, exe: json.dumps({
            'reason': 'compiler-artifact', 'target': {'name': name}, 'profile': {'test': test},
            'executable': exe, 'manifest_path': '/w/crates/core/Cargo.toml'})
        lines = [artifact('mtg_core', False, None), artifact('build', False, '/t/build-script'),
                 artifact('rules', True, '/t/rules-1'), 'not json', json.dumps({'reason': 'build-finished'})]
        self.assertEqual([('rules', '/t/rules-1', '/w/crates/core')], runner.executables(lines))

    def test_every_binary_runs_with_forwarded_threads_and_any_failure_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            log = root / 'calls'
            def script(name, code):
                path = root / name
                path.write_text(f'#!/bin/sh\necho "{name} $*" >> {log}\necho "test result: {name}"\nexit {code}\n')
                path.chmod(path.stat().st_mode | stat.S_IEXEC)
                return str(path)
            messages = '\\n'.join(json.dumps({'reason': 'compiler-artifact', 'target': {'name': n},
                                              'profile': {'test': True}, 'executable': script(n, c),
                                              'manifest_path': str(root / 'Cargo.toml')})
                                  for n, c in [('good', 0), ('bad', 3), ('also', 0)])
            cargo = root / 'cargo'
            cargo.write_text(f"#!/bin/sh\nprintf '{messages}\\n'\n")
            cargo.chmod(cargo.stat().st_mode | stat.S_IEXEC)
            path = os.environ['PATH']
            os.environ['PATH'] = f'{root}:{path}'
            try:
                self.assertEqual(1, runner.main(['--max-binaries', '2', '--workers', '4', '--test', '*', '--', 'FILTER']))
            finally:
                os.environ['PATH'] = path
            calls = sorted(log.read_text().splitlines())
            self.assertEqual(['also FILTER --test-threads=2', 'bad FILTER --test-threads=2',
                              'good FILTER --test-threads=2'], calls)


if __name__ == '__main__':
    unittest.main()
