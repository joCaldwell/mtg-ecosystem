#!/usr/bin/env python3
"""D1/D2 archive comparison. Each run supplies bulk/, coverage/, tiers.json and admission.json."""

import argparse
import csv
import hashlib
import json
from itertools import zip_longest
from pathlib import Path


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def json_bytes(value):
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode()


def payload(path):
    raw = path.read_bytes()
    return raw, json.loads(raw)


def changed_rows(before, after, manifests, indexes):
    """Recompute row proofs directly from both streams and verify the append-only cut."""
    index = []
    hashes = [hashlib.sha256(), hashlib.sha256()]
    for bundle in indexes:
        rows = bundle['rows']
        by_key = {(row['source_index'], row['oracle_id']): row for row in rows}
        require(len(rows) == len(by_key), 'duplicate source index and identity in bulk index')
        index.append(by_key)
    changed = set()
    with before.open('rb') as left, after.open('rb') as right:
        for stream, manifest, digest, version in zip((left, right), manifests, hashes, (4, 5), strict=True):
            header = stream.readline()
            require(header.endswith(b'\n') and json.loads(header)['schemaVersion'] == version,
                    'D2 requires schema 4 → schema 5')
            require(sha(header[:-1]) == manifest['archive_manifest_sha256'], 'archive header proof mismatch')
            parser = json.loads(header)['parser']
            require(parser['sourceSha256'] == manifest['parser_source_sha256']
                    and (version == 5 or parser['revision'] == manifest['parser_revision']),
                    'archive parser proof mismatch')
            digest.update(header)

        for source_index, pair in enumerate(zip_longest(left, right)):
            old, new = pair
            require(all(raw and raw.endswith(b'}\n') for raw in pair), f'row {source_index} has no matching JSONL row')
            old_row = json.loads(old)
            key = (old_row['sourceIndex'], old_row['oracleIdentity'])
            require(key[0] == source_index, f'archive source order differs for {key}')
            if old_row['status'] == 'excluded':
                require(old == new, f'excluded row {key} changed')
            else:
                prefix = old[:-2]
                require(new.startswith(prefix + b',"printed":'), f'legacy prefix differs for {key}')
                suffix = json.loads(b'{' + new[len(prefix) + 1:])
                require(list(suffix) == ['printed', 'printedSha256'], f'R6 append fields differ for {key}')
                require(new == prefix + b',' + json_bytes(suffix)[1:] + b'\n'
                        and sha(json_bytes(suffix['printed'])) == suffix['printedSha256'],
                        f'R6 evidence proof differs for {key}')
                changed.add(key)
            for raw, rows, digest in zip(pair, index, hashes, strict=True):
                row = rows.pop(key, None)
                require(row is not None and row['raw_row_sha256'] == sha(raw)
                        and row['record_sha256'] == sha(raw[:-1]), f'archive row proofs disagree for {key}')
                digest.update(raw)
    for rows, manifest, digest in zip(index, manifests, hashes, strict=True):
        require(not rows and digest.hexdigest() == manifest['archive_sha256'], 'archive inventory or file proof differs')
    return changed


def compare(mode, before, after, archives=None):
    runs = (before, after)
    manifest_payloads = [payload(run / 'bulk/manifest.json') for run in runs]
    manifests = [value for _, value in manifest_payloads]
    catalog_payloads = [payload(run / 'bulk/catalog.json') for run in runs]
    index_payloads = [payload(run / 'bulk/index.json') for run in runs]
    for role, payloads in [('catalog', catalog_payloads), ('index', index_payloads)]:
        for manifest, (raw, _) in zip(manifests, payloads, strict=True):
            require(manifest[role] == {'bytes': len(raw), 'sha256': sha(raw)}, f'{role} payload proof differs')
            if role == 'catalog':
                require(manifest['catalog_digest'] == manifest[role]['sha256'], 'catalog digest differs')
    old, new = [json.loads(raw) for raw, _ in manifest_payloads]
    catalogs = [value for _, value in catalog_payloads]
    indexes = [value for _, value in index_payloads]
    archive_fields = ('archive_sha256', 'archive_manifest_sha256', 'parser_revision', 'parser_source_sha256')
    if mode == 'd1':
        for role, payloads in [('catalog', catalog_payloads), ('index', index_payloads)]:
            require(payloads[0][0] == payloads[1][0], f'D1 {role} bytes differ')
        old['producer_build_id'] = new['producer_build_id']
    else:
        require(archives is not None, 'D2 requires both input archives')
        for raw, value in (*catalog_payloads, *index_payloads):
            require(json_bytes(value) == raw, 'comparison cannot reproduce payload bytes')
        changed = changed_rows(*archives, manifests, indexes)
        for definition in catalogs[0]['definitions']:
            face = definition
            while face is not None:
                prov = face['provenance']
                require(prov['archive_manifest_sha256'] == old['archive_manifest_sha256']
                        and prov['parser_sha256'] == old['parser_source_sha256'], 'unexpected R5 definition provenance')
                prov['archive_manifest_sha256'] = new['archive_manifest_sha256']
                prov['parser_sha256'] = new['parser_source_sha256']
                face = face.get('back_face')
        after_rows = {(row['source_index'], row['oracle_id']): row for row in indexes[1]['rows']}
        for row in indexes[0]['rows']:
            key = (row['source_index'], row['oracle_id'])
            if key in changed:
                for field in ('raw_row_sha256', 'record_sha256'):
                    row[field] = after_rows[key][field]
        require(json_bytes(catalogs[0]) == catalog_payloads[1][0], 'D2 definition fields or catalog order differ')
        require(json_bytes(indexes[0]) == index_payloads[1][0], 'D2 index fields or row order differ')
        for field in (*archive_fields, 'catalog_digest'):
            old[field] = new[field]
        for role in ('catalog', 'index'):
            old[role]['sha256'] = new[role]['sha256']
    require(old == new, 'bulk manifest differs outside allowed identities')
    require(json_bytes(old) == manifest_payloads[1][0]
            and len(manifest_payloads[0][0]) == len(manifest_payloads[1][0]), 'bulk manifest bytes differ')
    for filename in ('records.tsv', 'blockers.tsv'):
        require((before / 'coverage' / filename).read_bytes() == (after / 'coverage' / filename).read_bytes(),
                f'{filename} coverage differs')
    summaries = [payload(run / 'coverage/summary.json')[1] for run in runs]
    if mode == 'd2':
        for field in ('archive_sha256', 'archive_manifest_sha256', 'parser_revision'):
            summaries[0][field] = summaries[1][field]
    require(summaries[0] == summaries[1], 'coverage summary differs')
    tier_rows = [payload(run / 'tiers.json')[1] for run in runs]
    ids = [row['oracle_id'] for row in indexes[1]['rows'] if row['status'] == 'lowered_validated']
    require([row[0] for row in tier_rows[1]] == ids and tier_rows[0] == tier_rows[1]
            and all(len(row[1]) == 4 and all(type(decision) is bool for decision in row[1]) for row in tier_rows[1]),
            'direct tier decisions differ or omit lowered identities')
    admissions = [payload(run / 'admission.json')[1] for run in runs]
    for admission, (raw, manifest) in zip(admissions, manifest_payloads, strict=True):
        require(admission.pop('bulk_manifest_sha256') == sha(raw)
                and admission.pop('catalog_digest') == manifest['catalog_digest'], 'native admission artifact proof differs')
    require(admissions[0] == admissions[1] and admissions[1]['decks'], 'native admission witnesses differ or are missing')
    legal = sum(row['facts']['commander_legality'] == 'legal' and row['status'] == 'lowered_validated'
                for row in indexes[1]['rows'])
    require(legal == summaries[1]['lowered_validated'], 'coverage and index legal lowered counts differ')
    with (after / 'coverage/records.tsv').open() as rows:
        require(sum(row['status'] == 'lowered_validated' for row in csv.DictReader(rows, delimiter='\t')) == len(ids),
                'coverage and index total lowered counts differ')
    return {
        'comparison': mode, 'lowered': len(ids), 'commander_legal_lowered': legal,
        'tiers': [sum(row[1][index] for row in tier_rows[1]) for index in range(4)],
        'admission_witnesses': len(admissions[1]['decks']),
        'catalog_sha256': new['catalog']['sha256'], 'index_sha256': new['index']['sha256'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('d1', 'd2'))
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    parser.add_argument('--archives', nargs=2, type=Path, metavar=('R5', 'R6'))
    args = parser.parse_args()
    try:
        print(json.dumps(compare(args.mode, args.before, args.after, args.archives), indent=2))
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
