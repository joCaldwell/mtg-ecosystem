#!/usr/bin/env python3
"""Guarded finite definition-domain diagnostic. No implicit build or launch."""
import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys

HERE = Path(__file__).absolute().parent
STAGE = HERE.parents[1]
FIXED_SHA = 'fa98a77dd48fa10e6690df18601d8c6bd5ed04179d62bf6094bd2ecbcdaab018'
HELPER_SHA = 'f426db8fe1a0d2896d309937c4bede6be5fd081b00464495bc04ee16e1001648'
ENTRY = 'engine::definition_diversity_tests::definition_diversity_host_diagnostic'
MODULE = 'packages/game-engine/crates/core/src/engine/definition_diversity_tests.rs'
CHILD = 'packages/game-engine/crates/core/src/engine/definition_diversity_tests/heterogeneous.rs'
COUNTS = [0, 8, 64, 256]
HETERO = [(False, False), (False, True), (True, False), (True, True)]
NAMES = ['Blood Artist', 'Essence Warden', 'Elvish Visionary', 'Slaughterhouse Bouncer', 'Furnace of Rath', 'Giant Growth', 'Disfigure', 'Fog']
LIMIT = 64 * 1024**2


def need(ok, reason):
    if not ok:
        raise ValueError(reason)


def plain(path):
    p = Path(path)
    need(p.is_absolute() and p.as_posix() == str(p) and '..' not in p.parts, 'absolute plain path')
    need(p.is_file() and not any(q.is_symlink() for q in (p, *p.parents)), 'plain nonsymlink file')
    return p


def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False, allow_nan=False).encode()


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def read(path):
    p = plain(path)
    need(p.stat().st_size <= LIMIT, 'bounded report')
    return decode(p.read_bytes().decode('utf-8'))


def shape(row, keys, label):
    need(type(row) is dict and set(row) == set(keys.split()), label + ' exact fields')


def number(value, label, low=0, high=(1 << 64) - 1):
    need(type(value) is int and low <= value <= high, label + ' exact integer/range')
    return value


def native_numeric_tree(value, key=None):
    """Rust transport numbers are integers; reject Python numeric aliasing recursively.

    Complete typed equality is still checked at each expected composite. These
    numeric member names also cover raw IDs embedded in retained native outcomes.
    """
    numeric_keys = {'player', 'owner', 'controller', 'viewer', 'active', 'priority',
                    'turn_owner', 'incarnation', 'decision_id', 'boundary',
                    'after_decisions', 'journal_ordinal', 'source_block', 'target_slot',
                    'repetition', 'nanos', 'living', 'quantum', 'setup_inputs',
                    'rule_boundary', 'observation_version', 'hand_count', 'library_count',
                    'lands_played', 'number'}
    need(type(value) is not float, 'native numeric value must not be floating point')
    if key in numeric_keys:
        number(value, 'nested native ' + key)
    if type(value) is dict:
        if 'card' in value and 'incarnation' in value:
            number(value['card'], 'raw ObjectId physical card', high=(1 << 32) - 1)
        for member, child in value.items():
            native_numeric_tree(child, member)
    elif type(value) is list:
        for child in value:
            native_numeric_tree(child)


def quantity(value):
    need(type(value) is str and re.fullmatch(r'0|-?[1-9][0-9]*', value) is not None, 'canonical exact quantity')
    return int(value)


def hexsha(value):
    return type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None


def authenticated(name, path, expected):
    p = plain(path)
    need(sha(p) == expected, 'source hash BEFORE import')
    spec = importlib.util.spec_from_file_location(name, p)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def authority_guard(path, expected, fixed_sha, helper_sha, target, own, sources):
    need(fixed_sha == FIXED_SHA and helper_sha == HELPER_SHA, 'unchanged reviewed shared guards')
    fixed = authenticated('finite_domain_fixed', HERE / 'run-fixed-work.py', fixed_sha)
    host = fixed.load_host(helper_sha)
    a = fixed.authority_check(path, expected)
    fixed.require_target(a, target)
    need(Path(a['stage_root']) == STAGE, 'actual isolated stage')
    artifact = host.read_json(Path(a['compiler_artifact']['path']))
    need(artifact['features'] == [], 'actual baseline Cargo features must be empty')
    need(type(a['timeout_seconds']) is int and a['timeout_seconds'] == 120 and type(a['cpu_ids']) is list and len(a['cpu_ids']) == 1, 'fixed120s/one CPU')
    number(a['cpu_ids'][0], 'CPU')
    for p in [own, HERE / 'run-fixed-work.py', HERE / 'measurement_host.py']:
        need(a['host_scripts'].get(str(p)) == sha(plain(p)), 'explicit host source pin')
    native = host.read_json(Path(a['native_manifest']['path']))
    pinned = {r['path']: r['sha256'] for r in native['files']}
    need(set(a['diagnostic_sources']) == set(sources), 'complete explicit source inventory')
    for rel in sources:
        need(a['diagnostic_sources'][rel] == pinned.get(rel) == sha(plain(STAGE / rel)), 'diagnostic source/compiled manifest equality')
    return a, host, fixed


def one_definition(catalog, name):
    matches = [d for d in catalog['definitions'] if d['name'] == name]
    need(len(matches) == 1, 'unique source definition')
    return matches[0]


def facts(stage):
    fixture = stage / 'packages/game-engine/tests/fixtures'
    effects = read(fixture / 'expected-effects-ir.json')
    choices = read(fixture / 'expected-choices-ir.json')
    bases = [one_definition(effects, n)['characteristics'] for n in ['Yoked Ox', 'Samite Healer', 'Prodigal Sorcerer', 'Blood Artist']]
    programs = []
    ids = {}
    for name in NAMES:
        d = one_definition(choices, name)
        ids[name] = d['oracle_id']
        programs.append({'oracle_id': d['oracle_id'], 'source_provenance': d['provenance'], 'complete_spell': d.get('spell'), 'complete_triggered': d.get('triggered', []), 'complete_static': d.get('static_abilities', [])})
    template = copy.deepcopy(one_definition(effects, 'Giant Growth')['spell']['instructions'][0])
    need(template['op'] == 'modify_power_toughness', 'actual archived modifier template')
    return {'bases': bases, 'template': template, 'programs': programs, 'ids': ids, 'bear': one_definition(choices, 'Grizzly Bears')['characteristics']}


def source_guard(args):
    sources = [MODULE, CHILD, 'packages/game-engine/crates/core/src/engine/scenarios.rs', 'packages/game-engine/crates/core/src/engine/setup.rs']
    a, host, fixed = authority_guard(args.authority, args.authority_sha256, args.fixed_driver_sha256, args.helper_sha256, {'name': 'mtg_core', 'kind': ['lib']}, Path(__file__).absolute(), sources)
    source = plain(STAGE / MODULE).read_text()
    matches = re.findall(r'const CONTRACT: &str = r#"(.*?)"#;', source, re.S)
    need(len(matches) == 1, 'one embedded definition contract')
    contract = decode(matches[0])
    declaration = a['definition_diagnostic']
    need(canonical(declaration) == canonical({'entrypoint': ENTRY, 'contract_sha256': hashlib.sha256(matches[0].encode()).hexdigest(), 'synthetic_counts': COUNTS, 'heterogeneous_cases': [{'enabled': x, 'bouncer_controller_hand_empty': y} for x, y in HETERO], 'reference_policy': 'source_arithmetic_program_facts_and_complete_repeat_equality_v1'}), 'explicit approved worklist/reference policy')
    need(canonical(contract['synthetic_counts']) == canonical(COUNTS) and number(contract['warmups'], 'contract warmups') == 1 and number(contract['measured_repetitions'], 'contract measured repetitions') == 5 and contract['heterogeneous_sources'] == NAMES and contract['horizon_or_pool_changes'] is False, 'source contract scope')
    # These are the four first eligible objects selected by the pinned effects fixture.
    scenario = plain(STAGE / sources[2]).read_text()
    for pattern in [r'1\s*=>\s*OX', r'2\s*\|\s*18\s*=>\s*HEALER', r'3\s*=>\s*PRODIGAL', r'4\s*\|\s*19\s*\|\s*39\s*\|\s*42\s*=>\s*BLOOD_ARTIST']:
        need(re.search(pattern, scenario) is not None, 'source-backed partition bases')
    return a, host, contract, facts(STAGE)


def expected_manifest(count, f):
    result = []
    for index in range(count):
        partition, k = index % 4, index // 4
        instruction = copy.deepcopy(f['template'])
        instruction.update(target_slot=partition, power=str((k + 1) if k % 2 == 0 else -(k + 1)), toughness=str(2 * (k + 1) + partition))
        result.append({'partition': partition, 'instruction': instruction})
    return result


def arithmetic(count, bases):
    result = copy.deepcopy(bases)
    for partition, row in enumerate(result):
        m = count // 4 + int(partition < count % 4)
        row['power'] = str(quantity(row['power']) + ((m + 1) // 2 if m % 2 else -(m // 2)))
        row['toughness'] = str(quantity(row['toughness']) + m * (m + 1) + partition * m)
    return result


def sample(s, operation, rep, evaluators=0, hashes=0):
    shape(s, 'operation repetition nanos evaluator_calls state_hash_calls', 'timing sample')
    need(s['operation'] == operation and number(s['repetition'], 'sample repetition') == rep and number(s['evaluator_calls'], 'evaluator calls') == evaluators and number(s['state_hash_calls'], 'hash calls') == hashes, 'ordered source operation/call count')
    number(s['nanos'], 'segment ns', high=120_000_000_000)


def semantic(row):
    value = copy.deepcopy(row)
    for key in ['repetition', 'warmup', 'samples', 'same_work_semantics_sha256']:
        value.pop(key, None)
    if 'work' in value:
        value['work'].pop('samples')
    return value


def synthetic(row, count, rep, f):
    shape(row, 'row repetition warmup unique_authentic_active_source_programs archived_operation_templates synthetic_unique_canonical_payloads effect_instances selector_partitions complete_payload_manifest canonical_payload_sha256s manifest_sha256 expected_characteristics changed_characteristics full_State_sha256 changed_full_State_sha256 cleanup_expired_instances actual_effect_capacity above_capacity normal_restore_rejected synthetic_public_admission_rejected supported_cast_invocation_claim history_byte_oracles_verified evaluated_objects_per_pass actual_target_incarnation_invalidation_verified samples same_work_semantics_sha256', 'synthetic row')
    expected = expected_manifest(count, f)
    strings = [canonical(p).decode() for p in expected]
    need(len(set(strings)) == count and canonical(row['complete_payload_manifest']) == canonical(expected) and row['canonical_payload_sha256s'] == [hashlib.sha256(s.encode()).hexdigest() for s in strings] and row['manifest_sha256'] == digest(strings), 'complete unique identity-free payload manifest and hashes')
    for key, value in {'unique_authentic_active_source_programs': 0, 'archived_operation_templates': 1, 'synthetic_unique_canonical_payloads': count, 'effect_instances': count, 'selector_partitions': 4, 'cleanup_expired_instances': count, 'actual_effect_capacity': 16, 'evaluated_objects_per_pass': 4}.items():
        need(number(row[key], key) == value, 'synthetic count conservation')
    for key, value in {'above_capacity': count > 16, 'normal_restore_rejected': count > 0, 'synthetic_public_admission_rejected': True, 'supported_cast_invocation_claim': False, 'history_byte_oracles_verified': True, 'actual_target_incarnation_invalidation_verified': True}.items():
        need(row[key] is value, 'honest evaluator/admission scope')
    expected = arithmetic(count, f['bases'])
    changed = copy.deepcopy(expected)
    if count:
        changed[0]['power'] = str(quantity(changed[0]['power']) + 1)
        changed[0]['toughness'] = str(quantity(changed[0]['toughness']) + 2)
    need(canonical(row['expected_characteristics']) == canonical(expected) and canonical(row['changed_characteristics']) == canonical(changed), 'independent closed-form arithmetic/full characteristics')
    need(hexsha(row['full_State_sha256']) and hexsha(row['changed_full_State_sha256']) and (row['full_State_sha256'] == row['changed_full_State_sha256']) is (count == 0), 'full State change/no-op identity')
    operations = [('characteristics_first_no_property_cache', 4, 0), ('characteristics_repeat_no_property_cache', 4, 0), ('full_State_hash_cold_history_memos', 0, 1), ('full_State_hash_warm_history_memos', 0, 1), ('same_identity_payload_change_characteristics', 4, 0), ('production_target_incarnation_invalidation', 0, 0), ('production_cleanup_effects_event', 0, 0)]
    need(len(row['samples']) == len(operations), 'complete seven source segments')
    for s, (op, ev, hashes) in zip(row['samples'], operations):
        sample(s, op, rep, ev, hashes)


def heterogeneous(row, enabled, empty, rep, f):
    shape(row, 'row repetition warmup enabled bouncer_controller_hand_empty unique_authentic_source_programs complete_source_manifest peak_trigger_definition_instances peak_static_definition_instances peak_resolved_effect_instances unique_synthetic_generated_payloads unique_authentic_complete_semantic_payloads actually_paid_completed_spells resolved_effect_instances_before_cleanup static_provider_instances actual_trigger_matches matched_source_subject_pairs intervening_condition_checks condition_result noncreature_ETB_trigger_matches matcher_internal_comparisons_not_instrumented damage_replacement_probe cleanup_expired_instances static_provider_remains_after_cleanup before_cleanup_full_State_sha256 after_cleanup_full_State_sha256 typed_fixture_move_batches_not_normal_game_reachability pending_trigger_placement_or_resolution_claim synthetic_configuration_admission_rejected work same_work_semantics_sha256', 'heterogeneous row')
    need(row['enabled'] is enabled and row['bouncer_controller_hand_empty'] is empty, 'fixed hetero configuration')
    need(canonical(row['complete_source_manifest']) == canonical(f['programs']) and len({digest([p['complete_spell'], p['complete_triggered'], p['complete_static']]) for p in row['complete_source_manifest']}) == 8, 'all eight exact authentic complete source programs')
    e = int(enabled)
    expected_pairs = [[f['ids']['Essence Warden'], 2], [f['ids']['Elvish Visionary'], 2], [f['ids']['Blood Artist'], 9], [f['ids']['Blood Artist'], 3]] + ([[f['ids']['Slaughterhouse Bouncer'], 3]] if empty else []) if enabled else []
    pairs = row['matched_source_subject_pairs']
    need(type(pairs) is list, 'typed match pairs')
    for pair in pairs:
        need(type(pair) is list and len(pair) == 2 and type(pair[0]) is str and pair[0] in f['ids'].values(), 'actual source ID and pair shape')
        number(pair[1], 'matched physical subject CardId', high=(1 << 32) - 1)
    need(type(pairs) is list and len(pairs) == len(expected_pairs) and canonical(sorted(pairs)) == canonical(sorted(expected_pairs)), 'actual match pairs/no duplicate or omitted matches')
    for key, value in {'unique_authentic_source_programs': 8 * e, 'peak_trigger_definition_instances': 4 * e, 'peak_static_definition_instances': e, 'peak_resolved_effect_instances': 3 * e, 'unique_synthetic_generated_payloads': 0, 'unique_authentic_complete_semantic_payloads': 8 * e, 'actually_paid_completed_spells': 3 * e, 'resolved_effect_instances_before_cleanup': 3 * e, 'static_provider_instances': e, 'actual_trigger_matches': len(expected_pairs), 'intervening_condition_checks': e, 'noncreature_ETB_trigger_matches': 0, 'cleanup_expired_instances': 3 * e}.items():
        need(number(row[key], key) == value, 'heterogeneous instance/work conservation')
    need(row['condition_result'] is (empty if enabled else None), 'exact intervening-if result')
    for key, value in {'matcher_internal_comparisons_not_instrumented': True, 'static_provider_remains_after_cleanup': enabled, 'typed_fixture_move_batches_not_normal_game_reachability': True, 'pending_trigger_placement_or_resolution_claim': False, 'synthetic_configuration_admission_rejected': True}.items():
        need(row[key] is value, 'finite heterogeneous scope')
    probe = row['damage_replacement_probe']
    if enabled:
        original = quantity(f['bear']['power']) + 1
        need(canonical(probe) == canonical({'eligible_pairs': 2, 'applied': 2, 'original_amount': original, 'amounts_after': [original * 2, 0], 'actual_damage_commit_claim': False}), 'actual source power/Furnace/Fog probe, not committed damage')
    else:
        need(canonical(probe) == canonical({'eligible_pairs': 0, 'applied': 0, 'original_amount': quantity(f['bear']['power'])}), 'disabled source power probe has no interactions')
    need(hexsha(row['before_cleanup_full_State_sha256']) and hexsha(row['after_cleanup_full_State_sha256']), 'complete before/after State hashes')
    work = row['work']
    shape(work, 'accepted_inputs advance_calls typed_fixture_moves samples', 'heterogeneous work')
    need(number(work['accepted_inputs'], 'hetero accepted') == 26 * e and number(work['advance_calls'], 'hetero advance') == 29 * e and number(work['typed_fixture_moves'], 'fixture moves') == 4, 'actual source call/input/move counts')
    operations = [s['operation'] for s in work['samples']]
    expected_hist = {'actual_respond': 26 * e, 'actual_advance': 29 * e, 'prepare_fixture_combat_damage': 1, 'actual_damage_replacement_candidate_query': 1, 'actual_damage_replacement_application': 2 * e, 'actual_typed_fixture_move_and_trigger_collection': 4, 'actual_cleanup_expiry': 1}
    need({op: operations.count(op) for op in set(operations)} == {op: n for op, n in expected_hist.items() if n}, 'complete counted work timing samples')
    advance, respond = 'actual_advance', 'actual_respond'
    casts = ([advance, respond] * 9 + [advance]) * 2 + [advance, respond] * 8 + [advance] if enabled else []
    ordered = casts + ['prepare_fixture_combat_damage', 'actual_damage_replacement_candidate_query'] + ['actual_damage_replacement_application'] * (2 * e) + ['actual_typed_fixture_move_and_trigger_collection'] * 4 + ['actual_cleanup_expiry']
    need(operations == ordered, 'source segment order, no histogram-only substitution')
    for s in work['samples']:
        sample(s, s['operation'], rep)


def validate_report(report, build, contract, f):
    native_numeric_tree(report)
    shape(report, 'schema status expected_rows build_id contract contract_sha256 warmups measured_repetitions rows outer_guard_declaration_is_not_OS_proof authentic256_Oracle_ability_claim legal_pool_expansion product_latency_or_quality_claim', 'definition report')
    need(report['schema'] == 'definition-diversity-diagnostic-v1' and report['status'] == 'all_declared_rows_verified' and report['build_id'] == build and canonical(report['contract']) == canonical(contract), 'complete actual source report')
    need(report['contract_sha256'] == f['contract_sha256'], 'raw embedded contract SHA')
    need(number(report['expected_rows'], 'declared rows') == 48 and number(report['warmups'], 'warmups') == 1 and number(report['measured_repetitions'], 'repetitions') == 5 and len(report['rows']) == 48, 'all48 rows denominator')
    for key, value in {'outer_guard_declaration_is_not_OS_proof': True, 'authentic256_Oracle_ability_claim': False, 'legal_pool_expansion': False, 'product_latency_or_quality_claim': False}.items():
        need(report[key] is value, 'no unsupported claim')
    references = {}
    starts = []
    for rep in range(6):
        for slot in range(8):
            row = report['rows'][rep * 8 + slot]
            need(number(row['repetition'], 'row repetition') == rep and row['warmup'] is (rep == 0), 'fixed 1warm+5 row order')
            if slot < 4:
                count = COUNTS[slot]
                need(row['row'] == 'synthetic_unique_modifier_payloads', 'synthetic row order')
                synthetic(row, count, rep, f)
                starts.append({'row': row['row'], 'count': count, 'repetition': rep})
            else:
                enabled, empty = HETERO[slot - 4]
                need(row['row'] == 'source_backed_heterogeneous', 'authentic row order')
                heterogeneous(row, enabled, empty, rep, f)
                starts.append({'row': row['row'], 'enabled': enabled, 'bouncer_controller_hand_empty': empty, 'repetition': rep})
            value = semantic(row)
            need(row['same_work_semantics_sha256'] == digest(value), 'all-field native semantic SHA independently recomputed')
            if rep == 0:
                references[slot] = value
            else:
                need(canonical(value) == canonical(references[slot]), 'complete all-field same-work equality; only segment clocks removed')
    return {'status': 'all48_rows_full_semantics_and_source_facts_verified', 'rows': 48, 'configurations': 8, 'warmup_rows': 8, 'measured_rows': 40, 'starts': starts, 'native_oracle_scope': 'Full generic State/arithmetic/replay/invalidation and actual typed collection proofs remain native. Host checks full emitted values, independent arithmetic/manifests/source facts, call conservation, ordering and complete repeat equality; not a rules executor.', 'authentic_256_claim': False, 'product_latency_claim': False}


def stdout_records(path, entry, markers):
    raw = plain(path).read_bytes()
    need(len(raw) <= LIMIT and raw.endswith(b'\n') and b'\0' not in raw and b'\r' not in raw, 'complete bounded UTF8 stdout')
    started = ended = running = trailer = False
    events = []
    for line in raw.decode('utf-8').splitlines():
        if not line:
            continue
        if line == 'running 1 test':
            need(not running and not started, 'one initial running header')
            running = True
            continue
        prefix = 'test ' + entry + ' ... '
        if line.startswith(prefix):
            need(running and not started, 'one exact ignored test start')
            started = True
            line = line[len(prefix):]
        if line == 'ok':
            need(started and not ended, 'one successful exact test end')
            ended = True
            continue
        if re.fullmatch(r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(?:\.[0-9]+)?s', line):
            need(ended and not trailer, 'one success trailer')
            trailer = True
            continue
        matched = False
        for marker in markers:
            if line.startswith(marker + '='):
                need(started and not ended, 'markers belong inside exact test')
                events.append((marker, decode(line[len(marker) + 1:])))
                matched = True
                break
        need(matched, 'unexpected stdout record')
    need(running and started and ended and trailer, 'one complete successful ignored entry')
    return events


def partial_diagnostic(directory, report):
    result = {'scope': 'post_failure_diagnostic_outside_bounded_row_NOT_PASS', 'rates_or_verified_rows_claimed': 0}
    for name, path in [('report', report), ('stdout', directory / 'execution/workload.stdout'), ('stderr', directory / 'execution/workload.stderr')]:
        if path.exists() and path.is_file() and not path.is_symlink():
            result[name] = {'sha256': sha(path), 'bytes': path.stat().st_size}
    if report.exists():
        try:
            value = read(report)
            result['native_status'] = value.get('status')
            result['completed_rows_or_cells_observed'] = len(value.get('rows', value.get('cells', [])))
            result['in_progress_cell'] = value.get('in_progress_cell')
            result['preparation_progress'] = value.get('preparation_progress')
        except Exception as error:
            result['partial_parse_error'] = str(error)[:1024]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--authority', type=Path, required=True)
    parser.add_argument('--authority-sha256', required=True)
    parser.add_argument('--fixed-driver-sha256', required=True)
    parser.add_argument('--helper-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    need(args.execute, 'explicit root execution release required')
    os.umask(0o077)
    a, host, contract, f = source_guard(args)
    f['contract_sha256'] = a['definition_diagnostic']['contract_sha256']
    host.real_path(args.output.parent, private=True)
    need(not args.output.exists() and not args.output.is_symlink(), 'fresh output')
    args.output.mkdir(mode=0o700)
    (args.output / 'tmp').mkdir(mode=0o700)
    report = args.output / 'execution/native-report.json'
    def audit():
        again = source_guard(args)
        need(again[0] == a, 'source endpoint')
        value = read(report)
        result = validate_report(value, a['engine_build_id'], contract, f)
        events = stdout_records(args.output / 'execution/workload.stdout', ENTRY, ['DEFINITION_DIVERSITY_ROW_START', 'DEFINITION_DIVERSITY_ROW_RESULT'])
        need(canonical(events) == canonical([(marker, payload) for start, row in zip(result['starts'], value['rows']) for marker, payload in [('DEFINITION_DIVERSITY_ROW_START', start), ('DEFINITION_DIVERSITY_ROW_RESULT', row)]]), 'all48 exact stdout start/result pairs equal full report')
        need(source_guard(args)[0] == a, 'postsemantic source endpoint')
        result.pop('starts')
        return {**result, 'raw_report_sha256': sha(report), 'raw_stdout_sha256': sha(args.output / 'execution/workload.stdout')}
    try:
        row = host.run_bounded(command=[a['test_executable']['path'], '--exact', '--ignored', '--nocapture', '--test-threads=1', ENTRY], cwd=STAGE, env={'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'TMPDIR': str(args.output / 'tmp'), 'DEFINITION_DIVERSITY_OUTER_GUARD': '120s-2gib-v1', 'MTG_DEFINITION_DIVERSITY_OUTPUT': str(report)}, directory=args.output / 'execution', label='definition-diversity', cpus=a['cpu_ids'], timeout_seconds=120, expected_executable_sha256=a['test_executable']['sha256'], audit=audit)
    except BaseException as error:
        row = {'status': 'failed', 'failure': {'type': type(error).__name__, 'reason': str(error)[:2048]}, 'resource_report_created': False}
    try:
        need(source_guard(args)[0] == a, 'final source endpoint')
    except Exception as error:
        row['status'] = 'failed'
        row['failure'] = {'type': type(error).__name__, 'reason': str(error)[:2048]}
    host.save_json(args.output / 'result.json', {'schema': 'definition-diversity-host-result-v1', 'status': 'verified_all_declared_work' if row['status'] == 'passed' else 'failed_or_partial_no_denominator_reduction', 'engine_build_id': a['engine_build_id'], 'authority_sha256': args.authority_sha256, 'resource_report_sha256': sha(args.output / 'execution/host-report.json') if (args.output / 'execution/host-report.json').exists() else None, 'audit': row.get('audit') if row['status'] == 'passed' else None, 'failure': row.get('failure'), 'partial': partial_diagnostic(args.output, report) if row['status'] != 'passed' else None, 'new_pool_or_latency_claim': False})
    return 0 if row['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
