#!/usr/bin/env python3
"""Compare whole-archive coverage on fixed Engine Rework deck and card cohorts.

Run via sh scripts/engine-env.sh python3 -B scripts/rework-sweep.py --help.
Each run accepts a simulator, whole-archive coverage receipts, or both. A
simulator also runs native headless admission, using an existing bulk artifact
when supplied. Receipt-only comparisons may supply a matching bulk artifact
and admission.json from an earlier run. Held-out snapshots contribute only
pool-wide slot totals, and are never passed to the simulator.

The cohort directory holds the four frozen, sorted Oracle-ID lists. Optional
qualification receipts use {manifest_sha256, configuration_sha256, games:
[{deck_id, seed, status}]}; deck_id is the original Pool B training snapshot
ID and status is complete, censored or failed. Each row describes one deck's
participation in a seeded game under the pinned training configuration.
"""
import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import importlib.util
import json
from pathlib import Path
import re
from statistics import median
import subprocess
import sys
from typing import NamedTuple

from rework_sweep_diagnostics import (OTHER_CLASSES, PHASES, UNASSIGNED, archive_residuals,
                                      rejection_phase, sole_other)
from rework_sweep_guardrails import guardrails

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('engine_decks', ROOT / 'scripts/engine-decks.py')
engine_decks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(engine_decks)
new_directory = engine_decks.new_directory
read_records = engine_decks.read_records
read_snapshot = engine_decks.read_snapshot
require = engine_decks.require
write_json = engine_decks.write_json
SUPPORTED = 'lowered_validated'
POOL_A = 'Pool A'
TRAIN = 'Pool B training (active)'
HELD = 'Pool B held-out (active aggregate)'
CONTROL = 'Pool B training (Sweep 5 control)'
HELD_CONTROL = 'Pool B held-out (Sweep 5 aggregate)'
COHORTS = {'pilot-439': 439, 'pilot-481': 481, 'mixed-803': 803, 'ambiguity-311': 311}
PILOT_SHA = 'fdf409faa8b0bcf89c169daf39db6383fa17d65aae709e7f8e843b7ff9ac36b0'
SHA = re.compile(r'[0-9a-f]{64}')
COMPILE_GAPS = {'archive_missing', 'archive_excluded', 'unparsed', 'compiler_unsupported', 'compiler_invalid'}


class RunInputs(NamedTuple):
    sim: Path | None
    coverage: Path | None
    bulk: Path | None
    admission: Path | None
    source: Path
    qualification: Path | None


class Measurement(NamedTuple):
    report: dict
    legal: set
    supported: set
    corpus: tuple
    outcomes: dict


def read_json(path):
    return json.loads(path.read_bytes())


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def deck_slots(deck):
    return Counter({card['oracle_id']: card['quantity'] for card in deck['cards']})


def load_pools(a_path, b_path, control_path):
    a = read_snapshot(a_path)['decks']

    def read_pool_b(path):
        try:
            return read_snapshot(path)['decks']
        except (KeyError, ValueError, TypeError) as error:
            # The shared reader's invalid-card error can identify a held-out
            # row. Keep all Pool B document validation errors aggregate.
            raise ValueError(f'invalid Pool B snapshot at {path}') from error

    def split(decks, training, heldout):
        train = [deck for deck in decks if deck['split'] == 'train']
        held = [deck for deck in decks if deck['split'] == 'heldout']
        require((len(train), len(held), len(decks)) == (training, heldout, training + heldout),
                f'expected {training} training and {heldout} held-out Pool B decks')
        return train, held

    require(len(a) == 15, 'expected 15 Pool A decks')
    train, held = split(read_pool_b(b_path), 157, 39)
    pools = {POOL_A: a, TRAIN: train, HELD: held}
    if control_path:
        control, control_held = split(read_pool_b(control_path), 161, 45)
        for active, original in ((train, control), (held, control_held)):
            by_id = {deck['id']: deck['cards'] for deck in original}
            require(all(by_id.get(deck['id']) == deck['cards'] for deck in active),
                    'active Pool B must be an unchanged subset of its Sweep 5 control')
        pools.update({CONTROL: control, HELD_CONTROL: control_held})
    return pools


def load_cohorts(directory):
    cohorts = {}
    pins = {}
    for name, count in COHORTS.items():
        path = directory / f'{name}-oracle-ids.txt'
        data = path.read_bytes()
        ids = data.decode('ascii').splitlines()
        require(len(ids) == count and ids == sorted(set(ids))
                and all(re.fullmatch(r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', oid) for oid in ids)
                and data == ''.join(oid + '\n' for oid in ids).encode(),
                f'invalid sorted {name} cohort in {path}')
        cohorts[name] = set(ids)
        pins[name] = hashlib.sha256(data).hexdigest()
    require(pins['pilot-439'] == PILOT_SHA, f"pilot-439 SHA-256 mismatch: {pins['pilot-439']}")
    return cohorts, pins


def blocked_slots(slots, supported):
    return sum(quantity for oid, quantity in slots.items() if oid not in supported)


def rankable_decks(pools):
    # Preserve the Sweep 5 inspector IDs when its control is supplied. Match
    # the active subset back to those same IDs and inspect each deck once.
    return [(pool, deck) for pool in (POOL_A, CONTROL if CONTROL in pools else TRAIN)
            for deck in pools[pool]]


def pool_report(decks, supported, rankable=False):
    slots = [deck_slots(deck) for deck in decks]
    blocked = [blocked_slots(quantities, supported) for quantities in slots]
    total = [sum(quantities.values()) for quantities in slots]
    result = {'compiled_slots': sum(total) - sum(blocked), 'total_slots': sum(total)}
    if rankable:
        result.update(decks=len(decks), slot_complete=blocked.count(0),
                      at_least_90_percent=sum(n * 10 <= t for n, t in zip(blocked, total, strict=True)),
                      min_blocked=min(blocked), median_blocked=median(blocked),
                      blocked_slot_distribution=dict(sorted(Counter(blocked).items())),
                      deck_scores=[{'id': d['id'], 'name': d['name'], 'blocked_slots': n, 'total_slots': t}
                                   for d, n, t in zip(decks, blocked, total, strict=True)])
    return result


def read_observations(path):
    observations = defaultdict(list)
    with path.open(newline='') as stream:
        reader = csv.DictReader(stream, delimiter='\t', quoting=csv.QUOTE_NONE)
        require(reader.fieldnames == ['oracle_id', 'commander_legal', 'kind', 'block', 'code'],
                f'{path}: unexpected blocker header')
        for row in reader:
            require(row['commander_legal'] in ('true', 'false') and row['kind'] in ('card', 'block', 'binding'),
                    f"{path}: invalid blocker row for {row['oracle_id']}")
            block = None if row['block'] == '-' else int(row['block'])
            observations[row['oracle_id']].append((row['code'], row['kind'], block))
    return observations


def measure_coverage(archive, receipts, slots):
    files = [receipts / filename for filename in ('summary.json', 'records.tsv', 'blockers.tsv')]
    summary = read_json(files[0])
    records = read_records(files[1])
    observations = read_observations(files[2])
    archive_digest = sha256(archive)
    require(summary['archive_sha256'] == archive_digest, 'coverage archive SHA-256 mismatch')
    counts = Counter(status for status, _ in records.values())
    require(sum(counts.values()) == summary['total'], f'coverage total mismatch: {counts}')
    require(observations.keys() <= records.keys(), 'coverage blockers contain unknown identities')

    legal = set()
    seen = set()
    phases = {phase: Counter() for phase in PHASES}
    subclasses = {label: Counter() for label in OTHER_CLASSES}
    supported = {oid for oid, (status, _) in records.items() if status == SUPPORTED}
    outcomes = dict.fromkeys(supported, SUPPORTED)
    with archive.open('rb') as stream:
        header = json.loads(next(stream))
        for line in stream:
            row = json.loads(line)
            oid = row['oracleIdentity']
            require(oid not in seen and oid in records, f'coverage/archive identity mismatch: {oid}')
            seen.add(oid)
            legal_card = row['source']['legalities']['commander'] == 'legal'
            if legal_card:
                legal.add(oid)
            if oid in supported:
                continue
            status, reason = records[oid]
            if status == 'invalid':
                outcomes[oid] = 'invalid'
                continue
            residuals = archive_residuals(row)
            phase = outcomes[oid] = rejection_phase(reason, residuals)
            phases[phase].update(archive_cards=1, legal_cards=int(legal_card), rankable_slots=slots[oid])
            if (legal_card and phase not in ('read', 'admission')
                    and (label := sole_other(row, observations[oid], reason, residuals))):
                subclasses[label].update(legal_cards=1, rankable_slots=slots[oid])

    require(seen == records.keys(), 'coverage is missing archive identities or has extra rows')
    legal_supported = legal & supported
    require((len(legal), len(legal_supported)) == (summary['commander_legal'], summary['lowered_validated']),
            'coverage Commander legality or lowered count differs from the archive join')
    # Sweep 5's summary status histogram covers legal cards. Its records.tsv
    # (and the bulk index verified separately) covers all archive identities.
    legal_counts = Counter(records[oid][0] for oid in legal)
    require(legal_counts == Counter(summary['statuses']), 'coverage legal status histogram mismatch')
    phases['read']['rankable_slots'] += sum(slots[oid] for oid in slots.keys() - seen)
    other = {'legal_cards': sum(row['legal_cards'] for row in subclasses.values()),
             'rankable_slots': sum(row['rankable_slots'] for row in subclasses.values()),
             'subclasses': {name: {key: row[key] for key in ('legal_cards', 'rankable_slots')}
                            for name, row in subclasses.items()}}
    report = {
        'archive_sha256': archive_digest,
        'coverage_sha256': {p.name: sha256(p) for p in files},
        'archive_entries': len(seen),
        'statuses': {key: counts[key] for key in (SUPPORTED, 'unparsed', 'unsupported', 'invalid')},
        'commander_legal': len(legal),
        'commander_legal_supported': len(legal_supported),
        'rejections': {name: {key: row[key] for key in ('archive_cards', 'legal_cards', 'rankable_slots')}
                       for name, row in phases.items()},
        'other': other,
    }
    corpus = (header['corpus']['sha256'], header['paperEvidence']['sha256'], header['rules'], header['scope'])
    return Measurement(report, legal, supported, corpus, outcomes), records


def invoke(command, output):
    # Keep stdout/stderr for failures and successor audits.
    with output.open('xb') as stdout, output.with_suffix('.stderr').open('xb') as stderr:
        subprocess.run([str(part) for part in command], stdout=stdout, stderr=stderr, check=True)
    return read_json(output)


def verify_bulk(bulk, report, records, legal):
    manifest_path = bulk / 'manifest.json'
    manifest = read_json(manifest_path)
    require(manifest['archive_sha256'] == report['archive_sha256'], 'bulk archive SHA-256 mismatch')
    for stem in ('catalog', 'index'):
        payload = bulk / f'{stem}.json'
        require(sha256(payload) == manifest[stem]['sha256'] and payload.stat().st_size == manifest[stem]['bytes'],
                f'bulk {stem} payload mismatch')
    index = read_json(bulk / 'index.json')['rows']
    require(len(index) == len(records) and {row['oracle_id'] for row in index} == records.keys(),
            'bulk index/coverage identity mismatch')
    require(all(records[row['oracle_id']] == (row['status'], row['reason_code'] or '')
                and (row['facts']['commander_legality'] == 'legal') == (row['oracle_id'] in legal)
                for row in index), 'bulk index disagrees with coverage or Commander legality')
    return {'manifest_sha256': sha256(manifest_path), 'catalog_digest': manifest['catalog_digest'],
            'producer_build_id': manifest['producer_build_id']}


def run_admission(sim, bulk, artifact, decks, out):
    rows = []
    commands = []
    for start in range(0, len(decks), 50):
        end = min(start + 50, len(decks))
        batch = [dict(id=index, name=deck['name'], cards=deck['cards'])
                 for index, (_, deck) in enumerate(decks[start:end], start + 1)]
        request = out / f'admission-input-{start + 1:03}-{end:03}.json'
        write_json(request, {'deckCount': len(batch), 'decks': batch})
        cmd = [str(sim), 'deck-inspect', '--artifact', str(bulk), '--manifest-sha',
               artifact['manifest_sha256'], '--decks', str(request)]
        commands.append(cmd)
        inspected = invoke(cmd, out / f'admission-result-{start + 1:03}-{end:03}.json')
        require(inspected['bulk_manifest_sha256'] == artifact['manifest_sha256']
                and inspected['catalog_digest'] == artifact['catalog_digest'], 'native admission artifact mismatch')
        rows.extend(inspected['decks'])
    path = out / 'admission.json'
    write_json(path, {**artifact, 'decks': rows, 'commands': commands})
    return path


def admission_report(path, artifact, decks, supported):
    receipt = read_json(path)
    require(all(receipt[key] == artifact[key] for key in ('manifest_sha256', 'catalog_digest')),
            'supplied admission receipt has a different artifact identity')
    rows = {}
    for index, ((pool, deck), row) in enumerate(zip(decks, receipt['decks'], strict=True), 1):
        slots = deck_slots(deck)
        total = sum(slots.values())
        status = 'rejected' if row['deck_issues'] or row['card_gaps'] else 'admitted'
        require((row['id'], row['name'], row['compiled_slots'], row['total_slots'], row['status'])
                == (index, deck['name'], total - blocked_slots(slots, supported), total, status),
                f'native admission disagrees with coverage for inspector deck {index}')
        rows[pool, deck['id']] = row
    return rows


def qualification_report(path, artifact, active, admitted):
    receipt = read_json(path)
    require(receipt['manifest_sha256'] == artifact['manifest_sha256']
            and SHA.fullmatch(receipt['configuration_sha256']), 'seeded qualification identity mismatch')
    seen = set()
    complete_decks = set()
    outcomes = Counter()
    for row in receipt['games']:
        key = row['deck_id'], row['seed']
        require(key not in seen and key[0] in active and key[0] in admitted
                and type(key[1]) is int and key[1] >= 0
                and row['status'] in ('complete', 'censored', 'failed'), 'invalid seeded qualification row')
        seen.add(key)
        outcomes[row['status']] += 1
        if row['status'] == 'complete':
            complete_decks.add(key[0])
    return {'receipt_sha256': sha256(path), 'configuration_sha256': receipt['configuration_sha256'],
            'decks_with_complete_seeded_game': len(complete_decks),
            'deck_game_outcomes': {key: outcomes[key] for key in ('complete', 'censored', 'failed')}}


def measure_run(options, archive, pools, cohorts, out):
    out.mkdir()
    sim, receipts, bulk, admission, source, qualification = options
    native_decks = rankable_decks(pools)
    slots = Counter()
    for _, deck in native_decks:
        slots.update(deck_slots(deck))
    if not receipts:
        receipts = out / 'coverage'
        invoke([sim, 'catalog', 'coverage', archive, '--out', receipts], out / 'coverage.json')
    measured, records = measure_coverage(archive, receipts, slots)
    report, legal, supported = measured.report, measured.legal, measured.supported
    if sim and not bulk:
        bulk = out / 'bulk'
        invoke([sim, 'catalog', 'build-ir', archive, '--out', bulk], out / 'bulk-build.json')
    artifact = verify_bulk(bulk, report, records, legal) if bulk else None
    if sim and not admission:
        admission = run_admission(sim, bulk, artifact, native_decks, out)
    native = admission_report(admission, artifact, native_decks, supported) if admission else None
    active_admitted = set()
    report['pools'] = {}
    for pool, decks in pools.items():
        rankable = pool not in (HELD, HELD_CONTROL)
        scores = pool_report(decks, supported, rankable)
        if rankable:
            inspected = CONTROL if pool == TRAIN and CONTROL in pools else pool
            admitted = {d['id'] for d in decks if native is not None
                        and native[inspected, d['id']]['status'] == 'admitted'}
            scores['native_headless_admitted'] = len(admitted) if native is not None else None
            if native is not None:
                selected = [native[inspected, deck['id']] for deck in decks]
                scores['native_rejected'] = len(decks) - len(admitted)
                scores['with_noncompiler_admission_issue'] = sum(
                    bool(row['deck_issues'] or any(issue['code'] not in COMPILE_GAPS
                         for gap in row['card_gaps'] for issue in gap['issues'])) for row in selected)
            if pool == TRAIN:
                active_admitted = admitted
        report['pools'][pool] = scores

    report['cohorts'] = {}
    for name, members in cohorts.items():
        require(members <= legal, f'{name} cohort contains identities outside the legal archive')
        lowered = members & supported
        report['cohorts'][name] = {'cards': len(members), 'supported_cards': len(lowered),
                                   'rankable_slots': sum(slots[oid] for oid in members),
                                   'supported_rankable_slots': sum(slots[oid] for oid in lowered)}
    unresolved = cohorts['ambiguity-311'] - supported
    active = {d['id']: deck_slots(d) for d in pools[TRAIN]}
    blocked = [deck.keys() & unresolved for deck in active.values()]
    report['ambiguity_union'] = {'unresolved_cards': len(unresolved),
                                 'active_training_identities': len(set().union(*blocked)),
                                 'active_training_decks': sum(bool(ids) for ids in blocked)}
    report.update(
        artifact=artifact,
        simulator_sha256=sha256(sim) if sim else None,
        coverage_generated=options.coverage is None,
        admission_sha256=sha256(admission) if admission else None,
        qualification=qualification_report(qualification, artifact, active, active_admitted) if qualification else None,
        guardrails=guardrails(source / 'packages/game-engine/crates'),
    )
    return measured


def compare(baseline, candidate):
    before, legal, old_supported, old_corpus, old_outcomes = baseline
    after, new_legal, new_supported, new_corpus, new_outcomes = candidate
    require(old_corpus == new_corpus and legal == new_legal and old_outcomes.keys() == new_outcomes.keys(),
            'fixed comparison requires identical corpus, paper evidence, rules, scope and legality')
    gains = new_supported - old_supported
    losses = old_supported - new_supported
    transitions = Counter((old_outcomes[oid], new_outcomes[oid]) for oid in legal)
    pool_deltas = {}
    for pool, scores in after['pools'].items():
        old = before['pools'][pool]
        delta = {key: value - old[key] for key, value in scores.items()
                 if key in ('compiled_slots', 'slot_complete', 'at_least_90_percent', 'native_headless_admitted')
                 and value is not None and old[key] is not None}
        pool_deltas[pool] = delta
    guardrail_deltas = {key: after['guardrails'][key]['count'] - before['guardrails'][key]['count']
                        for key in ('kernel_cells', 'mirrored_shape_guards', 'routing_ladder_pairs')}
    variant_deltas = {}
    for group, data in after['guardrails']['mechanic_named_variants'].items():
        old = before['guardrails']['mechanic_named_variants'][group]
        variant_deltas[group] = {
            'count_delta': data['count'] - old['count'],
            'added': sorted(set(data['names']) - set(old['names'])),
            'removed': sorted(set(old['names']) - set(data['names'])),
        }
    return {
        'whole_archive_gains': len(gains), 'whole_archive_losses': len(losses),
        'legal_gains': len(legal & gains), 'legal_losses': len(legal & losses),
        'legal_gained_ids': sorted(legal & gains), 'legal_lost_ids': sorted(legal & losses),
        'legal_phase_transitions': [{'before': a, 'after': b, 'cards': n}
                                    for (a, b), n in sorted(transitions.items())],
        'pool_deltas': pool_deltas, 'guardrail_count_deltas': guardrail_deltas,
        'mechanic_named_variant_deltas': variant_deltas,
    }


def table(header, rows):
    return ['| ' + ' | '.join(map(str, line)) + ' |' for line in (header, ['---'] * len(header), *rows)]


def display(number):
    return 'Unmeasured' if number is None else f'{number:,}'


def markdown(report):
    lines = ['# Engine rework sweep', '',
             'Each run uses the same deck snapshots and fixed cohort membership. '
             'The JSON report identifies the archive, coverage receipts, Rust source, and any bulk and admission artifacts.',
             '', 'Phase counts use archive residuals first, then a tagged phase or the primary Rust reason. '
             'Invalid records are counted separately as archive outcomes. '
             'The read slots include rankable slots whose identities are absent from the archive. '
             'The admission phase includes tagged card outcomes. Native deck inspection is measured separately.', '']
    if delta := report.get('comparison'):
        lines.extend(['## Candidate changes', '',
                      f"Whole cards gained {delta['whole_archive_gains']:,} and lost {delta['whole_archive_losses']:,} "
                      f"across the archive. Commander-legal cards gained {delta['legal_gains']:,} and lost {delta['legal_losses']:,}.", '',
                      *table(['Pool', 'Compiled slot change', 'Slot-complete deck change', 'Native admission change'],
                             [(pool, f"{v['compiled_slots']:+,}", f"{v['slot_complete']:+,}" if 'slot_complete' in v else '—',
                               f"{v['native_headless_admitted']:+,}" if 'native_headless_admitted' in v else '—')
                              for pool, v in delta['pool_deltas'].items()]), ''])
        guardrail_changes = [(key.replace('_', ' '), change) for key, change in delta['guardrail_count_deltas'].items()
                             if change]
        guardrail_changes.extend((f'Mechanic-named {group}', change['count_delta'])
                                 for group, change in delta['mechanic_named_variant_deltas'].items()
                                 if change['count_delta'])
        if guardrail_changes:
            lines.extend([*table(['Rust guardrail', 'Change'], [(key, f'{n:+d}') for key, n in guardrail_changes]), ''])
        phase_changes = [(row['before'], row['after'], display(row['cards']))
                         for row in delta['legal_phase_transitions'] if row['before'] != row['after']]
        if phase_changes:
            lines.extend([*table(['Baseline phase or outcome', 'Candidate phase or outcome', 'Commander-legal cards'],
                                 phase_changes), ''])
    rankable = report['rankable_set']
    for name, run in report['runs'].items():
        other = run['other']
        lines.extend([f'## {name.title()}', '',
                      f"{run['commander_legal_supported']:,} / {run['commander_legal']:,} Commander-legal cards compile, "
                      f"with {run['archive_entries']:,} total archive entries.", '',
                      *table(['Archive outcome', 'Cards'],
                             [(status, f'{count:,}') for status, count in run['statuses'].items()]), '',
                      *table(['Pool', 'Compiled slots', 'Decks at 90%+', 'Slot-complete', 'Native headless admissions',
                              'Native rejections', 'Fewest blocked', 'Median blocked'],
                             [(pool, f"{p['compiled_slots']:,} / {p['total_slots']:,}", p.get('at_least_90_percent', '—'),
                               f"{p['slot_complete']} / {p['decks']}" if 'decks' in p else '—',
                               display(p['native_headless_admitted']) if 'native_headless_admitted' in p else '—',
                               p.get('native_rejected', '—'), p.get('min_blocked', '—'), p.get('median_blocked', '—'))
                              for pool, p in run['pools'].items()]), '',
                      'Blocked-slot distributions by unchanged deck, in blocked slots × decks.', ''])
        for pool, scores in run['pools'].items():
            if 'blocked_slot_distribution' in scores:
                distribution = ', '.join(f'{slots} × {count}'
                                         for slots, count in scores['blocked_slot_distribution'].items())
                lines.append(f'- {pool} — {distribution}')
        lines.extend(['', f'Rankable slot counts below use {rankable}.', '',
                      *table(['Rejection phase', 'Archive cards', 'Commander-legal cards', 'Rankable slots'],
                             [(phase, f"{v['archive_cards']:,}", f"{v['legal_cards']:,}", f"{v['rankable_slots']:,}")
                              for phase, v in run['rejections'].items()]), '',
                      *table(['Diagnostic family', 'Commander-legal cards', 'Rankable slots'],
                             [(UNASSIGNED, f"{other['legal_cards']:,}", f"{other['rankable_slots']:,}"),
                              *((label, f"{v['legal_cards']:,}", f"{v['rankable_slots']:,}")
                                for label, v in other['subclasses'].items())]), '',
                      *table(['Fixed cohort', 'Supported cards', 'Supported rankable slots'],
                             [(cohort, f"{v['supported_cards']:,} / {v['cards']:,}",
                               f"{v['supported_rankable_slots']:,} / {v['rankable_slots']:,}")
                              for cohort, v in run['cohorts'].items()]), ''])
        ambiguity = run['ambiguity_union']
        lines.extend([f"Ambiguity remains on {ambiguity['unresolved_cards']} frozen identities. "
                      f"{ambiguity['active_training_identities']} appear across "
                      f"{ambiguity['active_training_decks']} active training decks.", ''])
        if qualification := run['qualification']:
            outcomes = qualification['deck_game_outcomes']
            lines.extend([f"{qualification['decks_with_complete_seeded_game']} active training decks have a complete "
                          f"seeded game. Across deck appearances, {outcomes['complete']} completed, "
                          f"{outcomes['censored']} were censored, and {outcomes['failed']} failed.", ''])
        else:
            lines.extend(['Seeded complete-game qualification was unmeasured.', ''])
        metrics = run['guardrails']
        lines.extend([*table(['Rust guardrail', 'Count'],
                             [('Audited kernel cells', metrics['kernel_cells']['count']),
                              ('Compiler mirror rejection sites', metrics['mirrored_shape_guards']['count']),
                              ('Routing-ladder recognizer pairs', metrics['routing_ladder_pairs']['count']),
                              *((f'Mechanic-named {group}', data['count'])
                                for group, data in metrics['mechanic_named_variants'].items())]), ''])
    return '\n'.join(lines)


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ('archive', 'pool-a', 'pool-b', 'cohorts', 'out'):
        parser.add_argument(f'--{name}', type=Path, required=True)
    parser.add_argument('--pool-b-control', type=Path, help='original 161/45 Sweep 5 snapshot')
    parser.add_argument('--candidate-archive', type=Path, help='alternate archive of the same frozen corpus and rules')
    for prefix in ('baseline', 'candidate'):
        parser.add_argument(f'--{prefix}-sim', type=Path, help='mtg-sim binary')
        parser.add_argument(f'--{prefix}-coverage', type=Path, help='directory containing the whole-archive receipts')
        parser.add_argument(f'--{prefix}-bulk', type=Path, help='directory containing the bulk manifest, index and catalog')
        parser.add_argument(f'--{prefix}-admission', type=Path, help='admission.json bound to the bulk artifact')
        parser.add_argument(f'--{prefix}-source', type=Path, help='checkout root for the simulator Rust source')
        parser.add_argument(f'--{prefix}-qualification', type=Path, help='optional complete-game receipt')
    args = parser.parse_args(argv)
    for key, value in vars(args).items():
        if isinstance(value, Path):
            setattr(args, key, value.absolute())
    return args


def run_options(args, name):
    options = [getattr(args, f'{name}_{key}') for key in ('sim', 'coverage', 'bulk', 'admission', 'source', 'qualification')]
    sim, coverage, bulk, admission, source, qualification = options
    if name == 'candidate' and not any(options) and not args.candidate_archive:
        return None
    require((sim or coverage) and source, f'{name} requires a simulator or coverage receipts, and its source checkout')
    require(not admission or bulk, f'{name} admission receipt requires its bulk artifact')
    require(not qualification or sim or admission, f'{name} qualification requires native admission evidence')
    return RunInputs(*options)


def main(argv=None):
    args = parse_args(argv)
    try:
        baseline_options = run_options(args, 'baseline')
        candidate_options = run_options(args, 'candidate')
        pools = load_pools(args.pool_a, args.pool_b, args.pool_b_control)
        cohorts, pins = load_cohorts(args.cohorts)
        out = new_directory(args.out)
        baseline = measure_run(baseline_options, args.archive, pools, cohorts, out / 'baseline')
        report = {'format_version': 1,
                  'input_sha256': {'pool_a': sha256(args.pool_a), 'pool_b': sha256(args.pool_b),
                                   'pool_b_control': sha256(args.pool_b_control) if args.pool_b_control else None,
                                   'cohorts': pins},
                  'rankable_set': '176 Sweep 5 decks (15 Pool A and 161 Pool B training)' if CONTROL in pools
                                  else '172 active decks (15 Pool A and 157 Pool B training)',
                  'runs': {'baseline': baseline.report}}
        if candidate_options:
            candidate = measure_run(candidate_options, args.candidate_archive or args.archive, pools, cohorts, out / 'candidate')
            report['comparison'] = compare(baseline, candidate)
            report['runs']['candidate'] = candidate.report
        write_json(out / 'report.json', report)
        (out / 'report.md').write_text(markdown(report))
        print(out / 'report.md')
        invalid = [f"{name} ({run['statuses']['invalid']:,})"
                   for name, run in report['runs'].items() if run['statuses']['invalid']]
        require(not invalid, f"invalid IR records in {', '.join(invalid)}")
        return 0
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f'rework-sweep: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
