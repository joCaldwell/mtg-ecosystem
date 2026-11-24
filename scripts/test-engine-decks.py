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
    blockers = json.loads(os.environ.get('FAKE_BLOCKERS', '{}'))
    rows = ['oracle_id\tcommander_legal\tkind\tblock\tcode'] + [
        f'{k}\t{legal}\tblock\t0\t{code}' for k, (legal, codes) in sorted(blockers.items()) for code in codes]
    (out / 'blockers.tsv').write_text('\n'.join(rows) + '\n')
    (out / 'summary.json').write_text(json.dumps({'archive_sha256': os.environ.get('FAKE_ARCHIVE', 'a' * 64),
        'total': len(statuses), 'lowered_validated': 0, 'lowered_validated_percent': 0.0, 'statuses': {}}))
    sys.exit(0)
assert args[0] == 'bench-pair'
out = Path(args[args.index('--out') + 1])
if os.environ.get('FAKE_REJECT'): sys.exit(7)
out.mkdir()
(out / 'pairs.jsonl').write_text('raw rows')
print(json.dumps({'args':args, 'statistics':{'verdict':'valid', 'estimate':0, 'ci95':[0,0],
    'complete_case_estimate':0,'complete_case_ci95':[0,0], 'worst_case_bounds':[0,0],
    'sign_test':{'two_sided_p':1}, 'censoring_table':[[8,0],[0,0]]}}))
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
        self.assertTrue(all(row['deck_slots_touched'] == 0 for row in report['families']))
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

    def test_family_scoreboard_counts_touching_and_sole_blockers(self):
        snapshot = self.write('decks.json', {'deckCount': 1, 'decks': [{'id': 1, 'name': 'Test', 'cards': [
            {'oracle_id': 'aura', 'quantity': 1}, {'oracle_id': 'dual', 'quantity': 2},
            {'oracle_id': 'both', 'quantity': 1}]}]})
        archive = self.write('archive.jsonl', {})
        os.environ['FAKE_STATUSES'] = json.dumps({k: ['unsupported', 'x'] for k in ('aura', 'dual', 'both', 'other')})
        os.environ['FAKE_BLOCKERS'] = json.dumps({
            'aura': ['true', ['ref:attached-to:static', 'keyword:enchant:parameters']],
            'dual': ['true', ['cell:mana:choice']],
            'both': ['true', ['cell:mana:choice', 'operand:filter:chosen_color']],
            'other': ['false', ['verb:scry:unlowered']]})
        out = self.dir / 'families'
        self.assertEqual(0, decks.main(['coverage', str(self.sim), '--out', str(out), '--decks', str(snapshot),
                                        '--archive', str(archive), '--all']))
        rows = {row['family']: row for row in json.loads((out / 'report.json').read_text())['families']}
        self.assertEqual({'deck_slots_touched': 1, 'deck_cards_touched': 1, 'deck_slots_sole': 1, 'deck_cards_sole': 1,
                          'archive_touched': 1, 'archive_sole': 1},
                         {k: v for k, v in rows['W1 attachments'].items() if k != 'family'})
        mana = rows['W3 mana base']
        self.assertEqual((3, 2, 2, 1, 2, 1), (mana['deck_slots_touched'], mana['deck_cards_touched'],
                                              mana['deck_slots_sole'], mana['deck_cards_sole'],
                                              mana['archive_touched'], mana['archive_sole']))
        # A Commander-illegal archive card is not counted.
        self.assertEqual(0, rows['W6 operand totality for existing verbs']['archive_sole'])
        self.assertEqual(1, rows['W6 operand totality for existing verbs']['deck_slots_touched'])
        self.assertEqual('W1 attachments', decks.family('ref:attached-to:static'))
        self.assertEqual('Parser residue', decks.family('resolution:diagnostic_unsupported_scope'))

    def test_printed_optionality_is_a_control_flow_blocker(self):
        snapshot = self.write('decks.json', {'deckCount': 1, 'decks': [{'id': 1, 'name': 'Test', 'cards': [
            {'oracle_id': 'may', 'quantity': 2}, {'oracle_id': 'scry', 'quantity': 1}]}]})
        archive = self.write('archive.jsonl', {})
        os.environ['FAKE_STATUSES'] = json.dumps({k: ['unsupported', 'x'] for k in ('may', 'scry')})
        os.environ['FAKE_BLOCKERS'] = json.dumps({'may': ['true', ['verb:draw:optional']],
                                                  'scry': ['true', ['verb:scry:unlowered']]})
        out = self.dir / 'optional'
        self.assertEqual(0, decks.main(['coverage', str(self.sim), '--out', str(out), '--decks', str(snapshot),
                                        '--archive', str(archive), '--all']))
        rows = {row['family']: row for row in json.loads((out / 'report.json').read_text())['families']}
        control = rows['W5 step results, may, if, up to, for each']
        operands = rows['W6 operand totality for existing verbs']
        self.assertEqual((2, 1), (control['deck_slots_sole'], control['archive_sole']))
        self.assertEqual((1, 1), (operands['deck_slots_sole'], operands['archive_sole']))
        self.assertEqual('W6 operand totality for existing verbs', decks.family('verb:damage:optional_target'))

    def bench(self, out, *extra):
        return decks.main(['bench', str(self.sim), '--bulk-artifact', str(self.dir), '--manifest-sha', 'f' * 64,
                           '--pod-decks', 'a.json', '--blocks', '2', '--swap', '1:x:y', '--out', str(self.dir / out), *extra])

    def test_bench_forwards_one_command_and_renders_the_native_report(self):
        self.assertEqual(0, self.bench('pair', '--swap', '1:a:b', '--threads', '2', '--decision-limit', '0'))
        out = self.dir / 'pair'
        command = json.loads((out / 'report.json').read_text())['args']
        self.assertEqual(2, command.count('--swap'))
        self.assertEqual(['2', '0'], [command[command.index(flag)+1] for flag in ('--threads','--decision-limit')])
        self.assertIn('**valid**', (out / 'summary.md').read_text())
        self.assertEqual('raw rows', (out / 'pairs.jsonl').read_text())

    def test_bench_propagates_native_admission_failure(self):
        os.environ['FAKE_REJECT'] = '1'
        self.assertEqual(7, self.bench('rejected'))
        self.assertFalse((self.dir / 'rejected').exists())

    def test_play_builds_in_the_detached_checkouts_own_target(self):
        # Stub git/engine-env record every native call's CARGO_TARGET_DIR; the
        # stubbed build fails, so play stops before staging.
        root = self.dir / 'repo'
        (root / 'scripts').mkdir(parents=True)
        play = Path(__file__).with_name('play-deck.sh')
        (root / 'scripts/play-deck.sh').write_text(play.read_text())
        trace, stubs = self.dir / 'trace', self.dir / 'bin'
        stubs.mkdir()
        (stubs / 'git').write_text(f"""#!/bin/sh
case "$*" in
  'rev-parse --show-toplevel') echo {root} ;;
  'worktree add --detach '*) mkdir -p "$4/scripts"; cp {self.dir}/engine-env.sh "$4/scripts/engine-env.sh" ;;
  *) exit 3 ;;
esac
""")
        (self.dir / 'engine-env.sh').write_text(f"""#!/bin/sh
echo "$CARGO_TARGET_DIR|$MTG_ENGINE_LOCK_FD|$*" >> {trace}
case "$*" in
  *'engine-cache.py lock') echo {self.dir}/locks/cargo.lock ;;
  *'engine-cache.py check') echo '{{}}' ;;
  *) exit 7 ;;
esac
""")
        (stubs / 'git').chmod(0o755)
        for name in ('archive.jsonl', 'deck.json'):
            (self.dir / name).write_text('{}')
        result = __import__('subprocess').run(
            ['bash', str(root / 'scripts/play-deck.sh'), '--decklist', str(self.dir / 'deck.json'), '--archive', str(self.dir / 'archive.jsonl')],
            cwd=root, capture_output=True, text=True,
            env={**os.environ, 'PATH': f"{stubs}:{os.environ['PATH']}", 'CARGO_TARGET_DIR': str(self.dir / 'caller-target')})
        self.assertNotEqual(0, result.returncode)
        calls = [line.split('|', 2) for line in trace.read_text().splitlines()]
        run = next((root / 'generated').glob('deck-play.*'))
        self.assertEqual(['lock', 'check', 'build'], [c[2].split()[-1] if 'engine-cache' in c[2] else 'build' for c in calls])
        self.assertTrue(all(c[0] == str(run / 'source/target/engine') for c in calls), calls)
        self.assertEqual('9', calls[1][1])

    def test_stage_and_play_forward_every_argument_verbatim(self):
        from unittest.mock import patch
        calls = []
        fake = lambda command, **_: (calls.append(command), type('R', (), {'returncode': 0})())[1]
        with patch.object(decks.subprocess, 'run', fake):
            self.assertEqual(0, decks.main(['play', '--decklist', 'deck.json', '3', '--archive', 'r4.jsonl']))
            self.assertEqual(0, decks.main(['stage', 'SIM', 'OUT', '--decklist', 'deck.json', '--decks', 'd.json']))
        self.assertEqual(['--decklist', 'deck.json', '3', '--archive', 'r4.jsonl'], calls[0][2:])
        self.assertTrue(calls[0][1].endswith('play-deck.sh'))
        self.assertEqual(['SIM', 'OUT', '--decklist', 'deck.json', '--decks', 'd.json'], calls[1][3:])

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
