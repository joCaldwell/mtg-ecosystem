#!/usr/bin/env python3
"""Focused self-tests for fixed inputs, phase changes, admission and source metrics."""
from collections import Counter
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import importlib.util
from io import StringIO
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from rework_sweep_diagnostics import other_class, rejection_phase
from rework_sweep_guardrails import guardrails

spec = importlib.util.spec_from_file_location('rework_sweep', Path(__file__).with_name('rework-sweep.py'))
sweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sweep)


def deck(deck_id, name, oid, quantity=100):
    return {'id': deck_id, 'name': name, 'cards': [{'oracle_id': oid, 'quantity': quantity}]}


class SweepTests(unittest.TestCase):
    def setUp(self):
        sandbox = tempfile.TemporaryDirectory()
        self.addCleanup(sandbox.cleanup)
        self.out = Path(sandbox.name)

    def write_coverage(self, cases, receipts=None):
        header = {'corpus': {'sha256': 'source'}, 'paperEvidence': {'sha256': 'evidence'},
                  'rules': {'edition': 'test'}, 'scope': 'test'}
        archive = self.out / 'archive.jsonl'
        receipts = receipts or self.out / 'coverage'
        receipts.mkdir()
        archive_rows = [{'oracleIdentity': oid, 'status': flags.get('status', 'parsed'),
                         'source': {'legalities': {'commander': legality}},
                         'resolution': flags.get('resolution', {'complete': True})}
                        for oid, legality, _, _, flags in cases]
        archive.write_text(''.join(json.dumps(row) + '\n' for row in [header, *archive_rows]))
        legal = Counter(status for _, legality, status, _, _ in cases if legality == 'legal')
        sweep.write_json(receipts / 'summary.json', {
            'archive_sha256': sweep.sha256(archive), 'total': len(cases), 'commander_legal': legal.total(),
            'lowered_validated': legal[sweep.SUPPORTED], 'statuses': dict(legal),
        })
        (receipts / 'records.tsv').write_text('oracle_id\tname\tstatus\treason\n' + ''.join(
            f'{oid}\tName\t{status}\t{reason}\n' for oid, _, status, reason, _ in cases))
        (receipts / 'blockers.tsv').write_text('oracle_id\tcommander_legal\tkind\tblock\tcode\n')
        return archive, receipts

    def sweep_args(self, archive, receipts, output):
        args = ['--archive', str(archive), '--out', str(output),
                '--baseline-coverage', str(receipts), '--baseline-source', str(self.out)]
        for name in ('pool-a', 'pool-b', 'pool-b-control', 'cohorts'):
            path = self.out / name
            path.write_text('{}')
            args.extend([f'--{name}', str(path)])
        self.write_guardrail_sources(self.out / 'packages/game-engine/crates')
        return args

    def test_whole_archive_phases_and_other_with_physical_quantities(self):
        cases = [
            ('yes', 'legal', sweep.SUPPORTED, '', {}),
            ('read', 'legal', 'unsupported', 'keyword:fuse',
             {'status': 'unsupported', 'resolution': {'complete': False, 'residuals': ['diagnostic:unparsed-face']}}),
            ('resolve', 'legal', 'unsupported', 'source.layout:layout_class',
             {'resolution': {'complete': False, 'residuals': ['spans:block has no recorded source tokens']}}),
            ('other', 'legal', 'unsupported', 'compose.step.set-pt', {}),
            ('check', 'legal', 'unsupported', 'check:typed-diagnostic', {}),
            ('admission', 'legal', 'unsupported', 'admission:tagged', {}),
            ('nonlegal', 'not_legal', sweep.SUPPORTED, '', {}),
        ]
        archive, receipts = self.write_coverage(cases)
        measured, _ = sweep.measure_coverage(archive, receipts, Counter(read=4, other=3, missing=2))
        self.assertEqual(measured.report['statuses'][sweep.SUPPORTED], 2)
        self.assertEqual(measured.report['rejections']['read'],
                         {'archive_cards': 1, 'legal_cards': 1, 'rankable_slots': 6})
        self.assertEqual(measured.report['rejections']['resolve']['legal_cards'], 1)
        self.assertEqual(measured.report['rejections']['check']['legal_cards'], 1)
        self.assertEqual(measured.report['rejections']['admission']['legal_cards'], 1)
        other = measured.report['other']
        self.assertEqual(other['subclasses']['Continuous effects created at resolution'],
                         {'legal_cards': 1, 'rankable_slots': 3})
        self.assertEqual(other['legal_cards'], 2)
        archive.write_text(archive.read_text() + '\n')
        with self.assertRaisesRegex(ValueError, 'SHA-256 mismatch'):
            sweep.measure_coverage(archive, receipts, Counter())

    def test_legacy_phase_uses_archive_residuals_before_rust_diagnostics(self):
        cases = [
            ('keyword:fuse', {'diagnostic:unparsed-face', 'face:unparsed', 'status:unparsed'}, 'read'),
            ('source.face:loyalty_defense_or_inconsistent_color_indicator', {'status:unparsed'}, 'read'),
            ('keyword:fuse', {'archive_excluded:no-eligible-paper-printing'}, 'read'),
            ('source.layout:layout_class', {'spans:block has no recorded source tokens'}, 'resolve'),
            ('cell:unsupported', {'resolution:incomplete-without-enumerated-residual'}, 'resolve'),
            ('composition:kernel_cell_is_not_audited_test', set(), 'check'),
            ('ref:attached-to:triggered', set(), 'elaborate'),
            ('compose.effects:a_printed_number_is_absent_from_the_ast', set(), 'elaborate'),
            ('resolve:unbound', set(), 'resolve'),
        ]
        for reason, residuals, phase in cases:
            with self.subTest(reason=reason, residuals=residuals):
                self.assertEqual(rejection_phase(reason, residuals), phase)

    def test_other_classifier_groups_unanimous_codes_and_preserves_remainder(self):
        cases = [
            (['compose.step.set-pt', 'compose.step.colors'], 'Continuous effects created at resolution'),
            (['trigger.what:event_object_card', 'operand:amount'], 'References and operands'),
            (['compose.step.unimplemented'], 'Verbs with no IR'),
            (['compose.step.create-role', 'operand:token'], 'Token specifications'),
            (['compose.step.set-pt', 'operand:amount'], 'Remainder'),
            (['operand:duration', 'compose.step.optional'], 'Remainder'),
            (['diagnostic-without-a-known-family'], 'Remainder'),
        ]
        for codes, expected in cases:
            with self.subTest(codes=codes):
                self.assertEqual(other_class(codes), expected)

    def test_held_out_snapshots_only_contribute_aggregate_slots(self):
        a = deck(1, 'Pool A deck', 'supported')
        active = deck(1, 'Training deck', 'blocked')
        held = deck(2, 'Private held-out deck', 'heldout-identity', 9)
        held_blocked = deck(4, 'Second private deck', 'blocked', 13)
        pools = {sweep.POOL_A: [a], sweep.TRAIN: [active], sweep.HELD: [held, held_blocked],
                 sweep.CONTROL: [active, deck(3, 'Control deck', 'blocked')],
                 sweep.HELD_CONTROL: [held, held_blocked]}
        self.assertEqual(sweep.pool_report([held], {'heldout-identity'}), {'compiled_slots': 9, 'total_slots': 9})
        inspected = sweep.rankable_decks(pools)
        self.assertEqual([d['name'] for _, d in inspected], [a['name'], active['name'], 'Control deck'])

        def inspect(cmd, _):
            path = Path(cmd[-1])
            self.assertNotIn(held['name'], path.read_text())
            self.assertNotIn('heldout-identity', path.read_text())
            return {'bulk_manifest_sha256': 'pin', 'catalog_digest': 'digest', 'decks': []}

        with patch.object(sweep, 'invoke', side_effect=inspect) as invoke:
            sweep.run_admission('sim', 'bulk', {'manifest_sha256': 'pin', 'catalog_digest': 'digest'},
                                inspected, self.out)
        invoke.assert_called_once()

        archive, receipts = self.write_coverage([
            ('supported', 'legal', sweep.SUPPORTED, '', {}),
            ('heldout-identity', 'legal', sweep.SUPPORTED, '', {}),
            ('blocked', 'legal', 'unsupported', 'compose.step.set-pt', {}),
        ])
        output = self.out / 'report'
        args = self.sweep_args(archive, receipts, output)
        with (patch.object(sweep, 'load_pools', return_value=pools),
              patch.object(sweep, 'load_cohorts', return_value=({'ambiguity-311': set()}, {})),
              redirect_stdout(StringIO())):
            self.assertEqual(sweep.main(args), 0)
        json_report = (output / 'report.json').read_text()
        md_report = (output / 'report.md').read_text()
        results = json.loads(json_report)['runs']['baseline']['pools']
        for pool in (sweep.HELD, sweep.HELD_CONTROL):
            self.assertEqual(results[pool], {'compiled_slots': 9, 'total_slots': 22})
        self.assertEqual(results[sweep.TRAIN]['deck_scores'][0]['name'], active['name'])
        for report in (json_report, md_report):
            for deck_name in (held['name'], held_blocked['name']):
                self.assertNotIn(deck_name, report)
        self.assertNotIn('heldout-identity', json_report)
        self.assertIn('| 9 / 22 |', md_report)
        self.assertNotIn('| 9 / 9 |', md_report)
        self.assertNotIn('| 0 / 13 |', md_report)

    def test_invalid_candidate_writes_both_comparison_reports_before_failure(self):
        case = ('card', 'legal', sweep.SUPPORTED, '', {})
        archive, receipts = self.write_coverage([case])
        _, candidate = self.write_coverage(
            [('card', 'legal', 'invalid', 'ir:failed-validation', {})], self.out / 'candidate-coverage')
        output = self.out / 'report'
        args = self.sweep_args(archive, receipts, output) + [
            '--candidate-coverage', str(candidate), '--candidate-source', str(self.out)]
        pools = {sweep.POOL_A: [deck(1, 'Pool A deck', 'card')],
                 sweep.TRAIN: [deck(2, 'Training deck', 'card')],
                 sweep.HELD: []}
        stderr = StringIO()
        with (patch.object(sweep, 'load_pools', return_value=pools),
              patch.object(sweep, 'load_cohorts', return_value=({'ambiguity-311': set()}, {})),
              redirect_stdout(StringIO()), redirect_stderr(stderr)):
            self.assertEqual(sweep.main(args), 2)
        self.assertIn('invalid IR records in candidate (1)', stderr.getvalue())
        report = sweep.read_json(output / 'report.json')
        self.assertEqual(report['runs']['baseline']['statuses']['invalid'], 0)
        self.assertEqual(report['runs']['candidate']['statuses']['invalid'], 1)
        self.assertEqual(report['comparison']['legal_losses'], 1)
        self.assertEqual(report['comparison']['legal_phase_transitions'], [
            {'before': sweep.SUPPORTED, 'after': 'invalid', 'cards': 1},
        ])
        markdown = (output / 'report.md').read_text()
        self.assertIn('| lowered_validated | invalid | 1 |', markdown)
        self.assertIn('| invalid | 1 |', markdown)

    def test_bulk_rejects_mismatched_archive_payload_and_coverage(self):
        catalog = self.out / 'catalog.json'
        catalog.write_text('{}')
        index = self.out / 'index.json'
        sweep.write_json(index, {'rows': [
            {'oracle_id': 'card', 'status': sweep.SUPPORTED, 'reason_code': None,
             'facts': {'commander_legality': 'legal'}},
        ]})
        manifest = self.out / 'manifest.json'
        sweep.write_json(manifest, {
            'archive_sha256': 'archive-pin', 'catalog_digest': 'digest', 'producer_build_id': 'producer',
            **{path.stem: {'sha256': sweep.sha256(path), 'bytes': path.stat().st_size}
               for path in (catalog, index)},
        })
        report = {'archive_sha256': 'archive-pin'}
        records = {'card': (sweep.SUPPORTED, '')}
        self.assertEqual(sweep.verify_bulk(self.out, report, records, {'card'})['catalog_digest'], 'digest')
        with self.assertRaisesRegex(ValueError, 'bulk archive SHA-256 mismatch'):
            sweep.verify_bulk(self.out, {'archive_sha256': 'wrong'}, records, {'card'})
        with self.assertRaisesRegex(ValueError, 'bulk index/coverage identity mismatch'):
            sweep.verify_bulk(self.out, report, {'wrong': records['card']}, {'card'})
        with self.assertRaisesRegex(ValueError, 'bulk index disagrees with coverage or Commander legality'):
            sweep.verify_bulk(self.out, report, records, set())
        catalog.write_text('[]')
        with self.assertRaisesRegex(ValueError, 'bulk catalog payload mismatch'):
            sweep.verify_bulk(self.out, report, records, {'card'})

    def test_admission_rejects_mismatched_artifact_and_slot_counts(self):
        path = self.out / 'admission.json'
        artifact = {'manifest_sha256': 'pin', 'catalog_digest': 'digest'}
        decks = [(sweep.POOL_A, deck(7, 'Rankable deck', 'card'))]
        row = {'id': 1, 'name': 'Rankable deck', 'compiled_slots': 100, 'total_slots': 100,
               'status': 'admitted', 'deck_issues': [], 'card_gaps': []}
        sweep.write_json(path, {**artifact, 'decks': [row]})
        self.assertEqual(sweep.admission_report(path, artifact, decks, {'card'}), {(sweep.POOL_A, 7): row})
        with self.assertRaisesRegex(ValueError, 'different artifact identity'):
            sweep.admission_report(path, {**artifact, 'manifest_sha256': 'wrong'}, decks, {'card'})
        with self.assertRaisesRegex(ValueError, 'admission disagrees with coverage'):
            sweep.admission_report(path, artifact, decks, set())

    def test_gains_and_losses_survive_an_equal_total_and_corpus_changes_fail(self):
        report = {'pools': {}, 'guardrails': {
            **{key: {'count': 1} for key in ('kernel_cells', 'mirrored_shape_guards', 'routing_ladder_pairs')},
            'mechanic_named_variants': {'Verb': {'count': 1, 'names': ['Dash']}},
        }}
        old = sweep.Measurement(report, {'a', 'b'}, {'a'}, ('corpus',), {'a': sweep.SUPPORTED, 'b': 'resolve'})
        new = sweep.Measurement(report, {'a', 'b'}, {'b'}, ('corpus',), {'a': 'check', 'b': sweep.SUPPORTED})
        compared = sweep.compare(old, new)
        self.assertEqual((compared['legal_gains'], compared['legal_losses']), (1, 1))
        self.assertEqual(compared['legal_phase_transitions'], [
            {'before': sweep.SUPPORTED, 'after': 'check', 'cards': 1},
            {'before': 'resolve', 'after': sweep.SUPPORTED, 'cards': 1},
        ])
        with self.assertRaisesRegex(ValueError, 'identical corpus'):
            sweep.compare(old, new._replace(corpus=('different',)))

    def test_seeded_qualification_requires_distinct_seeds_and_native_admission(self):
        path = self.out / 'games.json'
        game = {'deck_id': 7, 'seed': 123, 'status': 'complete'}
        pin = hashlib.sha256(b'configuration').hexdigest()
        receipt = {'manifest_sha256': 'manifest', 'configuration_sha256': pin,
                   'games': [game, {**game, 'seed': 456, 'status': 'censored'}]}
        path.write_text(json.dumps(receipt))
        stats = sweep.qualification_report(path, receipt, {7}, {7})
        self.assertEqual(stats['decks_with_complete_seeded_game'], 1)
        self.assertEqual(stats['deck_game_outcomes'], {'complete': 1, 'censored': 1, 'failed': 0})
        with self.assertRaisesRegex(ValueError, 'invalid seeded qualification row'):
            sweep.qualification_report(path, receipt, {7}, set())
        receipt['games'].append(game)
        path.write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, 'invalid seeded qualification row'):
            sweep.qualification_report(path, receipt, {7}, {7})

    def write_guardrail_sources(self, crates):
        fixtures = {
            'contracts/src/lib.rs': '''
                enum KeywordAbility { Flying, #[serde(rename = "first strike")] FirstStrike, Deathtouch }
                impl Instruction {
                    fn target_destroy(&self) -> Option<u32> { None }
                    fn set_damage(&self) -> Option<u32> { None }
                }
            ''',
            'contracts/src/kir/verbs.rs': '''
                enum BuiltinId { Proliferate }
                enum DelayedEvent { NextEndStepAfterDash, NextTurn }
                enum Verb { Draw, Fight, CreateMyriadToken { fields: BTreeMap<u32, u32> }, ExileMyriadTokens {} }
            ''',
            'contracts/src/kir/tiers.rs': 'enum Cell { Draw, Dash }',
            'contracts/src/kir/check/mod.rs': '''
                fn cell() { /* /* nested */ Cell::Dash */ let _ = Cell::Draw; }
            ''',
            'compiler/src/compose.rs': '''
                // "shape outside the lowering mirror"
                fn gap() { let _ = "shape outside the lowering mirror"; }
            ''',
            'core/src/engine/kir/mod.rs': '''
                enum Stage { Leaf, MyriadDefender }
                fn execute_kernel() { self
                    .execute_removal_instruction(frame, instruction); self.execute_set_damage(frame); }
            ''',
            'core/src/engine/removal.rs': '''
                fn execute_removal_instruction(instruction: Instruction) { instruction.target_destroy(); }
            ''',
            'core/src/engine/wipes.rs': '''
                fn execute_set_damage(frame: Resolving) { self.wipe_damage_spec(frame); }
                fn wipe_damage_spec(frame: Resolving) { self.frame_instruction(frame).set_damage(); }
            ''',
            'core/src/engine/event.rs': '''
                enum ProposedEvent { CreateEncoreTokens }
                enum EventOutcome { DeathtouchWindowConsumed }
            ''',
            'view/src/model.rs': 'enum Reason<S: Space> { MyriadDefender, Priority }',
        }
        for name, contents in fixtures.items():
            path = crates / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(contents)

    def test_rust_guardrails_detect_keyword_and_legacy_forwarding(self):
        self.write_guardrail_sources(self.out)
        result = guardrails(self.out)
        self.assertEqual(result['kernel_cells']['names'], ['Draw'])
        self.assertEqual(result['mirrored_shape_guards']['count'], 1)
        self.assertEqual(result['routing_ladder_pairs']['count'], 2)
        self.assertEqual(result['routing_ladder_pairs']['pairs'][1]['helpers'], ['set_damage'])
        groups = result['mechanic_named_variants']
        self.assertEqual(groups['Verb']['names'], ['CreateMyriadToken', 'ExileMyriadTokens'])
        self.assertEqual(groups['interpreter stages']['count'], 1)
        self.assertIn('Fight', groups['Verb']['all_variants'])
        self.assertEqual(groups['EventOutcome']['names'], ['DeathtouchWindowConsumed'])

    def test_guardrails_rejects_missing_kernel_source(self):
        self.write_guardrail_sources(self.out)
        (self.out / 'core/src/engine/kir/mod.rs').unlink()
        with self.assertRaisesRegex(ValueError, 'routing ladder source is missing: core/src/engine/kir/mod.rs'):
            guardrails(self.out)

    def test_guardrails_rejects_missing_dispatcher(self):
        self.write_guardrail_sources(self.out)
        (self.out / 'core/src/engine/kir/mod.rs').write_text('fn another_dispatcher() {}')
        with self.assertRaisesRegex(ValueError, 'routing ladder dispatcher execute_kernel is missing'):
            guardrails(self.out)

    def test_guardrails_rejects_missing_instruction_impl(self):
        self.write_guardrail_sources(self.out)
        path = self.out / 'contracts/src/lib.rs'
        path.write_text(path.read_text().replace('impl Instruction', 'impl AnotherType'))
        with self.assertRaisesRegex(ValueError, 'routing ladder recognizer declaration impl Instruction is missing'):
            guardrails(self.out)

    def test_guardrails_counts_zero_when_dispatcher_has_no_legacy_routes(self):
        self.write_guardrail_sources(self.out)
        (self.out / 'core/src/engine/kir/mod.rs').write_text('fn execute_kernel() {}')
        self.assertEqual(guardrails(self.out)['routing_ladder_pairs'], {'count': 0, 'pairs': []})


if __name__ == '__main__':
    unittest.main()
