#!/usr/bin/env python3
"""Trusted all-twelve-root diagnostic. Prepared source, not timing acceptance.

The parent approves a byte-pinned authority AFTER the release test executable and
shared resource helper exist. Never fabricate those pins from this script's run.
The shared helper alone owns systemd/cgroups/signals/watchdog/CPU accounting.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import sys

SCHEMA = 'foundations-same-policy-true-root-diagnostic-v1'
FILTER = 'search_cli::omniscient_diagnostic::same_strategy_omniscient_reference_diagnostic'
SHA = re.compile(r'[0-9a-f]{64}\Z')
INTEGER = re.compile(r'(?:0|-?[1-9][0-9]*)\Z')


def require(ok, why):
    if not ok:
        raise ValueError(why)


def digest(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b''):
            value.update(chunk)
    return value.hexdigest()


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def json_hash(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def strict(path):
    require(Path(path).stat().st_size <= 32 * 1024 * 1024, 'bounded diagnostic JSON')
    def pairs(rows):
        out = {}
        for key, value in rows:
            require(key not in out, 'duplicate JSON key')
            out[key] = value
        return out
    def constant(_):
        raise ValueError('nonfinite JSON number')
    return json.loads(Path(path).read_text(), object_pairs_hook=pairs, parse_constant=constant)


def plain(path):
    path = Path(path)
    require(path.is_absolute(), 'absolute pinned path required')
    require(all(not node.is_symlink() for node in (path, *path.parents)), 'symlink authority/output ancestry')
    require(path.is_file(), 'plain authority file required')
    return path


def approved_stage_root(authority):
    raw = authority['stage_root']
    require(type(raw) is str and raw == str(Path(raw)), 'canonical workspace path string required')
    root = Path(raw)
    require(root.is_absolute() and '..' not in root.parts, 'absolute approved workspace required')
    require(all(not node.is_symlink() for node in (root, *root.parents)), 'symlink workspace ancestry')
    require(root.is_dir() and root.resolve(strict=True) == root, 'plain canonical workspace directory required')
    return root


def quotient_assignment(proposal):
    # Same complete hidden-assignment quotient as core WorldProposal: ignore
    # interchangeable symbols, hand order and independent future RNG/slot labels.
    seats = [[sorted(card['oracle_id'] for card in seat['hidden_hand']),
              [card['oracle_id'] for card in seat['ordered_library']]]
             for seat in proposal['seats']]
    require(len(seats) == 4, 'complete four-seat hypothesis')
    return json_hash([proposal['context_id'], seats])


def utility(value):
    require(type(value) is str and INTEGER.fullmatch(value), 'exact canonical utility string')
    return int(value)


def trial_summary(candidates, trials, assignments):
    require(len(trials) == len(candidates) * 8, 'complete candidate-by-slot denominator')
    pairs = set()
    summaries = []
    for trial in trials:
        slot, index = trial['world_index'], trial['candidate_index']
        require(type(slot) is int and 0 <= slot < 8 and type(index) is int and 0 <= index < len(candidates), 'trial indices')
        require((slot, index) not in pairs, 'duplicate candidate/slot')
        pairs.add((slot, index))
        require(trial['candidate_id'] == candidates[index]['candidate_id'], 'candidate identity drift')
        require(trial['assignment_digest'] == assignments[slot], 'assignment identity drift')
        result = trial['result']
        require(result['outcome'] in ('horizon', 'terminal'), 'non-useful trial cannot become utility')
        require(type(result['state_sha256']) is str and SHA.fullmatch(result['state_sha256']), 'full State SHA')
        values = result['evaluations']
        require(len(values) == 4 and {row['player'] for row in values} == set(range(4)), 'all four exact evaluations')
        for row in values:
            require(type(row['player']) is int, 'player integer')
            utility(row['utility'])
        counts = trial['counts']
        require(set(counts) == {'accepted_inputs', 'advance_calls', 'query_calls', 'rule_steps'}, 'complete work counters')
        require(all(type(x) is int and x >= 0 for x in counts.values()), 'nonnegative integer counters')
        require(0 < counts['accepted_inputs'] <= 256 and counts['advance_calls'] > 0 and counts['query_calls'] > 0, 'bounded useful rollout')
        require(result['outcome'] != 'horizon' or counts['accepted_inputs'] == 256, 'unchanged horizon256')
    for index, candidate in enumerate(candidates):
        selected = [trial for trial in trials if trial['candidate_index'] == index]
        sums = [0] * 4
        for trial in selected:
            for value in trial['result']['evaluations']:
                sums[value['player']] += utility(value['utility'])
        summaries.append({'candidate_index': index, 'candidate_id': candidate['candidate_id'],
            'completed_trials': 8, 'distinct_assignments': len({row['assignment_digest'] for row in selected}),
            'terminal_trials': sum(row['result']['outcome'] == 'terminal' for row in selected),
            'horizon_trials': sum(row['result']['outcome'] == 'horizon' for row in selected),
            'utility_sums': [str(value) for value in sums],
            'counts': {key: sum(row['counts'][key] for row in selected) for key in selected[0]['counts']}})
    return summaries


def validate_report(report, authority):
    require(report['schema'] == SCHEMA and report['status'] == 'same_strategy_true_root_diagnostic_complete', 'complete diagnostic status')
    require(report['engine_build_id'] == authority['engine_build_id'] and report['source_manifest'] == authority['native_source_manifest'], 'actual executable/source identity')
    require(report['root_indices'] == list(range(12)) and len(report['roots']) == 12, 'all12 predeclared roots')
    require(report['horizon_accepted_inputs'] == 256 and report['repeat_slots'] == 8, 'frozen horizon/slots')
    require(report['quality_claim'] is False and report['acceptance_claim'] is False and report['separate_from_product_useful_trial_counts'] is True, 'diagnostic scope')
    sampled_total = true_total = 0
    choices = []
    for index, root in enumerate(report['roots']):
        require(root['root_index'] == index and root['case'] == authority['root_cases'][index], 'fixed root case/order, no root shopping')
        require(root['public_job_seed'] == list(hashlib.sha256(f'commander-open-search-v1/job/{index}'.encode()).digest()), 'public strategy seed')
        require(root['horizon_accepted_inputs'] == 256, 'per-root horizon')
        work = root['frozen_work']
        require(work['version'] == 'foundations-fixed-search-work-v1', 'frozen work version')
        require(work['horizon'] == 256 and work['context_id'] == root['capture_context_id'] and work['public_job_seed'] == root['public_job_seed'], 'frozen permitted work origin')
        require(root['frozen_work_sha256'] == json_hash(work), 'frozen work digest')
        policy = work['candidate_policy']
        require(policy['truncated_at_limit'] is False and 0 < len(policy['candidates']) <= 32, 'complete nonempty production candidate set')
        candidates = policy['candidates']
        require(len({candidate['candidate_id'] for candidate in candidates}) == len(candidates), 'unique generated candidate identities')
        require(root['candidate_set_sha256'] == json_hash(policy), 'same complete candidate identity')
        for candidate in candidates:
            require(candidate['candidate_id'] == json_hash([policy['policy'], candidate['plan']]), 'candidate plan digest')
        worlds = work['worlds']
        require(len(worlds) == 8 and [row['world_index'] for row in worlds] == list(range(8)), 'first8 ordered worlds')
        assignments = []
        for slot, world in enumerate(worlds):
            require(world['proposal']['context_id'] == work['context_id'] and world['proposal']['world_index'] == slot, 'sample proposal capture/slot')
            require(world['assignment_digest'] == quotient_assignment(world['proposal']), 'physical assignment quotient')
            assignments.append(world['assignment_digest'])
        require(len(set(assignments)) == 8, 'eight distinct assignments, not copy labels or RNG')
        execution = root['sampled']['execution']
        require(execution['complete_useful'] is True and execution['worker_faults'] == [] and execution['workers'] == 1, 'complete serial sampled execution')
        require([world['world_index'] for world in execution['worlds']] == list(range(8)), 'all sampled results retained')
        sampled_trials = [trial for world in execution['worlds'] for trial in world['trials']]
        sampled_rows = trial_summary(candidates, sampled_trials, assignments)
        require(sampled_rows == root['sampled']['summary'] and root['sampled']['distinct_worlds'] == 8, 'sampled aggregates')
        actual_sha = root['trusted_actual_root_state_sha256']
        require(type(actual_sha) is str and SHA.fullmatch(actual_sha), 'trusted actual root SHA')
        real = root['true_reference']
        require(real['distinct_worlds'] == 1 and real['repeat_slots'] == 8, 'one actual world, eight repeat slots')
        real_rows = trial_summary(candidates, real['trials'], [actual_sha] * 8)
        require(real_rows == real['summary'], 'actual-world aggregates')
        for candidate in range(len(candidates)):
            repeats = [trial for trial in real['trials'] if trial['candidate_index'] == candidate]
            require(all(trial['counts'] == repeats[0]['counts'] and trial['result'] == repeats[0]['result'] for trial in repeats), 'actual deterministic repeat equality')
        viewer = root['case']['viewer']
        sampled_choice = max(range(len(candidates)), key=lambda i: utility(sampled_rows[i]['utility_sums'][viewer]))
        real_choice = max(range(len(candidates)), key=lambda i: utility(real_rows[i]['utility_sums'][viewer]))
        selection = root['selection']
        require(selection['sampled_candidate_index'] == sampled_choice and selection['true_reference_candidate_index'] == real_choice, 'exact utility maximum/first tie selection')
        require(selection['same_candidate'] == (sampled_choice == real_choice) and selection['sampled_candidate_id'] == candidates[sampled_choice]['candidate_id'] and selection['true_reference_candidate_id'] == candidates[real_choice]['candidate_id'], 'selected plan identity')
        require(selection['true_world_utility_sum_of_sampled_choice'] == real_rows[sampled_choice]['utility_sums'][viewer] and selection['true_world_utility_sum_of_true_choice'] == real_rows[real_choice]['utility_sums'][viewer], 'truth score comparison')
        sampled_total += len(sampled_trials)
        true_total += len(real['trials'])
        choices.append({'root_index': index, 'sampled_candidate': sampled_choice, 'true_reference_candidate': real_choice})
    return {'status': 'all12_fixed_diagnostic_semantics_verified', 'sampled_useful_trials_diagnostic_only': sampled_total,
            'actual_world_repeat_trials_diagnostic_only': true_total, 'actual_distinct_worlds_per_root': 1,
            'selections': choices, 'quality_claim': False, 'acceptance_claim': False}


def metadata_guard(authority):
    # Reuse the reviewed metadata algorithm, authenticated BEFORE importing it.
    # Its module has no top-level shared-helper import or execution side effect.
    row = authority['fixed_driver']
    path = plain(row['path'])
    require(path == Path(__file__).resolve().with_name('run-fixed-work.py'), 'reviewed shared metadata guard location')
    require(digest(path) == row['sha256'], 'shared metadata guard changed before import')
    spec = importlib.util.spec_from_file_location('reviewed_fixed_metadata', path)
    fixed = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = fixed
    spec.loader.exec_module(fixed)
    fixed.load_host(authority['helper']['sha256'])
    return fixed


def source_guard(authority):
    require(authority['native_source_manifest'].get('status') == 'compiled-source-closure-verified', 'compiled source closure required')
    require(authority['schema'] == 'performance-host-authority-v1', 'fixed diagnostic authority')
    stage = approved_stage_root(authority)
    require(SHA.fullmatch(authority['engine_build_id']) and authority['native_source_manifest']['build_id'] == authority['engine_build_id'], 'approved build')
    fixed = metadata_guard(authority)
    fixed.require_target(authority, fixed.SIM_TARGET)
    artifact_row = authority['compiler_artifact']
    require(digest(plain(artifact_row['path'])) == artifact_row['sha256'], 'actual Cargo artifact changed')
    artifact = strict(artifact_row['path'])
    fixed.validate_settings(authority['build_settings'], artifact, authority['test_executable']['sha256'], authority['test_target'])
    manifest = authority['native_manifest']
    require(digest(plain(manifest['path'])) == manifest['sha256'] and strict(manifest['path']) == authority['native_source_manifest'], 'compiled native source manifest authority')
    require(len(authority['root_cases']) == 12 and [case['index'] for case in authority['root_cases']] == list(range(12)), 'predeclared12 cases')
    require(authority['timeout_seconds'] == 120 and len(authority['cpu_ids']) == 1 and type(authority['cpu_ids'][0]) is int and authority['cpu_ids'][0] >= 0, 'fixed whole-diagnostic outer120s/single CPU')
    rows = authority['native_source_manifest']['files']
    require(rows and [row['path'] for row in rows] == sorted({row['path'] for row in rows}), 'complete ordered unique source rows')
    paths = []
    for relative in ('packages/game-engine/crates', 'packages/game-server/src', 'packages/game-server/tests', 'packages/game-engine/tests/fixtures'):
        for base, dirs, files in os.walk(stage/relative, followlinks=False):
            require(not Path(base).is_symlink() and all(not (Path(base)/name).is_symlink() for name in dirs), 'source symlink directory')
            dirs[:] = [name for name in dirs if name not in ('generated', 'target', 'node_modules', '.git')]
            for name in files:
                path = Path(base)/name
                if path.suffix in ('.rs', '.json') or name == 'Cargo.toml':
                    require(not path.is_symlink(), 'source symlink file')
                    paths.append(str(path.relative_to(stage)))
    paths.extend(('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'packages/game-server/Cargo.toml'))
    require([row['path'] for row in rows] == sorted(paths), 'exact compiled source inventory, including new files')
    for row in rows:
        rel = PurePosixPath(row['path'])
        require(not rel.is_absolute() and rel.as_posix() == row['path'] and all(p not in ('.', '..') for p in rel.parts), 'safe source row')
        path = plain(stage / rel)
        require(path.stat().st_size == row['bytes'] and digest(path) == row['sha256'], 'source bytes changed')
    require(fixed.compiled_build_id(stage, rows, authority['build_settings']) == authority['engine_build_id'], 'source/toolchain/settings Build ID mismatch')
    for key in ('test_executable', 'helper', 'driver', 'fixed_driver'):
        row = authority[key]
        require(digest(plain(row['path'])) == row['sha256'], f'{key} changed')
    require(Path(authority['driver']['path']) == Path(__file__).resolve(), 'this reviewed driver required')
    require(Path(authority['helper']['path']) == Path(__file__).resolve().with_name('measurement_host.py'), 'reviewed shared helper location')
    require(authority['contracts'], 'explicit diagnostic contracts required')
    for row in authority['contracts']:
        require(digest(plain(row['path'])) == row['sha256'], 'diagnostic contract changed')
    return {'build_id': authority['engine_build_id'], 'source_manifest_sha256': json_hash(authority['native_source_manifest']),
            'test_executable_sha256': authority['test_executable']['sha256'], 'helper_sha256': authority['helper']['sha256'],
            'driver_sha256': authority['driver']['sha256']}


def resource_attempt(helper, **arguments):
    # Invalid helper arguments reject before it creates an execution directory.
    # Preserve that failed attempt without inventing kernel/resource evidence.
    try:
        return helper.run_bounded(**arguments)
    except Exception as error:
        return {'status': 'failed', 'resource_report_created': False,
                'failure': {'type': type(error).__name__, 'reason': str(error)[:2048]}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--authority', type=Path, required=True)
    parser.add_argument('--expected-authority-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    require(args.execute, 'prepared only until root review and explicit execution authorization')
    os.umask(0o077)
    require(digest(plain(args.authority)) == args.expected_authority_sha256, 'root-approved authority hash')
    authority = strict(args.authority)
    authority['native_source_manifest'] = strict(plain(authority['native_manifest']['path']))
    before = source_guard(authority)
    stage = approved_stage_root(authority)
    output = args.output.absolute()
    require(not output.exists() and not output.is_symlink() and output.parent.is_dir(), 'fresh private output')
    require(all(not p.is_symlink() for p in (output.parent, *output.parent.parents)), 'plain output ancestry')
    output.mkdir(mode=0o700)
    for name in ('home', 'tmp'):
        (output / name).mkdir(mode=0o700)
    shutil.copyfile(args.authority, output / 'authority.json')
    Path(output / 'authority.json').chmod(0o600)
    helper_path = plain(authority['helper']['path'])
    require(digest(helper_path) == authority['helper']['sha256'], 'shared helper changed before import')
    spec = importlib.util.spec_from_file_location('reviewed_measurement_host', helper_path)
    helper = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = helper
    spec.loader.exec_module(helper)
    execution = output / 'execution'
    report_path = execution / 'omniscient-report.json'
    env = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'HOME': str(output / 'home'),
           'TMPDIR': str(output / 'tmp'), 'MTG_OMNISCIENT_SEARCH_OUTPUT': str(report_path)}
    def audit():
        value = validate_report(strict(plain(report_path)), authority)
        require(source_guard(authority) == before and digest(args.authority) == args.expected_authority_sha256, 'source/authority endpoint changed')
        return {**value, 'report_sha256': digest(report_path), 'source_before_after_equal': True}
    row = resource_attempt(helper, command=[authority['test_executable']['path'], '--exact', FILTER,
                              '--ignored', '--nocapture', '--test-threads=1'],
        cwd=stage, env=env, directory=execution, label='omniscient-all12', cpus=authority['cpu_ids'],
        timeout_seconds=120.0, expected_executable_sha256=authority['test_executable']['sha256'], audit=audit)
    passed = row.get('status') == 'passed'
    failure = row.get('failure')
    try:
        if passed:
            require(row.get('audit', {}).get('status') == 'all12_fixed_diagnostic_semantics_verified', 'shared helper must retain actual completed semantic audit')
            # Semantic audit is inside the shared deadline; this extra endpoint
            # only adds rejection authority after cleanup, never time exclusions.
            require(source_guard(authority) == before and digest(args.authority) == args.expected_authority_sha256, 'post-cleanup source changed')
    except Exception as error:
        passed = False
        failure = {'type': type(error).__name__, 'reason': str(error)[:2048]}
    result = {'status': 'trusted_all12_diagnostic_verified' if passed else 'diagnostic_failed_or_truncated',
              'classification': 'same-policy true-world reference; no strategic quality or product latency claim',
              'authority_sha256': args.expected_authority_sha256, 'resource_run': row,
              'failure': failure, 'quality_claim': False, 'acceptance_claim': False, 'product_useful_trials_claimed': 0}
    with (output / 'result.json').open('x') as file:
        json.dump(result, file, sort_keys=True, indent=2, allow_nan=False)
        file.write('\n')
        file.flush()
        os.fsync(file.fileno())
    require(passed, 'diagnostic failed; private attempt and resource evidence retained')



if __name__ == '__main__':
    main()
