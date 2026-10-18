"""Pure host-validator fixtures; none are engine or benchmark evidence."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

HERE = Path(__file__).absolute().parent
spec = importlib.util.spec_from_file_location('definition_host_tested', HERE / 'run-definition-diversity-v1.py')
host = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host)


def facts():
    programs = [{'oracle_id': f'fake-{i}', 'source_provenance': {'fixture': True}, 'complete_spell': {'test_program': i}, 'complete_triggered': [], 'complete_static': []} for i in range(8)]
    return {'contract_sha256': 'c' * 64, 'bases': [{'power': str(i), 'toughness': str(i + 1)} for i in range(4)], 'template': {'op': 'modify_power_toughness', 'target_slot': 0, 'power': '3', 'toughness': '3', 'duration': 'until_end_of_turn', 'source_block': 0}, 'programs': programs, 'ids': {n: f'fake-{i}' for i, n in enumerate(host.NAMES)}, 'bear': {'power': '2'}}


def row_synthetic(count, rep, f):
    manifest = host.expected_manifest(count, f)
    strings = [host.canonical(p).decode() for p in manifest]
    expected = host.arithmetic(count, f['bases'])
    changed = copy.deepcopy(expected)
    if count:
        changed[0]['power'] = str(int(changed[0]['power']) + 1)
        changed[0]['toughness'] = str(int(changed[0]['toughness']) + 2)
    ops = [('characteristics_first_no_property_cache', 4, 0), ('characteristics_repeat_no_property_cache', 4, 0), ('full_State_hash_cold_history_memos', 0, 1), ('full_State_hash_warm_history_memos', 0, 1), ('same_identity_payload_change_characteristics', 4, 0), ('production_target_incarnation_invalidation', 0, 0), ('production_cleanup_effects_event', 0, 0)]
    return {'row': 'synthetic_unique_modifier_payloads', 'repetition': rep, 'warmup': rep == 0, 'unique_authentic_active_source_programs': 0, 'archived_operation_templates': 1, 'synthetic_unique_canonical_payloads': count, 'effect_instances': count, 'selector_partitions': 4, 'complete_payload_manifest': manifest, 'canonical_payload_sha256s': [host.digest(p) for p in manifest], 'manifest_sha256': host.digest(strings), 'expected_characteristics': expected, 'changed_characteristics': changed, 'full_State_sha256': 'a' * 64, 'changed_full_State_sha256': ('b' if count else 'a') * 64, 'cleanup_expired_instances': count, 'actual_effect_capacity': 16, 'above_capacity': count > 16, 'normal_restore_rejected': count > 0, 'synthetic_public_admission_rejected': True, 'supported_cast_invocation_claim': False, 'history_byte_oracles_verified': True, 'evaluated_objects_per_pass': 4, 'actual_target_incarnation_invalidation_verified': True, 'samples': [{'operation': op, 'repetition': rep, 'nanos': 1, 'evaluator_calls': ev, 'state_hash_calls': hashes} for op, ev, hashes in ops]}


def row_hetero(enabled, empty, rep, f):
    e = int(enabled)
    pairs = [[f['ids']['Essence Warden'], 2], [f['ids']['Elvish Visionary'], 2], [f['ids']['Blood Artist'], 9], [f['ids']['Blood Artist'], 3]] + ([[f['ids']['Slaughterhouse Bouncer'], 3]] if empty else []) if enabled else []
    a, r = 'actual_advance', 'actual_respond'
    casts = ([a, r] * 9 + [a]) * 2 + [a, r] * 8 + [a] if enabled else []
    ops = casts + ['prepare_fixture_combat_damage', 'actual_damage_replacement_candidate_query'] + ['actual_damage_replacement_application'] * (2 * e) + ['actual_typed_fixture_move_and_trigger_collection'] * 4 + ['actual_cleanup_expiry']
    return {'row': 'source_backed_heterogeneous', 'repetition': rep, 'warmup': rep == 0, 'enabled': enabled, 'bouncer_controller_hand_empty': empty, 'unique_authentic_source_programs': 8 * e, 'complete_source_manifest': f['programs'], 'peak_trigger_definition_instances': 4 * e, 'peak_static_definition_instances': e, 'peak_resolved_effect_instances': 3 * e, 'unique_synthetic_generated_payloads': 0, 'unique_authentic_complete_semantic_payloads': 8 * e, 'actually_paid_completed_spells': 3 * e, 'resolved_effect_instances_before_cleanup': 3 * e, 'static_provider_instances': e, 'actual_trigger_matches': len(pairs), 'matched_source_subject_pairs': pairs, 'intervening_condition_checks': e, 'condition_result': empty if enabled else None, 'noncreature_ETB_trigger_matches': 0, 'matcher_internal_comparisons_not_instrumented': True, 'damage_replacement_probe': {'eligible_pairs': 2, 'applied': 2, 'original_amount': 3, 'amounts_after': [6, 0], 'actual_damage_commit_claim': False} if enabled else {'eligible_pairs': 0, 'applied': 0, 'original_amount': 2}, 'cleanup_expired_instances': 3 * e, 'static_provider_remains_after_cleanup': enabled, 'before_cleanup_full_State_sha256': 'd' * 64, 'after_cleanup_full_State_sha256': 'e' * 64, 'typed_fixture_move_batches_not_normal_game_reachability': True, 'pending_trigger_placement_or_resolution_claim': False, 'synthetic_configuration_admission_rejected': True, 'work': {'accepted_inputs': 26 * e, 'advance_calls': 29 * e, 'typed_fixture_moves': 4, 'samples': [{'operation': op, 'repetition': rep, 'nanos': 1, 'evaluator_calls': 0, 'state_hash_calls': 0} for op in ops]}}


def fixture():
    f = facts()
    rows = []
    for rep in range(6):
        for row in [*(row_synthetic(n, rep, f) for n in host.COUNTS), *(row_hetero(e, b, rep, f) for e, b in host.HETERO)]:
            row['same_work_semantics_sha256'] = host.digest(host.semantic(row))
            rows.append(row)
    contract = {'fixture_only': True}
    return {'schema': 'definition-diversity-diagnostic-v1', 'status': 'all_declared_rows_verified', 'expected_rows': 48, 'build_id': 'f' * 64, 'contract': contract, 'contract_sha256': f['contract_sha256'], 'warmups': 1, 'measured_repetitions': 5, 'rows': rows, 'outer_guard_declaration_is_not_OS_proof': True, 'authentic256_Oracle_ability_claim': False, 'legal_pool_expansion': False, 'product_latency_or_quality_claim': False}, contract, f


class Guards(unittest.TestCase):
    def validate(self, report):
        _, contract, f = fixture()
        return host.validate_report(report, 'f' * 64, contract, f)

    def reject(self, edit):
        report, _, _ = fixture()
        edit(report)
        with self.assertRaises((ValueError, KeyError, TypeError)):
            self.validate(report)

    def test_full48_positive_and_independent_closed_form(self):
        report, _, _ = fixture()
        self.assertEqual(self.validate(report)['measured_rows'], 40)
        row = report['rows'][1]
        self.assertEqual([int(x['power']) for x in row['expected_characteristics']], [-1, 0, 1, 2])
        self.assertEqual([int(x['toughness']) for x in row['expected_characteristics']], [7, 10, 13, 16])

    def test_omitted_row(self): self.reject(lambda r: r['rows'].pop())
    def test_duplicate_row(self): self.reject(lambda r: r['rows'].__setitem__(2, copy.deepcopy(r['rows'][1])))
    def test_reordered_rows(self): self.reject(lambda r: r['rows'].__setitem__(slice(0, 2), list(reversed(r['rows'][:2]))))
    def test_bad_denominator_bool(self): self.reject(lambda r: r.__setitem__('warmups', True))
    def test_duplicate_payload_even_rehashed(self): self.reject(lambda r: r['rows'][1]['complete_payload_manifest'].__setitem__(7, copy.deepcopy(r['rows'][1]['complete_payload_manifest'][0])))
    def test_all_manifest_fields_not_only_ids(self): self.reject(lambda r: r['rows'][1]['complete_payload_manifest'][0]['instruction'].__setitem__('source_block', True))
    def test_changed_characteristics(self): self.reject(lambda r: r['rows'][2]['expected_characteristics'][0].__setitem__('power', '999'))
    def test_stale_full_state_across_repetition(self): self.reject(lambda r: (r['rows'][8].__setitem__('full_State_sha256', '9' * 64), r['rows'][8].__setitem__('changed_full_State_sha256', '9' * 64), r['rows'][8].__setitem__('same_work_semantics_sha256', host.digest(host.semantic(r['rows'][8])))))
    def test_authentic256_false_claim(self): self.reject(lambda r: r.__setitem__('authentic256_Oracle_ability_claim', True))
    def test_overlong_segment(self): self.reject(lambda r: r['rows'][0]['samples'][0].__setitem__('nanos', 120_000_000_001))
    def test_omitted_call_segment(self): self.reject(lambda r: r['rows'][6]['work']['samples'].pop())
    def test_reordered_call_segments(self): self.reject(lambda r: r['rows'][6]['work']['samples'].__setitem__(slice(0, 2), list(reversed(r['rows'][6]['work']['samples'][:2]))))
    def test_trigger_count_not_reference_normalized(self): self.reject(lambda r: r['rows'][7].__setitem__('actual_trigger_matches', 4))
    def test_wrong_source_program(self): self.reject(lambda r: r['rows'][6]['complete_source_manifest'][0].__setitem__('complete_triggered', [{'different': True}]))
    def test_wrong_damage_source_power(self): self.reject(lambda r: r['rows'][6]['damage_replacement_probe'].__setitem__('amounts_after', [4, 0]))
    def test_unknown_row_field(self): self.reject(lambda r: r['rows'][0].__setitem__('discarded_history', True))
    def test_duplicate_json_and_source_preimport(self):
        with self.assertRaises(ValueError): host.decode('{"a":1,"a":2}')
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'bad.py'
            p.write_text('raise RuntimeError("should not execute")')
            with self.assertRaisesRegex(ValueError, 'hash BEFORE'):
                host.authenticated('never_imported', p, '0' * 64)

    def test_exact_stdout_and_reject_torn_unknown_duplicate_start(self):
        raw = '\nrunning 1 test\ntest ' + host.ENTRY + ' ... DEFINITION_DIVERSITY_ROW_START={"fixture":true}\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.0s\n'
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'stdout'
            p.write_text(raw)
            self.assertEqual(len(host.stdout_records(p, host.ENTRY, ['DEFINITION_DIVERSITY_ROW_START'])), 1)
            for bad in [raw[:-1], raw + 'unexpected\n', raw.replace('ok\n\n', 'ok\nok\n\n'), raw.replace('running 1 test', 'running 1 test\nrunning 1 test')]:
                p.write_text(bad)
                with self.assertRaises(ValueError): host.stdout_records(p, host.ENTRY, ['DEFINITION_DIVERSITY_ROW_START'])

    def test_compiled_diagnostic_source_pin_cannot_be_silently_rebound(self):
        rel = 'run-definition-diversity-v1.py'
        source = HERE / rel
        a = {'stage_root': str(HERE), 'compiler_artifact': {'path': '/fake/artifact'}, 'native_manifest': {'path': '/fake/native'}, 'timeout_seconds': 120, 'cpu_ids': [0], 'host_scripts': {str(p): host.sha(p) for p in [source, HERE / 'run-fixed-work.py', HERE / 'measurement_host.py']}, 'diagnostic_sources': {rel: '0' * 64}}
        h = SimpleNamespace(read_json=lambda p: {'features': []} if str(p) == '/fake/artifact' else {'files': [{'path': rel, 'sha256': host.sha(source)}]})
        fixed = SimpleNamespace(load_host=lambda _: h, authority_check=lambda *_: a, require_target=lambda *_: None)
        with patch.object(host, 'authenticated', return_value=fixed), patch.object(host, 'STAGE', HERE):
            with self.assertRaisesRegex(ValueError, 'compiled manifest equality'):
                host.authority_guard(Path('/fake/a'), 'x', host.FIXED_SHA, host.HELPER_SHA, {}, source, [rel])

    def test_trigger_subject_float_or_bool_rehashed_all_repeats(self):
        for alias in [2.0, True]:
            with self.subTest(alias=alias):
                def edit(report):
                    for row in report['rows']:
                        if row['row'] == 'source_backed_heterogeneous' and row['enabled']:
                            row['matched_source_subject_pairs'][0][1] = alias
                            row['same_work_semantics_sha256'] = host.digest(host.semantic(row))
                self.reject(edit)

    def test_nested_float_and_bool_ID_aliases(self):
        for alias in [0.0, False]:
            with self.subTest(alias=alias):
                def edit(report):
                    for row in report['rows']:
                        if row['row'] == 'synthetic_unique_modifier_payloads' and row['effect_instances']:
                            row['complete_payload_manifest'][0]['instruction']['source_block'] = alias
                            row['same_work_semantics_sha256'] = host.digest(host.semantic(row))
                self.reject(edit)

    def test_unchanged_deadline_and_features_enforced_before_native(self):
        artifact = {'features': []}
        a = {'stage_root': str(host.STAGE), 'compiler_artifact': {'path': '/fake/artifact'}, 'timeout_seconds': 119, 'cpu_ids': [0]}
        h = SimpleNamespace(read_json=lambda _: artifact)
        fixed = SimpleNamespace(load_host=lambda _: h, authority_check=lambda *_: a, require_target=lambda *_: None)
        with patch.object(host, 'authenticated', return_value=fixed):
            with self.assertRaisesRegex(ValueError, 'fixed120s'): host.authority_guard(Path('/fake/a'), 'x', host.FIXED_SHA, host.HELPER_SHA, {}, HERE / 'run-definition-diversity-v1.py', [])
            a['timeout_seconds'] = 120
            artifact['features'] = ['diagnostic-allocations']
            with self.assertRaisesRegex(ValueError, 'features'): host.authority_guard(Path('/fake/a'), 'x', host.FIXED_SHA, host.HELPER_SHA, {}, HERE / 'run-definition-diversity-v1.py', [])


if __name__ == '__main__':
    unittest.main()
