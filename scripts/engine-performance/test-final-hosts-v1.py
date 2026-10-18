"""Tiny synthetic guard fixtures only; no native/Cargo/systemd/benchmark runs."""
from pathlib import Path
import copy
import hashlib
import importlib.util
import json
import os
import signal
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

HERE = Path(__file__).absolute().parent
ROOT = HERE.parents[1]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    obj = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(obj)
    return obj


h = load('test_portable_inputs', 'host_inputs.py')
pods = load('test_portable_pods', 'run-final-pod-series-v1.py')
primary = load('test_portable_primary', 'run-final-search-primary-v1.py')
review = load('test_portable_review', 'review-final-search-primary-v1.py')
primary.INPUTS = review.INPUTS = h


def write(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_text(json.dumps(obj, indent=2) + '\n')
    return path


def pinned(path):
    return {'path': str(path), 'sha256': h.sha(path)}


def resource_fixture():
    return {'schema': 'bounded-measurement-v1', 'status': 'passed', 'deadline_seconds': 120,
            'cpu_ids': [0], 'executable_sha256': '0'*64,
            'limits': {'memory_bytes': 2**31, 'postexit_child_rss_guard_bytes': 2**31,
                       'swap_bytes': 0, 'tasks': 128, 'systemd_runtime_max_seconds': 125},
            'cleanup': {'unit_empty': True, 'launcher_reaped': True},
            'workload': {'exit_code': 0, 'ru_maxrss_bytes': 1000},
            'kernel_final': {'memory_peak_bytes': 2000, 'memory_events': {}}}


class AuthorityFixture:
    """A tiny made-up compiled artifact. It establishes no real Build claim."""
    def __init__(self, base):
        self.base = base
        self.root = base / 'stage'
        self.archive = base / 'archive'
        self.out = base / 'runs'
        for p in [self.root, self.archive, self.out]:
            p.mkdir(mode=0o700)
        for rel in ['packages/game-engine/crates', 'packages/game-server/src',
                    'packages/game-server/tests', 'packages/game-engine/tests/fixtures']:
            (self.root / rel).mkdir(parents=True, mode=0o700)
        for rel in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'packages/game-server/Cargo.toml']:
            (self.root / rel).write_text('tiny fixture, not Cargo\n')
        (self.root / 'packages/game-engine/crates/example.rs').write_text('pub fn fixture() {}\n')
        workload = ROOT / h.WORKLOAD_REL
        (self.root / h.WORKLOAD_REL).write_bytes(workload.read_bytes())
        fixed_path = ROOT / 'scripts/engine-performance/run-fixed-work.py'
        self.fixed = h.module(pinned(fixed_path), 'fixture_real_fixed')
        self.files = []
        self.archive_rows = {}
        for rel in self.fixed.inventory(self.root):
            src = self.root / rel
            self.files.append({'path': rel, 'bytes': src.stat().st_size, 'sha256': h.sha(src)})
            dst = self.archive / 'files/source' / rel
            dst.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            dst.write_bytes(src.read_bytes())
            self.archive_row(dst)
        self.settings = {'rustc': 'rustc 1.99.0 fixture-only', 'profile': 'release',
            'target': 'x86_64-unknown-linux-gnu', 'rustflags': '', 'opt_level': '3',
            'debug': 'false', 'debug_assertions': 'false', 'overflow_checks': True}
        self.build = self.fixed.compiled_build_id(self.root, self.files, self.settings)
        self.native = self.archive / 'files/proof/native.json'
        write(self.native, {'status': 'compiled-source-closure-verified', 'build_id': self.build, 'files': self.files})
        self.archive_row(self.native)
        self.cli = self.archive / 'files/executables/cli'
        self.cli.parent.mkdir(parents=True, mode=0o700)
        self.cli.write_bytes(b'NOT an executable; no child is run by this fixture\n')
        self.archive_row(self.cli)
        self.artifact = self.archive / 'files/proof/artifact.json'
        write(self.artifact, {'reason': 'compiler-artifact', 'target': {'name': 'mtg-sim', 'kind': ['bin']},
            'features': [], 'executable': str(self.cli), 'profile': {'test': False, 'opt_level': '3',
            'debuginfo': 0, 'debug_assertions': False, 'overflow_checks': True}})
        self.archive_row(self.artifact)
        self.archive_manifest = self.archive / 'manifest.json'
        self.save_archive()
        prereq = []
        for role in ['functional_gates', 'independent_audit']:
            q = write(base / (role + '.json'), {'status': 'passed', 'build_id': 'old-explicit-fixture'})
            prereq.append({'role': role, 'file': pinned(q), 'expected_status': 'passed', 'expected_build_id': 'old-explicit-fixture'})
        host_paths = [HERE / n for n in ['host_inputs.py', 'run-final-search-primary-v1.py', 'review-final-search-primary-v1.py']]
        host_paths += [fixed_path, fixed_path.with_name('measurement_host.py')]
        self.a = {'schema': 'final-search-host-authority-v1', 'stage_root': str(self.root),
            'build_id': self.build, 'native_files': len(self.files), 'native_bytes': sum(v['bytes'] for v in self.files),
            'native_manifest': pinned(self.native), 'archive_manifest': pinned(self.archive_manifest),
            'archive_files_prefix': 'files', 'archive_native_prefix': 'source', 'cli': pinned(self.cli),
            'compiler_artifact': pinned(self.artifact), 'build_settings': self.settings,
            'fixed_driver': pinned(fixed_path), 'measurement_helper_sha256': h.sha(fixed_path.with_name('measurement_host.py')),
            'host_sources': {str(q): h.sha(q) for q in host_paths}, 'workload': pinned(self.root / h.WORKLOAD_REL),
            'run_parent': str(self.out), 'prerequisites': prereq,
            'primary_wrapper': pinned(HERE / 'run-final-search-primary-v1.py')}
        self.authority = base / 'authority.json'
        self.save()

    def archive_row(self, path):
        self.archive_rows[str(path.relative_to(self.archive/'files'))] = {'bytes': path.stat().st_size, 'sha256': h.sha(path)}

    def save_archive(self):
        write(self.archive_manifest, {'engine_build_id': self.build, 'files': self.archive_rows})

    def save(self):
        write(self.authority, self.a)

    def check(self):
        self.save()
        return h.check_search(self.authority, h.sha(self.authority), HERE / 'run-final-search-primary-v1.py')


class InputGuards(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.fixture = AuthorityFixture(Path(self.tmp.name))

    def test_complete_tiny_fixture_accepts_without_executing_cli(self):
        a, native, proof = self.fixture.check()
        self.assertEqual(native['build_id'], a['build_id'])
        self.assertEqual(proof['workload_sha256'], h.WORKLOAD_SHA)

    def test_wrong_manifest_count_and_build_reject_even_after_authority_rehash(self):
        for key, bad in [('native_files', 999), ('native_bytes', 1), ('build_id', '0' * 64)]:
            old = self.fixture.a[key]
            self.fixture.a[key] = bad
            with self.assertRaises(ValueError): self.fixture.check()
            self.fixture.a[key] = old

    def test_wrong_executable_bytes_reject(self):
        self.fixture.cli.write_bytes(b'changed')
        with self.assertRaises(ValueError): self.fixture.check()

    def test_source_changed_under_rehashed_manifest_still_rejects_old_build(self):
        q = self.fixture.root / 'packages/game-engine/crates/example.rs'
        q.write_text(q.read_text().replace('fixture', 'changed'))
        stored = self.fixture.archive / 'files/source' / q.relative_to(self.fixture.root)
        stored.write_bytes(q.read_bytes())
        for item in self.fixture.files:
            if item['path'] == str(q.relative_to(self.fixture.root)):
                item.update(bytes=q.stat().st_size, sha256=h.sha(q))
        write(self.fixture.native, {'status': 'compiled-source-closure-verified', 'build_id': self.fixture.build, 'files': self.fixture.files})
        self.fixture.archive_row(stored); self.fixture.archive_row(self.fixture.native); self.fixture.save_archive()
        self.fixture.a['native_manifest'] = pinned(self.fixture.native)
        self.fixture.a['archive_manifest'] = pinned(self.fixture.archive_manifest)
        with self.assertRaisesRegex(ValueError, 'recomputed BuildID'): self.fixture.check()

    def test_extra_native_source_and_wrong_archive_path_reject(self):
        q = self.fixture.root / 'packages/game-engine/crates/unlisted.rs'; q.write_text('extra')
        with self.assertRaisesRegex(ValueError, 'namespace'): self.fixture.check()
        q.unlink(); self.fixture.a['archive_native_prefix'] = '../source'
        with self.assertRaises(ValueError): self.fixture.check()

    def test_production_artifact_rejects_test_features_overflow_and_profile(self):
        original = h.read(self.fixture.artifact)
        for change in [lambda a: a['profile'].update(test=True), lambda a: a.update(features=['diagnostic-allocations']),
                       lambda a: a['profile'].update(overflow_checks=False), lambda a: a['profile'].update(opt_level='2')]:
            artifact = copy.deepcopy(original); change(artifact); write(self.fixture.artifact, artifact)
            self.fixture.archive_row(self.fixture.artifact); self.fixture.save_archive()
            self.fixture.a.update(compiler_artifact=pinned(self.fixture.artifact), archive_manifest=pinned(self.fixture.archive_manifest))
            with self.assertRaises(ValueError): self.fixture.check()

    def test_workload_and_prerequisite_status_cannot_be_relabelled(self):
        self.fixture.a['workload']['sha256'] = '0' * 64
        with self.assertRaises(ValueError): self.fixture.check()
        self.fixture.a['workload'] = pinned(self.fixture.root / h.WORKLOAD_REL)
        role = self.fixture.a['prerequisites'][0]
        q = Path(role['file']['path']); write(q, {'status': 'failed', 'build_id': 'old-explicit-fixture'})
        role['file'] = pinned(q); role['expected_status'] = 'failed'
        with self.assertRaisesRegex(ValueError, 'prerequisite status'): self.fixture.check()

    def test_duplicate_json_and_unsafe_retry_or_symlink_paths(self):
        q = Path(self.tmp.name) / 'dup.json'; q.write_text('{"status":1,"status":2}')
        with self.assertRaisesRegex(ValueError, 'duplicate'): h.read(q)
        out = self.fixture.out / 'run'; out.mkdir()
        with self.assertRaises(ValueError): h.fresh(out, self.fixture.out)
        link = self.fixture.out / 'alias'; link.symlink_to(self.fixture.cli)
        with self.assertRaises(ValueError): h.plain(link)
        with self.assertRaises(ValueError): h.fresh(self.fixture.out / '../escape', self.fixture.out)


class PodGuards(unittest.TestCase):
    def test_real_series_guard_unpacks_unchanged_batch_auth_tuple(self):
        with tempfile.TemporaryDirectory() as tmp:
            parent = Path(tmp); roles = {}
            for name in ['fixed.py', 'ordinary.py', 'batch.py', 'batch-authority.json', 'measurement_host.py']:
                q = parent/name; q.write_text('fixture-only, never imported'); roles[name] = pinned(q)
            actual = {'engine_build_id': 'b', 'timeout_seconds': 120, 'cpu_ids': [0], 'current_pod_cases': list(range(18))}
            a = {'schema': 'final-pod-series-authority-v1', 'engine_build_id': 'b', 'warmup_batches': 1,
                 'measured_batches': 10, 'batch_indices': list(range(11)), 'seed_indices': list(range(18)),
                 'retry_policy': 'fail-stop-no-retry-v1', 'run_parent': str(parent),
                 'fixed_driver': roles['fixed.py'], 'ordinary_driver': roles['ordinary.py'],
                 'single_batch_driver': roles['batch.py'], 'single_batch_authority': roles['batch-authority.json'],
                 'measurement_helper_sha256': roles['measurement_host.py']['sha256'],
                 'host_sources': {str(q): h.sha(q)for q in [HERE/'host_inputs.py', HERE/'run-final-pod-series-v1.py',
                    *[Path(v['path'])for v in roles.values()]]}}
            authority = write(parent/'series-authority.json', a)
            # Authenticated-module boundary stub returns EXACT real API tuple;
            # series.guard itself and all its actual pin/path guards run.
            modules = {roles['fixed.py']['path']: SimpleNamespace(load_host=lambda pin: object()),
                       roles['ordinary.py']['path']: object(),
                       roles['batch.py']['path']: SimpleNamespace(auth=lambda *args: (actual, ['deck'], {'TURN_LIMIT':1000}))}
            with patch.object(h, 'module', side_effect=lambda row,name: modules[row['path']]):
                result, got = pods.guard(h, authority, h.sha(authority))
            self.assertEqual(result, a)
            self.assertEqual(got, actual)

    def test_exact_dimensions_and_retry_policy(self):
        a = {'schema': 'final-pod-series-authority-v1', 'warmup_batches': 1, 'measured_batches': 10,
             'batch_indices': list(range(11)), 'seed_indices': list(range(18)), 'retry_policy': 'fail-stop-no-retry-v1'}
        pods.dimensions(h, a)
        for key, value in [('measured_batches', 5), ('warmup_batches', 0), ('seed_indices', list(range(17))),
                           ('batch_indices', list(range(10))), ('retry_policy', 'retry-until-pass')]:
            bad = copy.deepcopy(a); bad[key] = value
            with self.assertRaises(ValueError): pods.dimensions(h, bad)

    def test_result_requires_all18_and_bounded_controller_cleanup(self):
        v = {'schema': 'bounded-current18-pod-batch-v1', 'status': 'all18_complete_and_verified',
             'engine_build_id': 'b', 'failure': None,
             'audit': {'all18_complete_verified': True, 'planned_pods': 18, 'completed_pods': 18,
                       'generation_sum_ns': 10, 'whole_batch_ns': 20},
             'generation_pods_per_second': 18e9 / 10, 'whole_batch_pods_per_second': 18e9 / 20}
        bounded = {'status': 'passed', 'exit_code': 0, 'direct_child_reaped': True, 'owned_group_empty': True}
        pods.batch_result(h, v, bounded, 'b')
        for mutate in [lambda z: z['audit'].update(completed_pods=17), lambda z: z.update(engine_build_id='wrong'),
                       lambda z: z.update(generation_pods_per_second=999)]:
            bad = copy.deepcopy(v); mutate(bad)
            with self.assertRaises(ValueError): pods.batch_result(h, bad, bounded, 'b')
        for key, bad in [('status', 'failed'), ('owned_group_empty', False), ('direct_child_reaped', False), ('exit_code', 1)]:
            z = {**bounded, key: bad}
            with self.assertRaises(ValueError): pods.batch_result(h, v, z, 'b')

    def test_semantics_omit_elapsed_only_and_retain_all_journal_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'game.json'; base = {'elapsed_millis': 1, 'journal': [1], 'hidden': {'x': 2}}
            write(p, base); a = pods.semantic(h, p)
            write(p, {**base, 'elapsed_millis': 100}); self.assertEqual(a, pods.semantic(h, p))
            write(p, {**base, 'hidden': {'x': 3}}); self.assertNotEqual(a, pods.semantic(h, p))

    def test_actual_batch_resource_proof_rejects_cap_pressure_cleanup_and_exe_mismatch(self):
        actual = {'cpu_ids': [0], 'test_executable': {'sha256': '0'*64}}
        pods.resource_result(h, resource_fixture(), actual)
        for mutate in [lambda z: z['limits'].update(memory_bytes=3*2**30),
                       lambda z: z.update(deadline_seconds=121),
                       lambda z: z['kernel_final']['memory_events'].update(max=1),
                       lambda z: z['workload'].update(ru_maxrss_bytes=2**31+1),
                       lambda z: z['cleanup'].update(unit_empty=False),
                       lambda z: z.update(executable_sha256='1'*64)]:
            bad = resource_fixture(); mutate(bad)
            with self.assertRaises(ValueError): pods.resource_result(h, bad, actual)

    def test_failed_first_batch_stops_without_invisible_retry(self):
        with tempfile.TemporaryDirectory() as tmp:
            parent = Path(tmp); original = {'stage_root': str(parent)}
            a = {'engine_build_id': 'b', 'run_parent': str(parent)}
            for role in ['single_batch_driver', 'ordinary_driver', 'fixed_driver', 'single_batch_authority']:
                a[role] = {'path': '/fixture/unused', 'sha256': '0' * 64}
            a['measurement_helper_sha256'] = '0' * 64
            called = []
            def fake_run(command, cwd, out, err, timeout, cancelled):
                called.append(command); out.write_text('raw'); err.write_text('raw')
                directory = Path(command[command.index('--output') + 1]); directory.mkdir()
                write(directory / 'result.json', {'status': 'failed_or_truncated_no_throughput'})
                return {'status': 'failed', 'direct_child_reaped': True, 'owned_group_empty': True, 'exit_code': 1}
            with patch.object(pods, 'guard', return_value=(a, original)), patch.object(h, 'run_owned', side_effect=fake_run):
                args = SimpleNamespace(authority=parent / 'unused', authority_sha256='0'*64, output=parent / 'series')
                # First failed attempt is enough: no shortened/retried replacement.
                result = pods.execute(h, args)
            self.assertEqual(len(called), 1)
            self.assertEqual(len(result['attempts']), 1)
            self.assertEqual(result['planned_pods'], 198)
            self.assertEqual(result['unstarted_batch_indices'], list(range(1, 11)))
            self.assertIsNone(result['summaries'])
            self.assertEqual(result['attempts'][0]['status'], 'failed_preserved_no_retry')

    def run_fake_series(self, parent, changed_batch=None):
        original = {'stage_root': str(parent), 'cpu_ids': [0], 'test_executable': {'sha256': '0'*64}}
        a = {'engine_build_id': 'b', 'run_parent': str(parent)}
        for role in ['single_batch_driver', 'ordinary_driver', 'fixed_driver', 'single_batch_authority']:
            a[role] = {'path': '/fixture/unused', 'sha256': '0' * 64}
        a['measurement_helper_sha256'] = '0' * 64
        calls = []
        def fake_run(command, cwd, out, err, timeout, cancelled):
            index = len(calls); calls.append(command)
            self.assertEqual(timeout, 150)
            out.write_text('raw controller output'); err.write_text('raw stderr')
            directory = Path(command[command.index('--output') + 1]); directory.mkdir()
            resource = write(directory/'execution/host-report.json', resource_fixture())
            for game in range(18):
                write(directory/'games'/f'game-{game:02}.json', {'elapsed_millis': index,
                    'journal': {'game': game, 'hidden_fact': 999 if index == changed_batch and game == 3 else 7}})
            write(directory/'result.json', {'schema': 'bounded-current18-pod-batch-v1',
                'status': 'all18_complete_and_verified', 'engine_build_id': 'b', 'failure': None,
                'audit': {'all18_complete_verified': True, 'planned_pods': 18, 'completed_pods': 18,
                          'generation_sum_ns': 10, 'whole_batch_ns': 20},
                'generation_pods_per_second': 18e9/10, 'whole_batch_pods_per_second': 18e9/20,
                'host_report_sha256': h.sha(resource)})
            return {'status': 'passed', 'direct_child_reaped': True, 'owned_group_empty': True, 'exit_code': 0}
        with patch.object(pods, 'STOP', False), patch.object(pods, 'guard', return_value=(a, original)), patch.object(h, 'run_owned', side_effect=fake_run):
            result = pods.execute(h, SimpleNamespace(authority=parent/'unused', authority_sha256='0'*64, output=parent/'series'))
        return result, calls

    def test_cancellation_during_final_series_publication_withdraws_success(self):
        with tempfile.TemporaryDirectory() as tmp:
            original_save = h.save
            def cancel_save(path, value):
                original_save(path, value)
                if value.get('status') == 'all11_batches_semantically_verified':
                    pods.STOP = True
            with patch.object(h, 'save', side_effect=cancel_save):
                result, _calls = self.run_fake_series(Path(tmp))
            self.assertEqual(result['status'], 'failed_or_incomplete')
            self.assertEqual(h.read(Path(tmp)/'series/status.json')['status'], 'failed_or_incomplete')
            self.assertIsNone(result['summaries'])

    def test_all11_fake_batches_preserve198_planned180_measured(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, calls = self.run_fake_series(Path(tmp))
            self.assertEqual(result['status'], 'all11_batches_semantically_verified')
            self.assertEqual(len(calls), 11)
            self.assertEqual(result['warmup_complete_pods'], 18)
            self.assertEqual(result['measured_complete_pods'], 180)
            self.assertEqual(result['unstarted_batch_indices'], [])
            self.assertEqual(len(result['summaries']['generation_sum_ns']['raw']), 10)

    def test_second_batch_full_evidence_mismatch_fails_stops_and_retains_raw(self):
        with tempfile.TemporaryDirectory() as tmp:
            parent = Path(tmp); result, calls = self.run_fake_series(parent, changed_batch=1)
            self.assertEqual(result['status'], 'failed_or_incomplete')
            self.assertEqual(len(calls), 2)
            self.assertEqual(result['attempts'][0]['status'], 'passed')
            self.assertEqual(result['attempts'][1]['status'], 'failed_preserved_no_retry')
            self.assertEqual(result['unstarted_batch_indices'], list(range(2, 11)))
            self.assertTrue((parent/'series/batch-01/games/game-03.json').exists())
            self.assertIsNone(result['summaries'])


class PrimaryAndReplayGuards(unittest.TestCase):
    def test_native_primary_environment_is_explicit_not_ambient(self):
        with patch.dict(os.environ, {'LD_PRELOAD': '/not-loaded', 'LD_LIBRARY_PATH': '/ignored'}), \
             patch.object(primary.subprocess, 'Popen', return_value=object()) as spawn:
            primary.spawn_benchmark({'executable': '/NOT-EXECUTED'}, Path('/fresh'), object(), object())
        self.assertEqual(spawn.call_args.kwargs['env'], {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'})
        self.assertEqual(spawn.call_args.args[0], ['/NOT-EXECUTED', 'search-benchmark', '/fresh', '--quiescent'])

    def make_suite(self, folder):
        jobs = []
        for index in range(12):
            for repetition in range(5):
                name = f'root-{index:02}-repeat-{repetition}.json'
                job = {'index': index, 'repetition': repetition, 'evidence': name, 'worker_cpu_ids_verified': True,
                       'elapsed_micros': 2_000_000, 'maximum_process_rss_bytes': 1000, 'minimum_met': True}
                root = {'request': {'root_index': index, 'worker_cpu_ids': [0,1,2,3]},
                        'actual_worker_cpu_ids': [0,1,2,3], 'elapsed_micros': 2_000_000,
                        'maximum_process_rss_bytes': 1000, 'minimum_met': True,
                        'aggregates': [{'minimum_met': True, 'useful_distinct_worlds': 8}]}
                write(folder / name, {'build_id': 'b', 'report': root}); jobs.append(job)
        return {'build_id': 'b', 'numeric_gate_met': True, 'jobs': jobs,
                'attempted_jobs': 60, 'reported_job_latencies': 60, 'p95_nearest_rank': 57,
                'p95_elapsed_micros': 2_000_000}

    def test_exact60_root_denominator_and_p95(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp); report = self.make_suite(p)
            self.assertTrue(primary.summarize(report, p, 'b', 1000)['p95_met'])
            for mutate in [lambda z: z['jobs'].pop(), lambda z: z.update(attempted_jobs=59),
                           lambda z: z.update(p95_nearest_rank=56), lambda z: z.update(p95_elapsed_micros=1)]:
                bad = copy.deepcopy(report); mutate(bad)
                with self.assertRaises(RuntimeError): primary.summarize(bad, p, 'b', 1000)

    def test_wrong_evidence_path_exe_build_and_minimum_reject(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp); report = self.make_suite(p)
            report['jobs'][0]['evidence'] = '../escape'
            with self.assertRaises(RuntimeError): primary.summarize(report, p, 'b', 1000)
            report = self.make_suite(p)
            with self.assertRaises(RuntimeError): primary.summarize(report, p, 'wrong', 1000)
            q = p / 'root-00-repeat-0.json'; v = h.read(q); v['report']['aggregates'][0]['useful_distinct_worlds'] = 7; write(q, v)
            with self.assertRaises(RuntimeError): primary.summarize(report, p, 'b', 1000)

    def test_hard_latency_memory_and_cpu_topology_limits_unchanged(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp); report = self.make_suite(p)
            result = primary.summarize(report, p, 'b', 2**31 + 1)
            self.assertFalse(result['whole_process_memory_met'])
            for job in report['jobs']:
                q = p/job['evidence']; value = h.read(q)
                value['report']['elapsed_micros'] = job['elapsed_micros'] = 5_000_001
                write(q, value)
            report['p95_elapsed_micros'] = 5_000_001
            result = primary.summarize(report, p, 'b', 1000)
            self.assertFalse(result['all_hard_latency_met'])
            self.assertFalse(result['p95_met'])
            primary.validate_topology([0,1,2,3], [(0,i)for i in range(4)])
            with self.assertRaises(RuntimeError): primary.validate_topology([0,1,2,3], [(0,0)]*4)
            with self.assertRaises(RuntimeError): primary.validate_topology([0,1,2], [(0,i)for i in range(4)])

    def test_idle_active_recent_reused_pid_and_waited_child_burst_reject(self):
        def process(start=0, ticks=0):
            return {'pid': 42, 'name': 'node-MainThread', 'state': 'S', 'start_ticks': start,
                    'user_ticks': ticks, 'system_ticks': 0, 'waited_child_user_ticks': 0,
                    'waited_child_system_ticks': 0, 'cpu_ticks': ticks}
        old = process()
        baseline = primary.IdleBaseline([old], [old], 100000, 100200, 2_000_000_000, 0, ticks=100)
        baseline.sample([old], 1_000_000_000)
        burst = {**old, 'waited_child_user_ticks': 2, 'cpu_ticks': 2}
        with self.assertRaises(RuntimeError): baseline.sample([burst], 2_000_000_000)
        with self.assertRaises(RuntimeError): primary.IdleBaseline([old], [process(ticks=1)], 100000, 100200, 2_000_000_000, 0, ticks=100)
        recent = process(start=99999)
        with self.assertRaises(RuntimeError): primary.IdleBaseline([recent], [recent], 100000, 100200, 2_000_000_000, 0, ticks=100)
        with self.assertRaises(RuntimeError): baseline.sample([process(start=1)], 3_000_000_000)

    def test_replay_no_duplicate_missing_or_short_useful_denominator(self):
        evidence = {'useful_trials': 24, 'diagnostic_stops': 72}
        good = {'status': 'semantic_worklist_verified', 'measurements': 'not_remeasured', 'verified_trials': 24, 'unverified_diagnostic_stops': 72}
        review.verify_replay(good, evidence)
        for key, value in [('verified_trials', 23), ('unverified_diagnostic_stops', 0), ('measurements', 'remeasured')]:
            with self.assertRaises(AssertionError): review.verify_replay({**good, key: value}, evidence)
        rows = [{'index': i, 'repetition': j, 'status': 'passed'}for i in range(12)for j in range(5)]
        review.verify_replay_set(rows)
        for bad in [rows[:-1], rows[:-1]+[rows[0]], rows[:-1]+[{**rows[-1], 'status': 'failed'}]]:
            with self.assertRaises(AssertionError): review.verify_replay_set(bad)

    def test_signal_during_popen_acquires_then_cleans_owned_lease(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp); state = {'cancelled': False, 'alive': True, 'reaped': False}; signals = []
            child = SimpleNamespace(pid=12345, returncode=None)
            def popen(*args, **kw): state['cancelled'] = True; return child
            def killpg(pid, sig):
                signals.append(sig)
                if sig: state['alive'] = False
                if sig == 0 and not state['alive']: raise ProcessLookupError()
            def wait4(pid, options):
                if state['alive']: return (0, 0, None)
                state['reaped'] = True
                return (pid, signal.SIGTERM, SimpleNamespace(ru_utime=0, ru_stime=0, ru_maxrss=1))
            with patch.object(h.subprocess, 'Popen', side_effect=popen), patch.object(h.os, 'killpg', side_effect=killpg), patch.object(h.os, 'wait4', side_effect=wait4):
                result = h.run_owned(['/NOT-EXECUTED'], p, p/'out', p/'err', 300, lambda: state['cancelled'])
            self.assertEqual(result['status'], 'failed')
            self.assertTrue(result['direct_child_reaped'] and result['owned_group_empty'])
            self.assertIn(signal.SIGTERM, signals)
            self.assertTrue(state['reaped'])

    def test_primary_unreaped_failure_never_uses_unbounded_wait4(self):
        child = SimpleNamespace(pid=12345, returncode=None)
        ticks = iter(range(100))
        with patch.object(primary.os, 'killpg'), patch.object(primary.os, 'wait4', return_value=(0, 0, None)) as wait, \
             patch.object(primary.time, 'monotonic', side_effect=lambda: next(ticks)), patch.object(primary.time, 'sleep'):
            reaped, usage = primary.reap(child, False)
        self.assertFalse(reaped)
        self.assertIsNone(usage)
        self.assertTrue(all(call.args[1] == os.WNOHANG for call in wait.call_args_list))

    def test_cancel_during_successful_child_cleanup_cannot_return_passed(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp); state = {'cancelled': False, 'alive': True}
            child = SimpleNamespace(pid=12345, returncode=None)
            def killpg(pid, sig):
                if sig:
                    state.update(cancelled=True, alive=False)
                elif not state['alive']:
                    raise ProcessLookupError()
            usage = SimpleNamespace(ru_utime=0, ru_stime=0, ru_maxrss=1)
            with patch.object(h.subprocess, 'Popen', return_value=child), patch.object(h.os, 'killpg', side_effect=killpg), \
                 patch.object(h.os, 'wait4', return_value=(12345, 0, usage)):
                result = h.run_owned(['/NOT-EXECUTED'], p, p/'out', p/'err', 300, lambda: state['cancelled'])
            self.assertEqual(result['status'], 'failed')
            self.assertTrue(result['direct_child_reaped'] and result['owned_group_empty'])

    def test_cancellation_during_primary_and_reviewer_success_save_withdraws_success(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp); original_save = h.save
            def cancel_primary(path, value):
                original_save(path, value)
                if value['status'] == 'measurement_passed_replay_pending': primary.STOP = True
            result = {'status': 'measurement_passed_replay_pending'}
            with patch.object(primary, 'STOP', False), patch.object(h, 'save', side_effect=cancel_primary):
                primary.finalize_report(result, p/'primary.json')
            self.assertEqual(h.read(p/'primary.json')['status'], 'failed')
            def cancel_review():
                original_save(p/'review.json', review.result)
                if review.result['status'] == 'passed': review.STOP = True
            with patch.object(review, 'STOP', False), patch.object(review, 'result', {'status':'running'}, create=True), \
                 patch.object(review, 'save', side_effect=cancel_review):
                with self.assertRaises(AssertionError): review.publish_success()
            self.assertEqual(h.read(p/'review.json')['status'], 'failed')


class IdleProofGuards(unittest.TestCase):
 def fixture(self, empty=False):
  hz=os.sysconf('SC_CLK_TCK')
  row={'pid':10,'name':'node-MainThread','state':'S','start_ticks':1000,'user_ticks':20,'system_ticks':0,'waited_child_user_ticks':0,'waited_child_system_ticks':0,'cpu_ticks':20}
  base=[] if empty else [row]
  current=copy.deepcopy(base)
  samples=[]
  for elapsed,window in [(10_000_000_000,10_000_000_000),(11_000_000_000,1_000_000_000)]:
   samples.append({'elapsed_ns':elapsed,'window_ns':window,'aggregate_window_cpu_ticks':0,'aggregate_total_cpu_ticks':0,'processes':copy.deepcopy(current),'errors':[],'met':True,'per_process_deltas':[{'pid':r['pid'],'start_ticks':r['start_ticks'],'window_cpu_ticks':0,'total_cpu_ticks':0} for r in current]})
  proof={'policy':review.IDLE_POLICY,'clock_ticks_per_second':hz,'minimum_age_seconds':600,'preflight_sample_elapsed_ns':2_000_000_000,'preflight_uptime_ticks':[1000+700*hz,1000+702*hz],'aggregate_cpu_limit_percent_of_one_core':1,'aggregate_cpu_denominator':100,'first_preflight_processes':copy.deepcopy(base),'baseline_processes':copy.deepcopy(base),'samples':samples,'met':True}
  return {'idle_baseline':proof,'initial_other_heavy_processes':copy.deepcopy(base),'final_other_heavy_processes':copy.deepcopy(current),'other_heavy_process_observations':[],'quiescence_met':True,'wall_seconds':10.8}
 def apply_child_ticks(self,w,n):
  sample=w['idle_baseline']['samples'][-1];row=sample['processes'][0];row['waited_child_user_ticks']=n;row['cpu_ticks']=sum(row[k] for k in review.CPU_FIELDS);sample['aggregate_window_cpu_ticks']=sample['aggregate_total_cpu_ticks']=n;sample['per_process_deltas'][0].update(window_cpu_ticks=n,total_cpu_ticks=n);w['final_other_heavy_processes']=copy.deepcopy(sample['processes'])
 def test_valid_bounded_and_empty_baselines_independently_verify(self):
  for empty in [False,True]:
   w=self.fixture(empty);r=review.verify_idle_baseline(w)
   self.assertEqual(r['status'],'verified_bounded_idle_baseline');self.assertEqual(r['samples_verified'],2)
 def test_waited_child_cpu_is_charged_at_one_core_not_four_core_or_total_average(self):
  w=self.fixture();self.apply_child_ticks(w,os.sysconf('SC_CLK_TCK')//100)
  review.verify_idle_baseline(w)
  self.apply_child_ticks(w,os.sysconf('SC_CLK_TCK')//100+1)
  with self.assertRaisesRegex(AssertionError,'every adjacent window'):review.verify_idle_baseline(w)
 def test_preflight_active_recent_running_or_nonnode_rejects(self):
  for case in ['active','recent','running','nonnode','short']:
   w=self.fixture();p=w['idle_baseline']
   if case=='active':p['first_preflight_processes'][0]['user_ticks']-=1;p['first_preflight_processes'][0]['cpu_ticks']-=1
   if case=='recent':p['preflight_uptime_ticks'][0]=1000+599*os.sysconf('SC_CLK_TCK')
   if case=='running':p['first_preflight_processes'][0]['state']='R'
   if case=='nonnode':p['first_preflight_processes'][0]['name']=p['baseline_processes'][0]['name']='cargo'
   if case=='short':p['preflight_sample_elapsed_ns']-=1
   with self.subTest(case=case),self.assertRaises(AssertionError):review.verify_idle_baseline(w)
 def test_each_baseline_process_must_survive_with_identical_pid_start_name(self):
  for case in ['missing','new','reuse','rename','dead']:
   w=self.fixture();sample=w['idle_baseline']['samples'][-1]
   if case=='missing':sample['processes']=[]
   if case=='new':r=copy.deepcopy(sample['processes'][0]);r['pid']=99;sample['processes'].append(r)
   if case=='reuse':sample['processes'][0]['start_ticks']+=1
   if case=='rename':sample['processes'][0]['name']='node-other'
   if case=='dead':sample['processes'][0]['state']='X'
   with self.subTest(case=case),self.assertRaises(AssertionError):review.verify_idle_baseline(w)
 def test_offsetting_raw_counter_changes_cannot_hide_activity(self):
  w=self.fixture();row=w['idle_baseline']['samples'][-1]['processes'][0];row['user_ticks']-=1;row['waited_child_user_ticks']+=1
  with self.assertRaisesRegex(AssertionError,'each raw CPU counter monotonic'):review.verify_idle_baseline(w)
 def test_aggregate_and_per_process_delta_forgery_rejects(self):
  for case in ['aggregate','per_process','raw_sum','window']:
   w=self.fixture();sample=w['idle_baseline']['samples'][-1]
   if case=='aggregate':sample['aggregate_total_cpu_ticks']=1
   if case=='per_process':sample['per_process_deltas'][0]['window_cpu_ticks']=1
   if case=='raw_sum':sample['processes'][0]['cpu_ticks']=21
   if case=='window':sample['window_ns']+=1
   with self.subTest(case=case),self.assertRaises(AssertionError):review.verify_idle_baseline(w)
 def test_proof_requires_actual_final_inventory_and_complete_lifecycle(self):
  for case in ['final','elapsed','boolean','observation','argv']:
   w=self.fixture()
   if case=='final':w['final_other_heavy_processes']=[]
   if case=='elapsed':w['wall_seconds']=12
   if case=='boolean':w['quiescence_met']=False
   if case=='observation':w['other_heavy_process_observations']=[{'new_job':True}]
   if case=='argv':w['idle_baseline']['samples'][0]['processes'][0]['argv']='private'
   with self.subTest(case=case),self.assertRaises(AssertionError):review.verify_idle_baseline(w)


if __name__ == '__main__':
    unittest.main()
