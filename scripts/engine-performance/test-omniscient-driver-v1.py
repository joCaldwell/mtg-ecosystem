#!/usr/bin/env python3
"""Pure semantic fixtures: no game, systemd, profile, timer or native process."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('omniscient_driver', HERE/'run-omniscient-diagnostic-v1.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def fixture():
    source = {'status': 'compiled-source-closure-verified', 'build_id': 'a' * 64, 'files': []}
    authority = {'engine_build_id': 'a' * 64, 'native_source_manifest': source,
                 'root_cases': [{'index': i, 'viewer': i % 4} for i in range(12)]}
    report = {'schema': m.SCHEMA, 'status': 'same_strategy_true_root_diagnostic_complete',
              'engine_build_id': 'a' * 64, 'source_manifest': source, 'root_indices': list(range(12)),
              'horizon_accepted_inputs': 256, 'repeat_slots': 8, 'quality_claim': False,
              'acceptance_claim': False, 'separate_from_product_useful_trial_counts': True, 'roots': []}
    counts = {'accepted_inputs': 256, 'advance_calls': 257, 'query_calls': 1000, 'rule_steps': 600}
    for index in range(12):
        context = hashlib.sha256(f'context/{index}'.encode()).hexdigest()
        actual_sha = hashlib.sha256(f'actual/{index}'.encode()).hexdigest()
        policy = {'policy': 'foundations-public-plans-v1', 'truncated_at_limit': False,
                  'candidates': [{'plan': {'action': 'pass'}}]}
        candidate = policy['candidates'][0]
        candidate['candidate_id'] = m.json_hash([policy['policy'], candidate['plan']])
        worlds = []
        sampled_trials = []
        real_trials = []
        result = {'outcome': 'horizon', 'state_sha256': 'b' * 64,
                  'evaluations': [{'player': p, 'utility': str(p + 1), 'public_status': 'live'} for p in range(4)]}
        for slot in range(8):
            proposal = {'context_id': context, 'world_index': slot,
                        'seats': [{'hidden_hand': [], 'ordered_library': [{'oracle_id': f'card-{slot}-{seat}'}]} for seat in range(4)],
                        'future_seed': [slot] * 32}
            assignment = m.quotient_assignment(proposal)
            worlds.append({'world_index': slot, 'sampling_attempt': 0,
                           'assignment_digest': assignment, 'proposal': proposal})
            trial = {'world_index': slot, 'assignment_digest': assignment, 'candidate_index': 0,
                     'candidate_id': candidate['candidate_id'], 'counts': counts.copy(), 'result': copy.deepcopy(result)}
            sampled_trials.append(trial)
            real = copy.deepcopy(trial)
            real['assignment_digest'] = actual_sha
            real_trials.append(real)
        seed = list(hashlib.sha256(f'commander-open-search-v1/job/{index}'.encode()).digest())
        work = {'version': 'foundations-fixed-search-work-v1', 'horizon': 256, 'context_id': context, 'public_job_seed': seed,
                'candidate_policy': policy, 'worlds': worlds}
        def summary(distinct):
            return [{'candidate_index': 0, 'candidate_id': candidate['candidate_id'], 'completed_trials': 8,
                     'distinct_assignments': distinct, 'terminal_trials': 0, 'horizon_trials': 8,
                     'utility_sums': ['8', '16', '24', '32'],
                     'counts': {'accepted_inputs': 2048, 'advance_calls': 2056, 'query_calls': 8000, 'rule_steps': 4800}}]
        viewer = index % 4
        report['roots'].append({'root_index': index, 'case': authority['root_cases'][index],
            'public_job_seed': seed, 'horizon_accepted_inputs': 256, 'capture_context_id': context,
            'frozen_work': work, 'frozen_work_sha256': m.json_hash(work), 'candidate_set_sha256': m.json_hash(policy),
            'sampled': {'distinct_worlds': 8, 'summary': summary(8), 'execution': {'complete_useful': True,
                'workers': 1, 'worker_faults': [], 'worlds': [{'world_index': slot, 'trials': [trial]} for slot, trial in enumerate(sampled_trials)]}},
            'trusted_actual_root_state_sha256': actual_sha,
            'true_reference': {'distinct_worlds': 1, 'repeat_slots': 8, 'trials': real_trials, 'summary': summary(1)},
            'selection': {'sampled_candidate_index': 0, 'true_reference_candidate_index': 0, 'same_candidate': True,
                'sampled_candidate_id': candidate['candidate_id'], 'true_reference_candidate_id': candidate['candidate_id'],
                'true_world_utility_sum_of_sampled_choice': str((viewer + 1) * 8),
                'true_world_utility_sum_of_true_choice': str((viewer + 1) * 8)}})
    return report, authority


def portable_authority(root):
    files = {'Cargo.toml': '[workspace]\n', 'Cargo.lock': '# locked fixture\n',
             'rust-toolchain.toml': '[toolchain]\nchannel = "fixture"\n',
             'packages/game-server/Cargo.toml': '[package]\nname = "fixture"\n',
             'packages/game-engine/crates/core/src/lib.rs': '// source fixture\n'}
    for relative, value in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)
    executable = root / 'test-executable'
    executable.write_text('pinned fixture only, never executed')
    contract = root / 'contract.json'
    contract.write_text('{"contract":"fixture"}')
    def pin(path): return {'path': str(path), 'sha256': m.digest(path)}
    settings = {'rustc': 'rustc 1.99.0 (fixture 2026-10-15)', 'profile': 'release',
                'target': 'x86_64-unknown-linux-gnu', 'rustflags': '', 'opt_level': '3',
                'debug': 'false', 'debug_assertions': 'false', 'overflow_checks': True}
    artifact = root / 'cargo-artifact.json'
    artifact.write_text(json.dumps({'reason': 'compiler-artifact',
        'target': {'name': 'mtg-sim', 'kind': ['bin']}, 'profile': {'test': True,
        'opt_level': '3', 'debuginfo': 0, 'debug_assertions': False, 'overflow_checks': True},
        'executable': str(executable)}))
    authority = {'schema': 'performance-host-authority-v1', 'stage_root': str(root),
            'build_settings': settings, 'test_target': {'name': 'mtg-sim', 'kind': ['bin']},
            'compiler_artifact': pin(artifact), 'test_executable': pin(executable),
            'helper': pin(HERE / 'measurement_host.py'), 'driver': pin(Path(m.__file__)),
            'fixed_driver': pin(HERE / 'run-fixed-work.py'), 'contracts': [pin(contract)],
            'root_cases': [{'index': i} for i in range(12)], 'timeout_seconds': 120, 'cpu_ids': [0]}
    fixed = m.metadata_guard(authority)
    rows = [{'path': relative, 'sha256': m.digest(root / relative),
             'bytes': (root / relative).stat().st_size} for relative in sorted(files)]
    build = fixed.compiled_build_id(root, rows, settings)
    manifest = {'status': 'compiled-source-closure-verified', 'build_id': build, 'files': rows}
    manifest_path = root / 'compiled-manifest.json'
    manifest_path.write_text(json.dumps(manifest))
    authority.update(engine_build_id=build, native_source_manifest=manifest,
                     native_manifest=pin(manifest_path))
    return authority


class Guards(unittest.TestCase):
    def test_complete_twelve_roots_keep_diagnostic_denominators(self):
        report, authority = fixture()
        proof = m.validate_report(report, authority)
        self.assertEqual(proof['sampled_useful_trials_diagnostic_only'], 96)
        self.assertEqual(proof['actual_world_repeat_trials_diagnostic_only'], 96)
        self.assertEqual(proof['actual_distinct_worlds_per_root'], 1)
        self.assertFalse(proof['acceptance_claim'])

    def test_missing_root_wrong_source_or_case_reject(self):
        for mutation in ('missing', 'case', 'build'):
            report, authority = fixture()
            if mutation == 'missing': report['roots'].pop()
            if mutation == 'case': report['roots'][0]['case'] = {'index': 0, 'viewer': 3}
            if mutation == 'build': report['engine_build_id'] = 'f' * 64
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): m.validate_report(report, authority)

    def test_duplicate_or_missing_slot_reject(self):
        for mutation in ('missing', 'duplicate', 'candidate'):
            report, authority = fixture()
            trials = report['roots'][0]['true_reference']['trials']
            if mutation == 'missing': trials.pop()
            if mutation == 'duplicate': trials[7]['world_index'] = 6
            if mutation == 'candidate': trials[7]['candidate_id'] = 'not-the-plan'
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): m.validate_report(report, authority)

    def test_true_world_repeats_cannot_be_samples_or_change_state(self):
        for mutation in ('distinct', 'state', 'counts'):
            report, authority = fixture()
            real = report['roots'][0]['true_reference']
            if mutation == 'distinct': real['distinct_worlds'] = 8
            if mutation == 'state': real['trials'][1]['result']['state_sha256'] = 'c' * 64
            if mutation == 'counts': real['trials'][1]['counts']['query_calls'] += 1
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): m.validate_report(report, authority)

    def test_labels_and_rng_do_not_create_distinct_assignment(self):
        report, authority = fixture()
        root = report['roots'][0]
        work = root['frozen_work']
        duplicate = copy.deepcopy(work['worlds'][0]['proposal'])
        duplicate['world_index'] = 1
        duplicate['future_seed'] = [99] * 32
        work['worlds'][1]['proposal'] = duplicate
        work['worlds'][1]['assignment_digest'] = m.quotient_assignment(duplicate)
        root['frozen_work_sha256'] = m.json_hash(work)
        with self.assertRaisesRegex(ValueError, 'distinct assignments'): m.validate_report(report, authority)

    def test_truncation_or_shortened_horizon_never_useful(self):
        for mutation in ('resource', 'short'):
            report, authority = fixture()
            trial = report['roots'][0]['sampled']['execution']['worlds'][0]['trials'][0]
            if mutation == 'resource': trial['result'] = {'outcome': 'resource_truncated', 'reason': 'deadline'}
            else: trial['counts']['accepted_inputs'] = 255
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): m.validate_report(report, authority)

    def test_wrong_selection_or_decimal_utility_reject(self):
        for mutation in ('selection', 'quantity'):
            report, authority = fixture()
            if mutation == 'selection': report['roots'][0]['selection']['sampled_candidate_index'] = 1
            else: report['roots'][0]['true_reference']['trials'][0]['result']['evaluations'][0]['utility'] = '1.0'
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): m.validate_report(report, authority)

    def test_resource_preflight_failure_is_preserved_without_fabricated_report(self):
        class Helper:
            def run_bounded(self, **arguments):
                self.arguments = arguments
                raise ValueError('unavailable physical CPU')
        helper = Helper()
        row = m.resource_attempt(helper, command=['/pinned/native'], cpus=[999])
        self.assertEqual(helper.arguments, {'command': ['/pinned/native'], 'cpus': [999]})
        self.assertEqual(row['status'], 'failed')
        self.assertFalse(row['resource_report_created'])
        self.assertEqual(row['failure'], {'type': 'ValueError', 'reason': 'unavailable physical CPU'})
        self.assertNotIn('kernel_initial', row)
        self.assertNotIn('cleanup', row)


    def test_approved_absolute_workspace_is_portable_with_exact_inventory(self):
        with tempfile.TemporaryDirectory() as tmp:
            first, second = Path(tmp) / 'first', Path(tmp) / 'relocated'
            first.mkdir(); second.mkdir()
            a, b = portable_authority(first), portable_authority(second)
            self.assertEqual(m.approved_stage_root(a), first)
            self.assertEqual(m.approved_stage_root(b), second)
            self.assertEqual(m.source_guard(a), m.source_guard(b))
            # Relocation is permitted; unlisted source and modified bytes are not.
            extra = second / 'packages/game-engine/crates/core/src/extra.rs'
            extra.write_text('// unlisted')
            with self.assertRaisesRegex(ValueError, 'inventory'): m.source_guard(b)
            extra.unlink()
            (second / 'packages/game-engine/crates/core/src/lib.rs').write_text('// changed')
            with self.assertRaisesRegex(ValueError, 'source bytes'): m.source_guard(b)

    def test_relative_parent_escape_and_symlinked_workspace_reject(self):
        with tempfile.TemporaryDirectory() as tmp:
            real = Path(tmp) / 'real'
            real.mkdir()
            a = portable_authority(real)
            alias = Path(tmp) / 'alias'
            alias.symlink_to(real, target_is_directory=True)
            for value in ['relative', str(real / '..' / 'real'), str(alias)]:
                bad = dict(a, stage_root=value)
                with self.subTest(stage_root=value), self.assertRaises(ValueError): m.source_guard(bad)


    def test_sim_target_settings_and_cargo_overflow_evidence_are_mandatory(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            a = portable_authority(root)
            for change in [lambda v: v['build_settings'].pop('rustc'),
                           lambda v: v['build_settings'].update(debug_assertions=False),
                           lambda v: v['build_settings'].update(overflow_checks=False),
                           lambda v: v['build_settings'].update(rustc='rustc changed'),
                           lambda v: v.update(test_target={'name': 'mtg_core', 'kind': ['lib']})]:
                bad = copy.deepcopy(a); change(bad)
                with self.assertRaises(ValueError): m.source_guard(bad)
            artifact = Path(a['compiler_artifact']['path'])
            value = json.loads(artifact.read_text()); value['profile']['overflow_checks'] = False
            artifact.write_text(json.dumps(value))
            a['compiler_artifact']['sha256'] = m.digest(artifact)
            with self.assertRaisesRegex(ValueError, 'Cargo artifact'): m.source_guard(a)


    def test_raw_json_duplicates_and_nonfinite_numbers_reject(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'metadata.json'
            for raw in ['{"x":1,"x":2}', '{"x":NaN}', '{"x":Infinity}']:
                path.write_text(raw)
                with self.assertRaises(ValueError): m.strict(path)


if __name__ == '__main__': unittest.main()
