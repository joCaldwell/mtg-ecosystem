#!/usr/bin/env python3
"""Transition differential for reference selections and lowered IR.

Run via sh scripts/engine-env.sh python3 -B scripts/rework-reference-diff.py --help.

The Rust antecedent rule is the only source of meaning. While the unchanged
R5 archive is the input, this sweep-time check compares every antecedent the
rule chose (`mtg-sim catalog reference-trace`) with the frozen TypeScript
resolution in the archive, over every lowered definition, and exits 1 on
any unexplained disagreement. It also compares each baseline definition's
Kernel IR with the candidate's and lists every change for review.

For D3 source-authority choices, join each traced selection (family, block
and its block-relative path, kind and slot/selection/domain or cardinality)
to every original binder reference and projection in the claimed block. A
missing, additional, or different occurrence stops the sweep.

Explained equivalence: an event object that is the source itself ("when
this creature enters, it ...") reads the same incarnation the resolution
calls the source (CR 201.5, 603.10a).

Explained decision: the binder leaves "each other player" (a filtered
each-player with `other`) unresolved as `unsupported-scope`. Rust decides it
as each player but the controller (CR 102.2-102.3); the occurrence must still
be exactly that unresolved reference and projection, and its each-player base
must match the binder's bound selection. A linked result ("discarded this
way") matches the binder's event-link node for the producing step (CR 608.2c).
"""
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import sys

PRONOUNS = ('it', 'that')


def pointer(value, path):
    for part in path.strip('/').split('/') if path.strip('/') else []:
        if isinstance(value, list):
            if not part.isdigit() or int(part) >= len(value):
                return None
            value = value[int(part)]
        elif isinstance(value, dict):
            if part not in value:
                return None
            value = value[part]
        else:
            return None
    return value


def frozen_sites(record):
    """(block, word) -> normalized frozen antecedents, keyed by the card-wide
    block index, and the same sites keyed by their face-local index."""
    sites = defaultdict(set)
    local = defaultdict(lambda: defaultdict(set))
    resolution = record.get('resolution') or {}
    offset = 0
    for face_index, face in enumerate(resolution.get('faces', [])):
        blocks = face.get('blocks') or []
        abilities = ((record['faces'][face_index].get('result') or {}).get('abilities')
                     if face_index < len(record['faces']) else None) or []
        for index, block in enumerate(blocks):
            ability = abilities[index] if index < len(abilities) else {}
            selections = {}
            for entry in block.get('refs', []):
                ref = entry.get('ref') or {}
                operand = pointer(ability, entry['path'])
                if ref.get('ref') == 'target' and isinstance(operand, dict) \
                        and operand.get('selectionId'):
                    selections.setdefault(ref.get('slot'), operand['selectionId'])
            for entry in block.get('refs', []):
                operand = pointer(ability, entry['path'])
                if not isinstance(operand, dict) or operand.get('ref') not in PRONOUNS:
                    continue
                ref = entry.get('ref')
                if ref is None:
                    value = 'unresolved:' + json.dumps(
                        {k: v for k, v in entry.items() if k != 'path'}, sort_keys=True)
                elif ref.get('ref') == 'target':
                    value = 'target:' + str(selections.get(ref.get('slot'), ref.get('slot')))
                elif ref.get('ref') == 'source':
                    value = 'source'
                elif ref.get('ref') == 'event-object':
                    value = 'event'
                else:
                    value = json.dumps(ref, sort_keys=True)
                sites[(offset + index, operand['ref'])].add(value)
                local[face_index][(index, operand['ref'])].add(value)
        offset += len(blocks)
    return sites, local


def align(compiled, frozen, local):
    """Rekey compiled sites the lowerer numbered within a later face (an
    adventure or split half compiles its own program) onto card-wide keys."""
    aligned = defaultdict(set)
    for key, values in compiled.items():
        if key not in frozen:
            owners = [face for face, sites in local.items() if face > 0 and key in sites]
            if len(owners) == 1:
                candidates = [k for k in frozen if k[1] == key[1] and k not in compiled
                              and frozen[k] == local[owners[0]][key]]
                if len(candidates) == 1:
                    key = candidates[0]
        aligned[key] |= values
    return aligned


def compiled_sites(bound):
    sites = defaultdict(set)
    for entry in bound:
        resolved = entry['resolved']
        kind = resolved['ref']
        if kind == 'target':
            value = 'target:' + resolved['selection']
        elif kind == 'event-object':
            value = 'event-self' if resolved['is_source'] else 'event'
        elif kind == 'source':
            value = 'source'
        else:
            value = 'other:' + resolved['reference']
        sites[(entry['block'], entry['word'])].add(value)
    return sites


def agrees(compiled, frozen):
    def matches(c, f):
        return c == f or (c == 'event-self' and f in ('event', 'source'))
    return all(any(matches(c, f) for f in frozen) for c in compiled) and \
        all(any(matches(c, f) for c in compiled) for f in frozen)


def read_archive(path, wanted):
    records = {}
    with path.open() as stream:
        next(stream)
        for line in stream:
            start = line.find('"oracleIdentity":"')
            if start < 0:
                continue
            identity = line[start + 18:start + 54]
            if identity in wanted:
                records[identity] = json.loads(line)
    return records


def compare_references(archive, trace, unchanged):
    rows = [json.loads(line) for line in trace.read_text().splitlines() if line]
    records = read_archive(archive, {row['oracle_id'] for row in rows})
    counts = Counter()
    disagreements = []
    for row in rows:
        record = records[row['oracle_id']]
        frozen, local = frozen_sites(record)
        compiled = align(compiled_sites(row['bound']), frozen, local)
        if compiled or frozen:
            counts['definitions_with_pronouns'] += 1
        for key in sorted(set(compiled) | set(frozen)):
            c, f = compiled.get(key, set()), frozen.get(key, set())
            counts['sites'] += 1
            if c and f and agrees(c, f):
                counts['agree'] += 1
                if 'event-self' in c and f != {'event'}:
                    counts['explained_self_event'] += 1
                continue
            kind = 'frozen_only' if not c else 'compiled_only' if not f else 'mismatch'
            counts[kind] += 1
            # A pronoun a fixed-shape recognizer lowers, outside the antecedent
            # rule, in a definition whose IR equals the frozen baseline's.
            baseline = kind == 'frozen_only' and row['oracle_id'] in unchanged
            if baseline:
                counts['explained_unchanged_recognizer'] += 1
            disagreements.append({
                'oracle_id': row['oracle_id'],
                'name': record['source']['name'],
                'block': key[0],
                'word': key[1],
                'kind': kind,
                'explanation': 'fixed-shape recognizer; IR identical to the frozen baseline'
                if baseline else None,
                'compiled': sorted(c),
                'frozen': sorted(f),
                'oracle_text': record['source'].get('oracle_text')
                or ' // '.join(face.get('oracle_text', '') for face in record['source'].get('card_faces', [])),
            })
    return {'lowered_definitions': len(rows), **dict(sorted(counts.items())),
            'disagreements': disagreements}


def source_sites(choice):
    """Each traced selection with the binder node and projection it must match."""
    for selection in choice['selections']:
        kind = selection['kind']
        match kind:
            case 'spell-controller':
                node = {'node': 'runtime-player', 'role': 'spell-controller'}
                projected = {'ref': 'controller', 'role': 'spell-controller'}
                rule = '109.5'
            case 'each-player' | 'each-opponent':
                node = {'node': 'selection', 'selection': 'evaluate-reference',
                        'timing': 'resolution', 'cardinality': 'many',
                        'expression': {'player': kind}}
                projected = None
                rule = '608.2h'
            case 'target':
                noun = {'any_player': 'target-player', 'opponent_player': 'target-opponent'}[selection['domain']]
                node = {'node': 'selection', 'selection': 'target',
                        'timing': 'stack-announcement', 'cardinality': 'one',
                        'expression': {'player': noun, 'selectionId': selection['selection_id']}}
                projected = {'ref': 'target', 'slot': selection['slot']}
                rule = '601.2c'
            case 'each-other-player':
                node = None
                projected = None
                rule = '102.3'
            case 'linked-result':
                node = {'node': 'event-link', 'action': 'discard', 'step': selection['step']}
                projected = None
                rule = '608.2c'
            case 'announced-x':
                node = {'node': 'announced-number', 'announcement': 'cast'}
                projected = {'ref': 'announced-x'}
                rule = '107.3a'
            case 'resolution-cards':
                # The binder records no random flag; the occurrence is the
                # recipient's resolution-time selection either way.
                node = {'node': 'selection', 'selection': 'evaluate-reference',
                        'timing': 'resolution', 'cardinality': selection['cardinality']}
                projected = None
                rule = '701.9b'
            case _:
                raise ValueError(f'unknown source authority selection {selection}')
        yield selection['path'], node, projected, rule


def compare_source_choices(archive, trace):
    """Join every traced selection to the original binder graph and its
    projection over the whole claimed block: a missing, additional or
    different occurrence is a divergence."""
    with trace.open() as stream:
        choices = {row['oracle_id']: row['source_choices']
                   for line in stream if (row := json.loads(line)).get('source_choices')}
    records = read_archive(archive, choices.keys()) if choices else {}
    table = []
    families = Counter()
    for identity, sites in sorted(choices.items()):
        record = records[identity]
        binding = record['binding']
        nodes = {node['id']: node for node in binding['nodes']}
        for site in sites:
            face, block = site['face'], site['block']
            families[site['family']] += 1
            prefix = f'/faces/{face}/abilities/{block}'
            scope = f'card:{identity}{prefix}#ability'
            projection = record['resolution']['faces'][face]['blocks'][block]['refs']
            chosen = {path: (node, projected, rule)
                      for path, node, projected, rule in source_sites(site)}
            historical = defaultdict(list)
            projected = defaultdict(list)
            for ref in binding['references']:
                if ref['path'] == prefix or ref['path'].startswith(prefix + '/'):
                    historical[ref['path'][len(prefix):]].append(ref)
            for ref in projection:
                projected[ref['path']].append(ref)
            kinds = {selection['path']: selection['kind'] for selection in site['selections']}
            links = {node['path'][len(prefix):]: node for node in binding['nodes']
                     if node['node'] == 'event-link' and node['path'].startswith(prefix + '/')}
            for path in sorted(chosen.keys() | historical.keys() | projected.keys() | links.keys()):
                node, projection_ref, rule = chosen.get(path, (None, None, None))
                bound = historical[path]
                target = nodes.get(bound[0].get('target')) if len(bound) == 1 else None
                legacy = projected[path]
                want_projection = ({'path': path, 'ref': projection_ref} if projection_ref
                                   else {'path': path, 'evaluated': 'selection'})
                explanation = None
                if kinds.get(path) == 'each-other-player':
                    agree = (len(bound) == 1 and bound[0]['binding'] == 'unresolved'
                             and bound[0].get('code') == 'unsupported-scope'
                             and bound[0]['expression'] == {'player': 'filtered', 'base': {'player': 'each-player'},
                                                            'filter': {'other': True}}
                             and legacy == [{'path': path, 'unresolved': 'unsupported-scope'}])
                    explanation = 'binder unresolved; Rust decides each player but you (CR 102.2-102.3)'
                elif kinds.get(path) == 'linked-result':
                    link = links.get(path)
                    target = link
                    agree = (link is not None and not bound and not legacy and link.get('scope') == scope
                             and link.get('action') == node['action']
                             and link.get('event') == f'card:{identity}{prefix}{node["step"]}#event')
                else:
                    agree = (node is not None and target is not None and bound[0]['binding'] == 'bound'
                             and target.get('scope') == scope
                             and all(target.get(key) == value for key, value in node.items())
                             and legacy == [want_projection])
                table.append({
                    'oracle_id': identity,
                    'name': record['source']['name'],
                    'family': site['family'],
                    'oracle_text': record['faces'][face]['result']['lines'][block]['text'],
                    'path': prefix + path,
                    'rule': rule,
                    'rust': node,
                    'historical': bound,
                    'historical_node': target,
                    'projection': legacy,
                    'agrees': agree,
                    'explanation': explanation if agree else None,
                })
    by_family = {}
    for family in sorted(families):
        rows = [row for row in table if row['family'] == family]
        by_family[family] = {'identities': len({row['oracle_id'] for row in rows}), 'sites': len(rows),
                             'divergences': sum(not row['agrees'] for row in rows),
                             'explained_decisions': sum(bool(row['explanation']) for row in rows)}
    return {'identities': len(choices), 'sites': len(table),
            'historical_bound': sum(ref['binding'] == 'bound' for row in table for ref in row['historical']),
            'divergences': sum(not row['agrees'] for row in table),
            'explained_decisions': sum(bool(row['explanation']) for row in table), 'families': by_family,
            'occurrences': table}


def diff_paths(a, b, path='', out=None, limit=12):
    out = [] if out is None else out
    if len(out) >= limit:
        return out
    if isinstance(a, dict) and isinstance(b, dict):
        for key in sorted(set(a) | set(b)):
            if a.get(key) != b.get(key):
                diff_paths(a.get(key), b.get(key), f'{path}/{key}', out, limit)
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for index, (x, y) in enumerate(zip(a, b)):
            if x != y:
                diff_paths(x, y, f'{path}/{index}', out, limit)
    else:
        out.append({'path': path, 'before': a, 'after': b})
    return out


def compare_kir(baseline, candidate):
    def load(path):
        catalog = json.loads(path.read_bytes())
        return {d['oracle_id']: d for d in catalog['definitions']}
    before, after = load(baseline), load(candidate)
    changed = []
    for identity in sorted(before.keys() & after.keys()):
        if before[identity] != after[identity]:
            changed.append({'oracle_id': identity, 'name': before[identity].get('name'),
                            'differences': diff_paths(before[identity], after[identity])})
    shared = before.keys() & after.keys()
    changed_ids = {row['oracle_id'] for row in changed}
    return {
        'unchanged_ids': shared - changed_ids,
        'baseline_definitions': len(before),
        'candidate_definitions': len(after),
        'unchanged': len(before.keys() & after.keys()) - len(changed),
        'changed': len(changed),
        'lost': sorted(before.keys() - after.keys()),
        'gained': len(after.keys() - before.keys()),
        'changed_definitions': changed,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--trace', type=Path, required=True, help='reference-trace JSONL')
    parser.add_argument('--baseline-catalog', type=Path, required=True)
    parser.add_argument('--candidate-catalog', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True, help='existing output directory')
    parser.add_argument('--explained', type=Path,
                        help='JSON list of reviewed {oracle_id, block, word, reason} disagreements '
                        'and {oracle_id, lost: true, reason} lost definitions')
    args = parser.parse_args(argv)
    kir = compare_kir(args.baseline_catalog, args.candidate_catalog)
    references = compare_references(args.archive, args.trace, kir.pop('unchanged_ids'))
    source = compare_source_choices(args.archive, args.trace)
    reviewed = json.loads(args.explained.read_text()) if args.explained else []
    explained = {(e['oracle_id'], e['block'], e['word']): e['reason']
                 for e in reviewed if 'block' in e}
    lost_reasons = {e['oracle_id']: e['reason'] for e in reviewed if e.get('lost')}
    kir['lost'] = [{'oracle_id': identity, 'explanation': lost_reasons.get(identity)}
                   for identity in kir['lost']]
    for row in references['disagreements']:
        row['explanation'] = row['explanation'] or explained.get(
            (row['oracle_id'], row['block'], row['word']))
    unexplained = [row for row in references['disagreements'] if not row['explanation']]
    references['unexplained'] = len(unexplained)
    (args.out / 'reference-parity.json').write_text(json.dumps(references, indent=2) + '\n')
    (args.out / 'kir-parity.json').write_text(json.dumps(kir, indent=2) + '\n')
    if source['sites']:
        (args.out / 'source-resolution-parity.json').write_text(json.dumps(source, indent=2) + '\n')
    summary = {k: v for k, v in references.items() if k != 'disagreements'}
    summary['kir'] = {k: (len(v) if isinstance(v, list) else v)
                      for k, v in kir.items() if k != 'changed_definitions'}
    summary['source_resolution'] = {k: v for k, v in source.items() if k != 'occurrences'}
    print(json.dumps(summary))
    return 1 if unexplained or source['divergences'] or any(
        not row['explanation'] for row in kir['lost']) else 0


if __name__ == '__main__':
    sys.exit(main())
