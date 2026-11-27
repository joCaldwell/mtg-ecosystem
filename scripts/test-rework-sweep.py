#!/usr/bin/env python3
"""Focused self-tests for fixed inputs, phase changes, admission and source metrics."""
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from rework_sweep_diagnostics import rejection_phase
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

    def test_whole_archive_phases_and_other_with_physical_quantities(self):
        header = {'corpus': {'sha256': 'source'}, 'paperEvidence': {'sha256': 'evidence'},
                  'rules': {'edition': 'test'}, 'scope': 'test'}
        cases = [
            ('yes', 'legal', 'parsed', sweep.SUPPORTED, ''),
            ('read', 'legal', 'unsupported', 'unparsed', 'status:unparsed'),
            ('resolve', 'legal', 'parsed', 'unsupported', 'ref:step-result:spell'),
            ('other', 'legal', 'parsed', 'unsupported', 'compose.step.set-pt'),
            ('check', 'legal', 'parsed', 'unsupported', 'check:typed-diagnostic'),
            ('admission', 'legal', 'parsed', 'unsupported', 'admission:tagged'),
            ('nonlegal', 'not_legal', 'parsed', sweep.SUPPORTED, ''),
        ]
        archive = self.out / 'archive.jsonl'
        archive_rows = [{'oracleIdentity': oid, 'status': status, 'resolution': {'complete': True},
                         'source': {'legalities': {'commander': legality}}}
                        for oid, legality, status, _, _ in cases]
        archive.write_text(''.join(json.dumps(row) + '\n' for row in [header, *archive_rows]))
        (self.out / 'summary.json').write_text(json.dumps({
            'archive_sha256': sweep.sha256(archive), 'total': len(cases), 'commander_legal': 6,
            'lowered_validated': 1, 'statuses': {'unparsed': 1, 'unsupported': 4, sweep.SUPPORTED: 1, 'invalid': 0},
        }))
        (self.out / 'records.tsv').write_text('oracle_id\tname\tstatus\treason\n' + ''.join(
            f'{oid}\tName\t{status}\t{reason}\n' for oid, _, _, status, reason in cases))
        (self.out / 'blockers.tsv').write_text('oracle_id\tcommander_legal\tkind\tblock\tcode\n')
        measured, _ = sweep.measure_coverage(archive, self.out, Counter(other=3, missing=2))
        self.assertEqual(measured.report['statuses'][sweep.SUPPORTED], 2)
        self.assertEqual(measured.report['rejections']['read']['rankable_slots'], 2)
        self.assertEqual(measured.report['rejections']['resolve']['legal_cards'], 1)
        self.assertEqual(measured.report['rejections']['check']['legal_cards'], 1)
        self.assertEqual(measured.report['rejections']['admission']['legal_cards'], 1)
        other = measured.report['other']
        self.assertEqual(other['subclasses']['Continuous effects created at resolution'],
                         {'legal_cards': 1, 'rankable_slots': 3})
        self.assertEqual(other['legal_cards'], 2)
        archive.write_text(archive.read_text() + '\n')
        with self.assertRaisesRegex(ValueError, 'SHA-256 mismatch'):
            sweep.measure_coverage(archive, self.out, Counter())

    def test_legacy_phase_uses_primary_and_resolution_evidence(self):
        self.assertEqual(rejection_phase('unsupported', 'span:missing', {'span:missing'}), 'resolve')
        self.assertEqual(rejection_phase('unsupported', 'composition:kernel_cell_is_not_audited_test', set()), 'check')
        self.assertEqual(rejection_phase('unparsed', 'archive_excluded:paper', set()), 'read')
        self.assertEqual(rejection_phase('unsupported', 'operand:player:test', set()), 'elaborate')

    def test_held_out_snapshots_only_contribute_aggregate_slots(self):
        a = deck(1, 'Pool A deck', 'supported')
        active = deck(1, 'Training deck', 'blocked')
        held = deck(2, 'Private held-out deck', 'heldout-identity', 9)
        pools = {sweep.POOL_A: [a], sweep.TRAIN: [active], sweep.HELD: [held],
                 sweep.CONTROL: [active, deck(3, 'Control deck', 'blocked')],
                 sweep.HELD_CONTROL: [held]}
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

    def test_rust_guardrails_detect_keyword_and_legacy_forwarding(self):
        fixtures = {
            'contracts/src/lib.rs': '''
                enum KeywordAbility { Flying, #[serde(rename = "first strike")] FirstStrike }
                impl Instruction { fn target_destroy(&self) -> Option<u32> { None } }
            ''',
            'contracts/src/kir/verbs.rs': '''
                enum BuiltinId { Proliferate }
                enum DelayedEvent { NextEndStepAfterDash, NextTurn }
                enum Verb { Draw, CreateMyriadToken { fields: BTreeMap<u32, u32> }, ExileMyriadTokens {} }
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
                    .execute_removal_instruction(frame, instruction); }
            ''',
            'core/src/engine/removal.rs': '''
                fn execute_removal_instruction(instruction: Instruction) { instruction.target_destroy(); }
            ''',
            'core/src/engine/event.rs': 'enum ProposedEvent { CreateEncoreTokens }',
            'view/src/model.rs': 'enum Reason<S: Space> { MyriadDefender, Priority }',
        }
        for name, contents in fixtures.items():
            path = self.out / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(contents)
        result = guardrails(self.out)
        self.assertEqual(result['kernel_cells']['names'], ['Draw'])
        self.assertEqual(result['mirrored_shape_guards']['count'], 1)
        self.assertEqual(result['routing_ladder_pairs']['count'], 1)
        groups = result['mechanic_named_variants']
        self.assertEqual(groups['Verb']['names'], ['CreateMyriadToken', 'ExileMyriadTokens'])
        self.assertEqual(groups['interpreter stages']['count'], 1)
        self.assertEqual(groups['EventOutcome']['count'], 0)


if __name__ == '__main__':
    unittest.main()
