"""Exercise engine-decks.py coverage joins, baselines and paired benches with a fake mtg-sim."""
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('decks', Path(__file__).with_name('engine-decks.py'))
decks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(decks)

# A stand-in mtg-sim: coverage writes receipts from FAKE_STATUSES; bench-pods
# prints a report whose schedule FAKE_SHIFT perturbs and FAKE_REJECT rejects.
FAKE = r'''#!PYTHON
import json, os, sys
from pathlib import Path
args = sys.argv[1:]
if args[:2] == ['catalog', 'coverage']:
    out = Path(args[args.index('--out') + 1]); out.mkdir()
    statuses = json.loads(os.environ['FAKE_STATUSES'])
    lines = ['oracle_id\tname\tstatus\treason'] + [f'{k}\tn\t{s}\t{r}' for k, (s, r) in sorted(statuses.items())]
    (out / 'records.tsv').write_text('\n'.join(lines) + '\n')
    (out / 'summary.json').write_text(json.dumps({'archive_sha256': os.environ.get('FAKE_ARCHIVE', 'a' * 64),
        'total': len(statuses), 'lowered_validated': 0, 'lowered_validated_percent': 0.0, 'statuses': {}}))
    sys.exit(0)
assert args[0] == 'bench-pods' and '--rotate-seats' in args
decks = args[args.index('--pod-decks') + 1]
if os.environ.get('FAKE_REJECT') and decks.endswith(os.environ['FAKE_REJECT']):
    print(json.dumps({'reason': 'deck_gaps', 'decks': []})); sys.exit(1)
games, start = int(args[args.index('--games') + 1]), int(args[args.index('--seed-start') + 1])
shift = int(os.environ.get('FAKE_SHIFT', '0')) if decks.endswith('b.json') else 0
rows = [{'case': {'index': i, 'seed_index': start + i // 4, 'rotation': i % 4}, 'starting_seat': (start + i // 4 + shift) % 4,
         'outcome': 'win', 'winner': i % 4, 'placements': [1, 2, 3, 4], 'turns': 9, 'decisions': 80,
         'admission_sha256': decks[-6:]} for i in range(games)]
print(json.dumps({'build': {'id': 'b' * 64}, 'catalog_sha256': 'c' * 64, 'games': games, 'seed_start': start,
    'rotate_seats': True, 'limits': {'decisions': 100}, 'policy': 'develop', 'rows': rows, 'proofs_verified': True}))
'''


def pod(*commanders):
    return {'decks': [[{'oracle_id': c, 'quantity': 1, 'role': 'commander'},
                       {'oracle_id': 'forest', 'quantity': 99, 'role': 'main'}] for c in commanders]}


class DeckHarnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.dir = Path(self.temp.name)
        self.sim = self.dir / 'mtg-sim'
        self.sim.write_text(FAKE.replace('PYTHON', sys.executable))
        self.sim.chmod(self.sim.stat().st_mode | stat.S_IXUSR)
        self.env = dict(os.environ)

    def tearDown(self):
        os.environ.clear()
        os.environ.update(self.env)
        self.temp.cleanup()

    def write(self, name, value):
        path = self.dir / name
        path.write_text(json.dumps(value))
        return path

    def test_coverage_reports_blocked_slots_and_baseline_deltas(self):
        snapshot = self.write('decks.json', {'deckCount': 1, 'decks': [{'id': 3, 'name': 'Test', 'cards': [
            {'oracle_id': 'leader', 'name': 'Leader', 'quantity': 1, 'is_commander': True},
            {'oracle_id': 'forest', 'name': 'Forest', 'quantity': 98, 'is_commander': False},
            {'oracle_id': 'spell', 'name': 'Spell', 'quantity': 1, 'is_commander': False}]}]})
        archive = self.write('archive.jsonl', {})
        os.environ['FAKE_STATUSES'] = json.dumps({'leader': ['lowered_validated', ''], 'forest': ['lowered_validated', ''],
                                                  'spell': ['unsupported', 'effect: scry']})
        first = self.dir / 'first'
        self.assertEqual(0, decks.main(['coverage', str(self.sim), '--out', str(first), '--decks', str(snapshot),
                                        '--archive', str(archive)]))
        report = json.loads((first / 'report.json').read_text())
        self.assertEqual((99, 100), (report['decks'][0]['compiled_slots'], report['decks'][0]['total_slots']))
        self.assertEqual([{'reason': 'effect: scry', 'cards': 1}], report['top_blockers'])
        os.environ['FAKE_STATUSES'] = json.dumps({'leader': ['lowered_validated', ''], 'forest': ['lowered_validated', ''],
                                                  'spell': ['lowered_validated', '']})
        second = self.dir / 'second'
        self.assertEqual(0, decks.main(['coverage', str(self.sim), '--out', str(second), '--decks', str(snapshot),
                                        '--archive', str(archive), '--baseline', str(first / 'report.json')]))
        delta = json.loads((second / 'report.json').read_text())['baseline']
        self.assertEqual([{'id': 3, 'name': 'Test', 'compiled_slots_delta': 1, 'newly_compiled': ['spell'],
                           'newly_blocked': []}], delta['decks'])
        self.assertEqual({'effect: scry': -1}, delta['blocker_card_deltas'])
        # Outputs are create-new: an existing directory is never reused.
        self.assertEqual(2, decks.main(['coverage', str(self.sim), '--out', str(second), '--decks', str(snapshot),
                                        '--archive', str(archive)]))

    def bench(self, out, *extra):
        return decks.main(['bench', str(self.sim), '--bulk-artifact', str(self.dir), '--manifest-sha', 'f' * 64,
                           '--a', str(self.dir / 'a.json'), '--games', '8', '--out', str(self.dir / out), *extra])

    def test_paired_bench_shares_one_schedule_and_keeps_raw_rows(self):
        self.write('a.json', pod('x', 'y', 'z', 'w'))
        self.write('b.json', pod('x', 'y', 'z', 'v'))
        self.assertEqual(0, self.bench('pair', '--b', str(self.dir / 'b.json')))
        pair = json.loads((self.dir / 'pair/pair.json').read_text())
        self.assertEqual(('ok', [3], 8), (pair['status'], pair['changed_slots'], len(pair['rows'])))
        self.assertEqual({'a', 'b'}, set(pair['rows'][0]) & {'a', 'b'})
        self.assertEqual('a.json', pair['rows'][0]['a']['admission_sha256'])
        # A single arm is an A-only run with the same raw rows.
        self.assertEqual(0, self.bench('single'))

    def test_schedule_mismatch_ineligible_arm_and_wide_substitution_fail_cleanly(self):
        self.write('a.json', pod('x', 'y', 'z', 'w'))
        self.write('b.json', pod('x', 'y', 'z', 'v'))
        os.environ['FAKE_SHIFT'] = '1'
        self.assertEqual(1, self.bench('shifted', '--b', str(self.dir / 'b.json')))
        self.assertEqual('schedule_mismatch', json.loads((self.dir / 'shifted/pair.json').read_text())['status'])
        del os.environ['FAKE_SHIFT']
        os.environ['FAKE_REJECT'] = 'b.json'
        self.assertEqual(1, self.bench('rejected', '--b', str(self.dir / 'b.json')))
        pair = json.loads((self.dir / 'rejected/pair.json').read_text())
        self.assertEqual('arm_failed', pair['status'])
        self.assertIn('deck_gaps', pair['arms']['b']['failure'])
        self.write('wide.json', pod('x', 'q', 'z', 'v'))
        self.assertEqual(2, self.bench('wide', '--b', str(self.dir / 'wide.json')))
        self.assertFalse((self.dir / 'wide').exists())
        self.write('bad.json', {'decks': [[{'oracle_id': 'x', 'quantity': 1}]] * 4})
        self.assertEqual(2, self.bench('bad', '--b', str(self.dir / 'bad.json')))
        self.assertEqual(2, decks.main(['bench', str(self.sim), '--bulk-artifact', '.', '--manifest-sha', 'f',
                                        '--a', str(self.dir / 'a.json'), '--games', '6', '--out', str(self.dir / 'x')]))

    def test_stage_template_is_the_reviewed_current_profile(self):
        stage = importlib.util.spec_from_file_location('stage', Path(__file__).with_name('stage-deck.py'))
        module = importlib.util.module_from_spec(stage)
        stage.loader.exec_module(module)
        template = json.loads(module.TEMPLATE.read_text())
        self.assertEqual('commander-foundations-v1', template['name'])
        self.assertTrue((module.TEMPLATE.parent / template['commander_policy']['banned_list']).is_file())
        self.assertTrue((module.FIXTURES / module.R4_MANIFEST).is_file())


if __name__ == '__main__':
    unittest.main()
