#!/usr/bin/env python3
"""Generic Archive R4 deck harness: coverage, staging, play and paired benches.

  engine-decks.py coverage SIM --out NEW_DIR [--decks SNAPSHOT] [--all] [--archive R4.jsonl] [--baseline REPORT]
  engine-decks.py stage SIM OUTPUT (--deck NAME | --decklist FILE) [--decks FILE] [--archive R4.jsonl]
  engine-decks.py play (DECK | --decklist FILE) [SEED] [--decks FILE] [--archive R4.jsonl]
  engine-decks.py bench SIM --bulk-artifact DIR --manifest-sha SHA --a A.json [--b B.json]
                        --games N --out NEW_DIR [--seed-start N] [--threads N] [--decision-limit N]

SIM is an mtg-sim executable built from this checkout (release for timing).
coverage joins `SIM catalog coverage` receipts to a deck snapshot: compiled
slots and blocking cards per deck, top blockers, and with --baseline the
deltas against an earlier report.json. --all scans the whole archive instead
of only the snapshot's Oracle IDs. stage and play wrap stage-deck.py and
play-deck.sh. bench runs `SIM bench-pods --rotate-seats` for deck file A and,
optionally, A′ (B) with the same seeds, seats and limits, and writes raw
paired rows only; interpreting them statistically belongs to Phase 3B. A′ may
change at most one deck slot. A failed arm stays recorded as a failed pair.
Inputs default to this checkout's generated/ file, else the primary checkout's.
"""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
STATUSES = ('lowered_validated', 'unsupported', 'invalid', 'unparsed')


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
    return {'archive_changed': baseline['archive_sha256'] != report['archive_sha256'], 'decks': decks,
            'blocker_card_deltas': {reason: delta for reason, delta in sorted(reasons.items()) if delta}}


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
                                               for reason, cards in sorted(reasons.items(), key=lambda r: (-r[1], r[0]))]}
    if args.baseline:
        report['baseline'] = compare(report, json.loads(Path(args.baseline).read_text()))
    write_json(out / 'report.json', report)
    for deck in decks:
        print(f"{deck['compiled_slots']:>3}/{deck['total_slots']:<3} {deck['name']}")
    print(f'report: {out / "report.json"}')
    return 0


def normalized_pod(path):
    """Four seat lists with rows sorted; rejects anything bench-pods would."""
    pod = json.loads(Path(path).read_text())
    require(isinstance(pod, dict) and isinstance(pod.get('decks'), list) and len(pod['decks']) == 4,
            f'{path}: expected an object with four seat decks')
    seats = []
    for deck in pod['decks']:
        rows = []
        for row in deck:
            require(set(row) == {'oracle_id', 'quantity', 'role'} and type(row['quantity']) is int
                    and row['role'] in ('main', 'commander', 'companion'), f'{path}: invalid deck row {row!r}')
            rows.append((row['oracle_id'], row['role'], row['quantity']))
        seats.append(sorted(rows))
    return seats


def schedule(report):
    """The seed, seat and limit schedule every arm must share exactly."""
    return {'games': report['games'], 'seed_start': report['seed_start'], 'rotate_seats': report['rotate_seats'],
            'limits': report['limits'], 'policy': report['policy'],
            'cases': [[row['case'], row['starting_seat']] for row in report['rows']]}


def bench(args):
    require(args.games > 0 and args.games % 4 == 0, '--games must be a positive multiple of four (seats rotate)')
    for flag, value in (('--threads', args.threads), ('--decision-limit', args.decision_limit)):
        require(value is None or value > 0, f'{flag} must be positive')
    arms = {'a': Path(args.a).resolve()}
    pods = {'a': normalized_pod(arms['a'])}
    if args.b:
        arms['b'] = Path(args.b).resolve()
        pods['b'] = normalized_pod(arms['b'])
        changed = [slot for slot in range(4) if pods['a'][slot] != pods['b'][slot]]
        require(len(changed) <= 1, f"A′ must change at most one deck slot; it changes slots {changed}")
    out = new_directory(args.out)
    pair = {'pair_version': 1, 'arms': {}, 'changed_slots': [], 'status': 'ok'}
    if args.b:
        pair['changed_slots'] = [slot for slot in range(4) if pods['a'][slot] != pods['b'][slot]]
    reports = {}
    for arm, path in arms.items():
        command = [str(args.sim), 'bench-pods', '--bulk-artifact', str(Path(args.bulk_artifact).resolve()),
                   '--manifest-sha', args.manifest_sha, '--pod-decks', str(path), '--games', str(args.games),
                   '--seed-start', str(args.seed_start), '--rotate-seats', '--analytics-out', str(out / f'{arm}-analytics')]
        # Explicit values always reach bench-pods, which owns their validation.
        if args.threads is not None:
            command += ['--threads', str(args.threads)]
        if args.decision_limit is not None:
            command += ['--decision-limit', str(args.decision_limit)]
        result = subprocess.run(command, capture_output=True, text=True)
        (out / f'{arm}.stdout.json').write_text(result.stdout)
        (out / f'{arm}.stderr.txt').write_text(result.stderr)
        record = {'pod_decks': str(path), 'exit_code': result.returncode, 'command': command}
        try:
            report = json.loads(result.stdout)
        except ValueError:
            report = None
        if result.returncode or not isinstance(report, dict) or 'rows' not in report:
            # Admission rejections print a typed JSON error; keep it verbatim.
            record['failure'] = (result.stdout.strip() or result.stderr.strip())[-4000:]
            pair['status'] = 'arm_failed'
        else:
            reports[arm] = report
            record.update({'build_id': report['build']['id'], 'catalog_sha256': report['catalog_sha256'],
                           'bulk': report.get('bulk'), 'proofs_verified': report['proofs_verified']})
        pair['arms'][arm] = record
    if pair['status'] == 'ok' and len(reports) == 2 and schedule(reports['a']) != schedule(reports['b']):
        pair['status'] = 'schedule_mismatch'
    if reports and pair['status'] != 'schedule_mismatch':
        first = next(iter(reports.values()))
        pair['schedule'] = schedule(first)
        pair['rows'] = [{'case': row['case'], 'starting_seat': row['starting_seat'],
                         **{arm: {key: report['rows'][index][key] for key in
                                  ('outcome', 'winner', 'placements', 'turns', 'decisions', 'admission_sha256')
                                  if key in report['rows'][index]}
                            for arm, report in reports.items()}}
                        for index, row in enumerate(first['rows'])]
    write_json(out / 'pair.json', pair)
    print(f"{pair['status']}: {out / 'pair.json'}")
    return 0 if pair['status'] == 'ok' else 1


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
    cov.add_argument('--archive', type=Path, default=shared_generated('oracle-ast-phase1-r4.jsonl'))
    cov.add_argument('--all', action='store_true', help='scan the whole archive, not only snapshot IDs')
    cov.add_argument('--baseline', type=Path)
    commands.add_parser('stage', help='forwarded verbatim to stage-deck.py')
    commands.add_parser('play', help='forwarded verbatim to play-deck.sh')
    pods = commands.add_parser('bench')
    pods.add_argument('sim', type=Path)
    pods.add_argument('--bulk-artifact', required=True)
    pods.add_argument('--manifest-sha', required=True)
    pods.add_argument('--a', required=True)
    pods.add_argument('--b')
    pods.add_argument('--games', type=int, required=True)
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
