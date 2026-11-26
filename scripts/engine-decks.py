#!/usr/bin/env python3
"""Generic Archive R5 deck harness: coverage, staging, play and paired benches.

  engine-decks.py coverage SIM --out NEW_DIR [--decks SNAPSHOT] [--all] [--archive R5.jsonl] [--baseline REPORT]
  engine-decks.py stage SIM OUTPUT (--deck NAME | --decklist FILE) [--decks FILE] [--archive R5.jsonl]
  engine-decks.py play (DECK | --decklist FILE) [SEED] [--decks FILE] [--archive R5.jsonl]
  engine-decks.py bench SIM --bulk-artifact DIR --manifest-sha SHA --pod-decks A.json --swap SLOT:REMOVE:ADD
                        --blocks N --out NEW_DIR [--seed-start N] [--threads N] [--decision-limit N]

SIM is an mtg-sim executable built from this checkout (release for timing).
coverage joins `SIM catalog coverage` receipts to a deck snapshot: compiled
slots and blocking cards per deck, top blockers, the Track 3A family
scoreboard and with --baseline the deltas against an earlier report.json.
The scoreboard classifies every blocker of every unsupported card (blockers.tsv)
into a family and reports, per family, the deck slots and unique deck cards it
touches and those it alone blocks ("sole": fixing the family alone unlocks
them), and with --all the same for Commander-legal archive cards. --all scans
the whole archive instead of only the snapshot's Oracle IDs. stage and play wrap stage-deck.py and
play-deck.sh. bench runs one bench-pair process, saves its report and renders a
Markdown summary. Rust owns scheduling, substitutions and statistics.
Inputs default to this checkout's generated/ file, else the primary checkout's.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
STATUSES = ('lowered_validated', 'unsupported', 'invalid', 'unparsed')
# Track 3A families (Phase 3 RFC section 2): the first family whose pattern
# matches a blocker code owns it. Order matters: specific families precede
# W6 (operands of existing verbs), and parser residue is last.
FAMILIES = (
    ('W1 attachments', (r'keyword:(equip|enchant|reconfigure|bestow|fortify)', r'ref:attached-to',
                        r'compose\.block\.(equipped|enchanted|attached)', r'^subtype:(curse|aura)',
                        r'compose\.step\.attach', r'static\.(modifier|keyword|cant)')),
    ('W2 as-enters and entry replacement', (r'ref:controller:entering-controller', r'ref:entering-object',
                                            r'static:(entry-choice|enters-tapped|entry-copy|opening-hand)',
                                            r'compose\.block\.(as-enters|enters-with-counters|copyOf)',
                                            r'cross-block:value-choice', r'compose\.step\.(choose-value|copyOf)')),
    ('W3 mana base', (r'^mana:', r'^cell:mana', r'compose\.block\.taps?-for-mana', r'^cost:', r'compose\.cost',
                      r'^restriction:', r'event-object:taps-for-mana', r'verb:add-mana', r'compose\.step\.combination')),
    ('W4 search and library', (r'verb:(search|look-at|reveal|shuffle)', r'compose\.step\.(scan-library|shuffle-into-library|look|reveal)',
                               r'compose\.block\.look-permission')),
    ('W5 step results, may, if, up to, for each', (r'ref:(step-result|bound-var|linked-pool|derived|evaluated)',
                                                   r'compose\.step\.(optional|forEach|for-each|condition|pay-cost|upTo)',
                                                   r'compose\.effects:', r'compose\.control:', r'effects\.condition', r'compose\.block\.condition',
                                                   r'^bindings', r'operand:bindings', r'operand:target:up_to',
                                                   r'^verb:[^:]+:optional$')),
    ('W7 trigger events', (r'ref:event-(object|player)', r'ref:source:', r'ref:target:', r'ref:controller:',
                           r'abilities\.(trigger|condition)', r'^cell:trigger', r'^trigger:',
                           r'compose\.block\.(leaves|blocks|discard|gains-life|player-attacks|counter-change|exile|move-zone|tap-objects|is-filter|control-count|count|occurrence|properties|cls|subtypes|from|group|grouping|counterConstraints|create-token)')),
    ('W8 static and continuous effects', (r'^static', r'^cell:static', r'abilities\.static', r'compose\.block:cost_modification',
                                          r'compose\.step\.(modify-characteristics|cant|become|change-text|player-gain-abilities|gain-control)',
                                          r'compose\.block\.player-combat-rule')),
    ('W9 replacement and prevention', (r'replacement', r'prevent-damage', r'redirect-damage')),
    ('W10 cost keywords and variable costs', (r'source\.mana_cost', r'kind:additional-cost', r'compose\.block\.alternative-cost',
                                              r'^keyword:', r'^cell:keyword', r'abilities\.keywords', r'source\.keywords')),
    ('W11 multi-face and special layouts', (r'source\.layout',)),
    ('W12 designations and keyword actions', (r'become-monarch', r'take-initiative', r'keyword-action', r'fight')),
    ('W13 delayed triggers', (r'create-delayed-trigger', r'add-phases')),
    ('W14 stack copy and other-zone casting', (r'copy-stack-object', r'compose\.block\.copy$', r'choose-new-targets',
                                               r'copy-card', r'verb:(play-card|modal)', r'linked-exile', r'hideaway',
                                               r'compose\.step\.(play|modal)')),
    ('W15 characteristic shells', (r'source\.(power|toughness|type_line)', r'^subtype:', r'^cell:shell',
                                   r'commander-set', r'compose\.step\.riders')),
    ('W6 operand totality for existing verbs', (r'^verb:', r'^operand:', r'_shape_outside_the_lowering_mirror', r'^cell:(target|cost|program|activated)',
                                                r'^(filter|target|targets|set|who|definition|amount|regeneration|forEach):',
                                                r'^effects\.', r'compose\.step\.')),
    ('Parser residue', (r'^resolution:', r'^ref:(unsupported|unresolved|ambiguous|missing)', r'^status:', r'^lines:')),
)
LONG_TAIL = 'Long tail'
_FAMILY_PATTERNS = [(name, [re.compile(p) for p in patterns]) for name, patterns in FAMILIES]


def shared_generated(name):
    local = ROOT / 'generated' / name
    if local.exists():
        return local
    common = subprocess.run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT,
                            capture_output=True, text=True).stdout.strip()
    return Path(common).parent / 'generated' / name if common else local


def require(condition, message):
    if not condition:
        raise ValueError(message)


def new_directory(path):
    path = Path(path)
    require(not os.path.lexists(path), f'{path} already exists')
    path.mkdir(parents=True)
    return path


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def read_snapshot(path):
    snapshot = json.loads(Path(path).read_bytes())
    decks = snapshot['decks']
    require(snapshot['deckCount'] == len(decks), 'snapshot deckCount mismatch')
    ids = set()
    for deck in decks:
        require(type(deck['id']) is int and deck['id'] not in ids, f"invalid deck ID {deck['id']!r}")
        ids.add(deck['id'])
        seen = set()
        for card in deck['cards']:
            require(isinstance(card['oracle_id'], str) and card['oracle_id'] not in seen,
                    f"invalid or repeated Oracle ID {card['oracle_id']!r} in deck {deck['id']}")
            require(type(card['quantity']) is int and card['quantity'] > 0,
                    f"invalid quantity for {card['oracle_id']} in deck {deck['id']}")
            seen.add(card['oracle_id'])
    return snapshot


def read_records(path):
    """oracle_id -> (status, reason) from coverage records.tsv."""
    lines = Path(path).read_text().splitlines()
    require(lines and lines[0] == 'oracle_id\tname\tstatus\treason', f'{path}: unexpected header')
    rows = {}
    for line in lines[1:]:
        oracle_id, _name, status, reason = line.split('\t', 3)
        require(status in STATUSES and oracle_id not in rows, f'{path}: invalid row {oracle_id}')
        rows[oracle_id] = (status, reason)
    return rows


def family(code):
    """The Track 3A family a blocker code belongs to."""
    for name, patterns in _FAMILY_PATTERNS:
        if any(p.search(code) for p in patterns):
            return name
    return LONG_TAIL


def read_blockers(path):
    """oracle_id -> (commander_legal, set of blocker codes) from coverage blockers.tsv."""
    lines = Path(path).read_text().splitlines()
    require(lines and lines[0] == 'oracle_id\tcommander_legal\tkind\tblock\tcode', f'{path}: unexpected header')
    cards = {}
    for line in lines[1:]:
        oracle_id, legal, kind, _block, code = line.split('\t', 4)
        require(legal in ('true', 'false') and kind in ('binding', 'block', 'card'), f'{path}: invalid row {oracle_id}')
        cards.setdefault(oracle_id, (legal == 'true', set()))[1].add(code)
    return cards


def scoreboard(snapshot, rows, blockers, whole_archive):
    """Per family: deck slots/cards it touches and solely blocks, and (whole
    archive) Commander-legal archive cards it touches and solely blocks."""
    families = {oracle_id: {family(code) for code in codes} for oracle_id, (_legal, codes) in blockers.items()}
    slots = Counter()
    for deck in snapshot['decks']:
        for card in deck['cards']:
            slots[card['oracle_id']] += card['quantity']
    blocked = [oracle_id for oracle_id in slots if rows.get(oracle_id, ('missing',))[0] == 'unsupported']
    table = defaultdict(Counter)
    for oracle_id in blocked:
        owned = families.get(oracle_id, set())
        for name in owned:
            table[name]['deck_slots_touched'] += slots[oracle_id]
            table[name]['deck_cards_touched'] += 1
            if len(owned) == 1:
                table[name]['deck_slots_sole'] += slots[oracle_id]
                table[name]['deck_cards_sole'] += 1
    if whole_archive:
        for oracle_id, (legal, _codes) in blockers.items():
            if not legal or rows.get(oracle_id, ('missing',))[0] != 'unsupported':
                continue
            owned = families[oracle_id]
            for name in owned:
                table[name]['archive_touched'] += 1
                if len(owned) == 1:
                    table[name]['archive_sole'] += 1
    keys = ['deck_slots_touched', 'deck_cards_touched', 'deck_slots_sole', 'deck_cards_sole']
    if whole_archive:
        keys += ['archive_touched', 'archive_sole']
    names = [name for name, _ in FAMILIES] + [LONG_TAIL]
    report = [{'family': name, **{key: table[name][key] for key in keys}} for name in names]
    # Deck-first: rank by the deck slots a family alone unlocks, then touches.
    return sorted(report, key=lambda row: (-row['deck_slots_sole'], -row['deck_slots_touched'], row['family']))


def deck_report(deck, rows):
    total = sum(card['quantity'] for card in deck['cards'])
    blocked = []
    for card in sorted(deck['cards'], key=lambda card: card['oracle_id']):
        status, reason = rows.get(card['oracle_id'], ('missing', 'not in archive'))
        if status != 'lowered_validated':
            blocked.append({'oracle_id': card['oracle_id'], 'name': card.get('name'),
                            'quantity': card['quantity'], 'status': status, 'reason': reason})
    compiled = total - sum(card['quantity'] for card in blocked)
    return {'id': deck['id'], 'name': deck['name'], 'total_slots': total, 'compiled_slots': compiled,
            'compiled_percent': round(100 * compiled / total, 2) if total else 0.0, 'blocked': blocked}


def compare(report, baseline):
    before = {deck['id']: deck for deck in baseline['decks']}
    decks = []
    for deck in report['decks']:
        old = before.get(deck['id'])
        if old is None:
            continue
        old_blocked = {card['oracle_id'] for card in old['blocked']}
        new_blocked = {card['oracle_id'] for card in deck['blocked']}
        decks.append({'id': deck['id'], 'name': deck['name'],
                      'compiled_slots_delta': deck['compiled_slots'] - old['compiled_slots'],
                      'newly_compiled': sorted(old_blocked - new_blocked),
                      'newly_blocked': sorted(new_blocked - old_blocked)})
    old_reasons = {row['reason']: row['cards'] for row in baseline['top_blockers']}
    reasons = {row['reason']: row['cards'] - old_reasons.get(row['reason'], 0) for row in report['top_blockers']}
    reasons.update({reason: -cards for reason, cards in old_reasons.items() if reason not in reasons})
    old_families = {row['family']: row for row in baseline.get('families', [])}
    family_deltas = {}
    for row in report['families']:
        old = old_families.get(row['family'], {})
        delta = {key: value - old.get(key, 0) for key, value in row.items() if key != 'family'}
        if any(delta.values()):
            family_deltas[row['family']] = delta
    return {'archive_changed': baseline['archive_sha256'] != report['archive_sha256'], 'decks': decks,
            'blocker_card_deltas': {reason: delta for reason, delta in sorted(reasons.items()) if delta},
            'family_deltas': family_deltas}


def coverage(args):
    snapshot_path = Path(args.decks).resolve()
    snapshot = read_snapshot(snapshot_path)
    out = new_directory(args.out)
    receipts = out / 'receipts'
    command = [str(args.sim), 'catalog', 'coverage', str(Path(args.archive).resolve()), '--out', str(receipts)]
    if not args.all:
        command += ['--only-deck-snapshot', str(snapshot_path)]
    result = subprocess.run(command, capture_output=True, text=True)
    require(result.returncode == 0, f'catalog coverage failed ({result.returncode}): {result.stderr.strip()[-2000:]}')
    summary = json.loads((receipts / 'summary.json').read_text())
    rows = read_records(receipts / 'records.tsv')
    blockers = read_blockers(receipts / 'blockers.tsv')
    decks = [deck_report(deck, rows) for deck in sorted(snapshot['decks'], key=lambda deck: deck['id'])]
    reasons = Counter()
    for deck in decks:
        for card in deck['blocked']:
            reasons[card['reason']] += 1
    report = {'report_version': 1, 'archive_sha256': summary['archive_sha256'],
              'deck_snapshot_sha256': hashlib.sha256(snapshot_path.read_bytes()).hexdigest(),
              'scope': 'whole_archive' if args.all else 'deck_selection', 'archive_summary': {
                  key: summary[key] for key in ('total', 'lowered_validated', 'lowered_validated_percent', 'statuses')},
              'decks': decks, 'top_blockers': [{'reason': reason, 'cards': cards}
                                               for reason, cards in sorted(reasons.items(), key=lambda r: (-r[1], r[0]))],
              'families': scoreboard(snapshot, rows, blockers, args.all)}
    if args.baseline:
        report['baseline'] = compare(report, json.loads(Path(args.baseline).read_text()))
    write_json(out / 'report.json', report)
    for deck in decks:
        print(f"{deck['compiled_slots']:>3}/{deck['total_slots']:<3} {deck['name']}")
    archive = ' archive touched/sole' if args.all else ''
    print(f'family scoreboard: deck slots touched/sole, cards touched/sole{archive}')
    for row in report['families']:
        line = (f"{row['deck_slots_touched']:>5}/{row['deck_slots_sole']:<5} "
                f"{row['deck_cards_touched']:>4}/{row['deck_cards_sole']:<4}")
        if args.all:
            line += f" {row['archive_touched']:>6}/{row['archive_sole']:<6}"
        print(f"{line} {row['family']}")
    print(f'report: {out / "report.json"}')
    return 0


def bench(args):
    command = [str(args.sim), 'bench-pair', '--bulk-artifact', args.bulk_artifact,
               '--manifest-sha', args.manifest_sha, '--pod-decks', args.pod_decks,
               '--blocks', str(args.blocks), '--seed-start', str(args.seed_start), '--out', str(args.out)]
    for swap in args.swap:
        command.extend(['--swap', swap])
    for flag, value in [('--threads', args.threads), ('--decision-limit', args.decision_limit)]:
        if value is not None:
            command.extend([flag, str(value)])
    result = subprocess.run(command, stdout=subprocess.PIPE, text=True)
    if result.returncode:
        return result.returncode
    report = json.loads(result.stdout)
    write_json(args.out / 'report.json', report)
    stats = report['statistics']
    table = stats['censoring_table']
    lines = ['# Paired Commander benchmark', '',
             f"**{stats['verdict']}** across {args.blocks} seed blocks.", '',
             '| Focal win-rate difference | Estimate | 95% block-bootstrap interval |',
             '| --- | ---: | --- |',
             f"| Complete blocks | {stats['estimate']} | {stats['ci95']} |",
             f"| Complete pairs | {stats['complete_case_estimate']} | {stats['complete_case_ci95']} |", '',
             f"Worst-case bounds {stats['worst_case_bounds']}. Two-sided sign-test p = {stats['sign_test']['two_sided_p']}.", '',
             '| A outcome | A′ complete | A′ censored |', '| --- | ---: | ---: |',
             f'| Complete | {table[0][0]} | {table[0][1]} |', f'| Censored | {table[1][0]} | {table[1][1]} |']
    (args.out / 'summary.md').write_text('\n'.join(lines) + '\n')
    print(f"{stats['verdict']} · {args.out / 'report.json'}")
    return 0


def main(argv=None):
    argv = sys.argv[1:] if argv is None else list(argv)
    # stage and play forward every argument verbatim (including their own
    # --decklist/--decks/--archive flags) to the wrapped tools.
    if argv[:1] == ['stage']:
        return subprocess.run([sys.executable, '-B', str(ROOT / 'scripts/stage-deck.py'), *argv[1:]]).returncode
    if argv[:1] == ['play']:
        return subprocess.run(['bash', str(ROOT / 'scripts/play-deck.sh'), *argv[1:]]).returncode
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    cov = commands.add_parser('coverage')
    cov.add_argument('sim', type=Path)
    cov.add_argument('--out', type=Path, required=True)
    cov.add_argument('--decks', type=Path, default=shared_generated('phase1-target-decks.json'))
    cov.add_argument('--archive', type=Path, default=shared_generated('oracle-ast-phase1-r5.jsonl'))
    cov.add_argument('--all', action='store_true', help='scan the whole archive, not only snapshot IDs')
    cov.add_argument('--baseline', type=Path)
    commands.add_parser('stage', help='forwarded verbatim to stage-deck.py')
    commands.add_parser('play', help='forwarded verbatim to play-deck.sh')
    pods = commands.add_parser('bench')
    pods.add_argument('sim', type=Path)
    pods.add_argument('--bulk-artifact', required=True)
    pods.add_argument('--manifest-sha', required=True)
    pods.add_argument('--pod-decks', required=True)
    pods.add_argument('--swap', action='append', required=True)
    pods.add_argument('--blocks', type=int, required=True)
    pods.add_argument('--seed-start', type=int, default=0)
    pods.add_argument('--threads', type=int)
    pods.add_argument('--decision-limit', type=int)
    pods.add_argument('--out', type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        return coverage(args) if args.command == 'coverage' else bench(args)
    except (OSError, ValueError, KeyError) as error:
        print(f'engine-decks: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
