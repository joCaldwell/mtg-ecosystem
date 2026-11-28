#!/usr/bin/env python3
"""D1/D2 archive comparison and the R6 fixture re-extraction check.

Each D1/D2 run supplies bulk/, coverage/, tiers.json and admission.json.
`fixtures` re-extracts every R5-origin fixture identity from R6 in a scratch
copy, verifies both archives there, and never touches the tracked fixtures."""

import argparse
import csv
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
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


def pretty(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def fixtures(sim, source, out, archives, replacement=None):
    """Regenerate R6 fixtures in OUT and verify both origins without editing the source tree."""
    registry = json.loads((source / 'accepted-archives.json').read_bytes())
    r5 = next(origin for origin in registry if origin['cut'] == 'r5')
    previous_r6 = next(origin for origin in registry if origin['cut'] == 'r6')
    r6, header = replacement or (previous_r6.copy(), (source / previous_r6['manifest_file']).read_bytes())
    r6['record_sha256'] = []
    registry[registry.index(previous_r6)] = r6
    provenance = json.loads((source / 'provenance.json').read_bytes())
    provenance['records'] = [row for row in provenance['records'] if row['archive'] != previous_r6['manifest_file']]
    rows = [row for row in provenance['records'] if row['archive'] == r5['manifest_file']]
    require(rows and {r['record_sha256'] for r in rows} == set(r5['record_sha256'])
            and len({r['oracle_id'] for r in rows}) == len(rows), 'R5 provenance rows and origin pins differ')

    out.mkdir(parents=True)
    pretty(out / 'accepted-archives.json', registry)
    pretty(out / 'provenance.json', provenance)
    (out / r6['manifest_file']).write_bytes(header)
    shutil.copyfile(source / r5['manifest_file'], out / r5['manifest_file'])
    for row in rows:
        (out / row['file']).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / row['file'], out / row['file'])

    def sim_json(*args):
        command, *operands = args
        options = ['--promotion'] if replacement else []
        done = subprocess.run([str(sim.resolve()), 'catalog', command, *options, *map(str, operands)],
                              capture_output=True, check=False)
        require(done.returncode == 0, f'{args[0]} failed: {done.stderr.decode(errors="replace").strip()}')
        return json.loads(done.stdout)

    provenance_path = out / 'provenance.json'
    extracted = sim_json('extract', archives[1], provenance_path, *(row['oracle_id'] for row in rows))
    after = json.loads(provenance_path.read_bytes())
    count = len(provenance['records'])
    require(after['records'][:count] == provenance['records'], 'existing provenance rows changed')
    after['records'][count:] = sorted(after['records'][count:], key=lambda row: row['oracle_id'])
    pretty(provenance_path, after)
    verified = {cut: sim_json('verify', archive, provenance_path) for cut, archive in zip(('r5', 'r6'), archives)}
    require(all(report['verified_records'] == len(rows) for report in verified.values()), 'verify omitted fixture rows')
    new_rows = {row['oracle_id']: row for row in after['records'] if row['archive'] == r6['manifest_file']}
    require(len(new_rows) == len(rows) == len(extracted['extracted']), 'R6 extraction is not one row per R5 identity')
    registry_after = {origin['manifest_sha256']: origin for origin in json.loads((out / 'accepted-archives.json').read_bytes())}
    require(registry_after[r5['manifest_sha256']] == r5, 'R5 origin pins changed')
    require(sorted(registry_after[r6['manifest_sha256']]['record_sha256']) == sorted({row['record_sha256'] for row in new_rows.values()}),
            'R6 origin pins differ from extracted rows')
    kinds = {'excluded': 0, 'extended': 0}
    for old in rows:
        new = new_rows[old['oracle_id']]
        legacy, current = (out / old['file']).read_bytes(), (out / new['file']).read_bytes()
        require(new['file'].startswith('phase1-r6/') and new['archive_line'] == old['archive_line']
                and new['source_sha256'] == old['source_sha256'] and new['ast_sha256'] == old['ast_sha256'],
                f'{old["oracle_id"]}: R6 provenance identity differs')
        require(old['record_sha256'] == sha(legacy.rstrip()) and new['record_sha256'] == sha(current.rstrip()),
                f'{old["oracle_id"]}: fixture bytes differ from their pins')
        if json.loads(legacy)['status'] == 'excluded':
            require(current == legacy, f'{old["oracle_id"]}: excluded row changed')
            kinds['excluded'] += 1
        else:
            prefix = legacy[:-2]
            suffix = json.loads(b'{' + current[len(prefix) + 1:])
            require(current.startswith(prefix + b',"printed":') and list(suffix) == ['printed', 'printedSha256']
                    and current == prefix + b',' + json_bytes(suffix)[1:] + b'\n'
                    and sha(json_bytes(suffix['printed'])) == suffix['printedSha256'],
                    f'{old["oracle_id"]}: R6 fixture is not the R5 row plus evidence')
            kinds['extended'] += 1
    return {'comparison': 'fixtures', 'identities': len(rows), **kinds,
            'verified': {cut: report['verified_records'] for cut, report in verified.items()}}


def verify_promotion_d0(root, r5, candidate, grammar):
    done = subprocess.run(
        ['sh', root / 'scripts/engine-env.sh', 'node', root / 'scripts/archive-parser.ts',
         '--legacy-baseline', r5.resolve(), '--verify-candidate', candidate.resolve(),
         *(['--legacy-rows', 'changed'] if grammar else [])],
        cwd=root, capture_output=True, check=False,
    )
    require(done.returncode == 0, f'D0 verification failed: {done.stderr.decode(errors="replace").strip()}')
    return json.loads(done.stdout)


def promotion_inputs(root, r5, candidate, receipt, revision, grammar=False):
    d0 = receipt['d0']
    expected = 'passed-with-listed-legacy-changes' if grammar else 'passed'
    require(receipt['status'] == 'candidate' and d0['status'] == expected,
            f'promotion requires a D0 candidate receipt with status {expected}')
    require(type(d0['elapsedMs']) is int and d0['elapsedMs'] >= 0, 'invalid D0 elapsed time')
    entry = receipt['registryEntry']
    require(re.fullmatch(r'[0-9a-f]{40}', revision) and entry['parser_revision'] in ('', revision),
            'reviewed producer revision is missing or differs from the receipt')
    verified = verify_promotion_d0(root, r5, candidate, grammar)
    # Elapsed time varies across runs. The producer commit is supplied by review.
    supplied = {**receipt, 'd0': {**d0, 'elapsedMs': verified['d0']['elapsedMs']},
                'registryEntry': {**entry, 'parser_revision': ''}}
    require(supplied.keys() == verified.keys(), 'D0 receipt fields differ from the verified report')
    for field, expected in verified.items():
        require(json.dumps(supplied[field], sort_keys=True, allow_nan=False)
                == json.dumps(expected, sort_keys=True, allow_nan=False),
                f'D0 receipt {field} differs from the verified report')

    r6 = {**verified['registryEntry'], 'parser_revision': revision}
    with candidate.open('rb') as archive:
        raw = archive.readline(1024 * 1024 + 1)
    require(raw.endswith(b'\n') and sha(raw[:-1]) == r6['manifest_sha256'], 'R6 header changed after D0 verification')
    return r6, raw, json.loads(raw)


def pin_frozen_inputs(inputs, candidate, r6, header):
    for role, field in [('oracle', 'corpus'), ('paperEvidence', 'paperEvidence')]:
        source = header[field]
        basename = source['snapshotFile']
        require(Path(basename).name == basename, f'invalid {role} snapshot basename')
        inputs[role].update(path=(Path(inputs[role]['path']).parent / basename).as_posix(),
                            bytes=source['bytes'], sha256=source['sha256'])
    inputs['oracle']['canonicalRecordsSha256'] = r6['r6']['canonical_records_sha256']
    inputs['scope'] = header['scope']
    inputs['rules'] = {field: header['rules'][field] for field in ('edition', 'sha256')}
    inputs['reader'] = {'revision': r6['parser_revision'], 'sourceSha256': r6['parser_source_sha256']}
    inputs['archive'] = {'file': candidate.name, 'schemaVersion': header['schemaVersion'],
                         'bytes': r6['r6']['file_bytes'], 'sha256': r6['file_sha256'],
                         'headerSha256': r6['manifest_sha256']}
    inputs['r6'].update(archiveManifest=r6['manifest_file'], printed=header['printed'], nameCatalog=header['nameCatalog'])


def install_promotion(source, registry_tests, out, previous_r6, r6):
    old_header, new_header = previous_r6['manifest_file'], r6['manifest_file']
    inputs_name = 'compile-inputs-phase1-r6.json'
    old_header_path = source / old_header
    require(Path(old_header).name == old_header and old_header_path.is_file() and not old_header_path.is_symlink()
            and sha(old_header_path.read_bytes().removesuffix(b'\n')) == previous_r6['manifest_sha256'],
            'current R6 header does not match its registry pin')
    require(old_header == new_header or not os.path.lexists(source / new_header), 'new R6 header path already exists')
    tests = registry_tests.read_bytes()
    require(tests.count(old_header.encode()) == 1, 'registry test must reference the current R6 header exactly once')

    before = json.loads((source / 'provenance.json').read_bytes())
    old_rows = [row for row in before['records'] if row['archive'] == old_header]
    current = source / 'phase1-r6'
    require(not current.is_symlink(), f'{current} is a symlink')
    expected = set()
    for row in old_rows:
        file = Path(row['file'])
        require(re.fullmatch(r'phase1-r6/[0-9a-f-]{36}\.json', file.as_posix()), f'invalid R6 fixture path {file}')
        path = source / file
        require(path.is_file() and not path.is_symlink()
                and sha(path.read_bytes().rstrip()) == row['record_sha256'], f'local R6 fixture differs from its pin: {path}')
        expected.add(path)
    require((set(current.iterdir()) if current.exists() else set()) == expected, f'unregistered entries in {current}')

    # Preserve the verified scratch copy as evidence. Stage same-filesystem
    # replacements before installing them so package files are never truncated.
    with tempfile.TemporaryDirectory(prefix='.promote-', dir=source) as stage:
        staged = Path(stage)
        replacements = []
        names = ['accepted-archives.json', 'provenance.json', new_header, inputs_name]
        for name in names:
            shutil.copyfile(out / name, staged / name)
            replacements.append((staged / name, source / name))
        for file in sorted((out / 'phase1-r6').iterdir()):
            shutil.copyfile(file, staged / file.name)
            replacements.append((staged / file.name, current / file.name))
        test_copy = staged / registry_tests.name
        test_copy.write_bytes(tests.replace(old_header.encode(), new_header.encode()))
        current.mkdir(exist_ok=True)
        for temp, target in replacements:
            if not target.exists() or target.read_bytes() != temp.read_bytes():
                os.replace(temp, target)
        if old_header != new_header:
            os.replace(test_copy, registry_tests)
    replaced = {target for _, target in replacements}
    for obsolete in sorted(expected - replaced):
        obsolete.unlink()
    if old_header != new_header:
        old_header_path.unlink()


def promote_grammar(args, root, source, registry):
    """Keep prior origins; pin only the new cut's reviewed witness rows."""
    require(args.previous_r6 and args.fixture, 'grammar promotion requires the previous R6 archive and witness IDs')
    require(re.fullmatch(r'[a-z][a-z0-9]*(?:-[a-z0-9]+)*', args.grammar_cut), 'invalid grammar cut name')
    require(len(set(args.fixture)) == len(args.fixture)
            and all(re.fullmatch(r'[0-9a-f-]{36}', oid) for oid in args.fixture), 'invalid witness IDs')
    r6, raw, header = promotion_inputs(root, args.r5, args.candidate,
                                       json.loads(args.receipt.read_bytes()), args.producer_revision,
                                       grammar=True)
    require(not any(a['manifest_sha256'] == r6['manifest_sha256'] for a in registry), 'cut is already registered')
    inputs_name = f'compile-inputs-{args.grammar_cut}.json'
    require(not (source / inputs_name).exists() and not (source / r6['manifest_file']).exists(),
            'new grammar cut already has local inputs')
    require(all(not (source / 'phase1-r6' / r6['manifest_sha256'] / f'{oid}.json').exists()
                for oid in args.fixture), 'new grammar cut already has local fixtures')
    # Re-extract the prior registered fixture cohort independently. Its row
    # pins and original files stay in place for cached readers.
    args.out.mkdir(parents=True)
    prior = fixtures(args.sim, source, args.out / 'prior', (args.r5, args.previous_r6))
    staged = args.out / 'grammar'
    staged.mkdir()
    provenance = json.loads((source / 'provenance.json').read_bytes())
    pretty(staged / 'accepted-archives.json', [*registry, r6])
    pretty(staged / 'provenance.json', provenance)
    (staged / r6['manifest_file']).write_bytes(raw)

    def sim_json(command, *tail):
        done = subprocess.run([str(args.sim.resolve()), 'catalog', command, '--promotion',
                               str(args.candidate), str(staged / 'provenance.json'), *tail],
                              capture_output=True, check=False)
        require(done.returncode == 0, f'{command} failed: {done.stderr.decode(errors="replace").strip()}')
        return json.loads(done.stdout)

    extracted = sim_json('extract', *args.fixture)
    verified = sim_json('verify')
    require(len(extracted['extracted']) == verified['verified_records'] == len(args.fixture),
            'grammar witness extraction or verification omitted rows')
    installed = json.loads((staged / 'provenance.json').read_bytes())
    before = provenance['records']
    after = installed['records']
    require(after[:len(before)] == before, 'existing provenance rows changed')
    new_rows = after[len(before):]
    require({row['oracle_id'] for row in new_rows} == set(args.fixture)
            and all(row['archive'] == r6['manifest_file'] for row in new_rows),
            'grammar fixture rows have the wrong identities')
    pins = json.loads((staged / 'accepted-archives.json').read_bytes())
    require(pins[:-1] == registry and pins[-1]['record_sha256'] == sorted(r['record_sha256'] for r in new_rows),
            'grammar promotion altered old pins or failed to pin exact new rows')
    inputs = json.loads((source / 'compile-inputs-phase1-r6.json').read_bytes())
    pin_frozen_inputs(inputs, args.candidate, r6, header)
    inputs['r6']['legacyRows'] = 'changed'
    pretty(staged / inputs_name, inputs)
    files = [r6['manifest_file'], inputs_name, *(r['file'] for r in new_rows),
             'accepted-archives.json', 'provenance.json']
    with tempfile.TemporaryDirectory(prefix='.promote-grammar-', dir=source) as temp:
        temp = Path(temp)
        for name in files:
            (temp / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(staged / name, temp / name)
        for name in files:
            (source / name).parent.mkdir(parents=True, exist_ok=True)
            os.replace(temp / name, source / name)
    return {'comparison': 'grammar-promotion', 'cut': args.grammar_cut,
            'prior': prior, 'verified': verified, 'extracted': extracted,
            'archive_sha256': r6['file_sha256'], 'header_sha256': r6['manifest_sha256'],
            'producer_revision': args.producer_revision}


def promote(args):
    root = Path(__file__).resolve().parent.parent
    source = root / 'packages/game-engine/tests/fixtures'
    registry_tests = root / 'packages/game-engine/crates/contracts/src/archives.rs'
    registry = json.loads((source / 'accepted-archives.json').read_bytes())
    if args.grammar_cut:
        return promote_grammar(args, root, source, registry)
    previous_r6 = next(a for a in registry if a['cut'] == 'r6')
    r6, raw, header = promotion_inputs(root, args.r5, args.candidate,
                                       json.loads(args.receipt.read_bytes()), args.producer_revision)
    report = fixtures(args.sim, source, args.out, (args.r5, args.candidate), replacement=(r6, raw))
    inputs_name = 'compile-inputs-phase1-r6.json'
    inputs = json.loads((source / inputs_name).read_bytes())
    pin_frozen_inputs(inputs, args.candidate, r6, header)
    pretty(args.out / inputs_name, inputs)
    install_promotion(source, registry_tests, args.out, previous_r6, r6)
    return {**report, 'comparison': 'promotion', 'archive_sha256': r6['file_sha256'],
            'header_sha256': r6['manifest_sha256'], 'producer_revision': args.producer_revision}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    if sys.argv[1:2] == ['promote']:
        parser.add_argument('mode', choices=('promote',))
        parser.add_argument('candidate', type=Path)
        parser.add_argument('receipt', type=Path, help='passing D0 JSON receipt')
        parser.add_argument('--r5', type=Path, required=True, help='pinned R5 control archive')
        parser.add_argument('--sim', type=Path, required=True, help='mtg-sim built from this checkout')
        parser.add_argument('--out', type=Path, required=True, help='new scratch directory for the verified fixtures')
        parser.add_argument('--producer-revision', required=True, help='reviewed candidate producer commit')
        parser.add_argument('--grammar-cut', help='name for an additive grammar cut')
        parser.add_argument('--previous-r6', type=Path, help='retained R6 base archive for an additive cut')
        parser.add_argument('--fixture', action='append', help='one Oracle ID to pin from an additive cut')
    else:
        parser.add_argument('mode', choices=('d1', 'd2', 'fixtures'))
        parser.add_argument('before', type=Path, help='D1/D2 earlier run, or the fixtures directory')
        parser.add_argument('after', type=Path, help='D1/D2 later run, or a new scratch directory')
        parser.add_argument('--archives', nargs=2, type=Path, metavar=('R5', 'R6'))
        parser.add_argument('--sim', type=Path, help='mtg-sim binary for fixtures')
    args = parser.parse_args()
    try:
        if args.mode == 'promote':
            result = promote(args)
        elif args.mode == 'fixtures':
            require(args.sim is not None and args.archives is not None, 'fixtures requires --sim and --archives')
            result = fixtures(args.sim, args.before, args.after, args.archives)
        else:
            result = compare(args.mode, args.before, args.after, args.archives)
        print(json.dumps(result, indent=2))
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
