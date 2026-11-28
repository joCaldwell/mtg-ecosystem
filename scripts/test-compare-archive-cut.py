import importlib.util
from copy import deepcopy
import json
from pathlib import Path
from subprocess import CompletedProcess
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location('compare_archive_cut', Path(__file__).with_name('compare-archive-cut.py'))
cut = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cut)
ARCHIVE_FIELDS = ('archive_sha256', 'archive_manifest_sha256', 'parser_revision', 'parser_source_sha256')


class CompareArchiveCut(unittest.TestCase):
    def bundle(self, root, r6=False, *, definition_change=False, index_change=False, tier_change=False):
        root.mkdir()
        (root / 'bulk').mkdir()
        (root / 'coverage').mkdir()
        parser_hash = ('6' if r6 else '5') * 64
        parser = {'sourceSha256': parser_hash}
        revision = ('6' if r6 else '5') * 40
        if not r6:
            parser['revision'] = revision
        header = cut.json_bytes({'schemaVersion': 5 if r6 else 4, 'parser': parser})
        record = {'sourceIndex': 0, 'oracleIdentity': 'a', 'status': 'parsed'}
        if r6:
            printed = {'schemaVersion': 1, 'faces': []}
            record.update(printed=printed, printedSha256=cut.sha(cut.json_bytes(printed)))
        records = [cut.json_bytes(record), cut.json_bytes({'sourceIndex': 1, 'oracleIdentity': 'b', 'status': 'excluded'})]
        archive = root / 'archive.jsonl'
        archive.write_bytes(b'\n'.join([header, *records, b'']))
        provenance = {'archive_manifest_sha256': cut.sha(header), 'parser_sha256': parser_hash, 'source_sha256': 'a' * 64}
        back = {'name': 'B' if definition_change else 'A', 'provenance': provenance}
        catalog = {'definitions': [{'oracle_id': 'a', 'provenance': provenance, 'back_face': back}], 'schema_version': 1}
        index = {'rows': [
            {'oracle_id': oid, 'source_index': index, 'raw_row_sha256': cut.sha(raw + b'\n'),
             'record_sha256': cut.sha(raw), 'status': status, 'reason_code': ('x' if index_change else 'y'),
             'facts': {'commander_legality': 'legal'}}
            for index, (oid, raw, status) in enumerate(zip(('a', 'b'), records, ('lowered_validated', 'unparsed'), strict=True))
        ]}
        manifest = dict(zip(ARCHIVE_FIELDS, (cut.sha(archive.read_bytes()), cut.sha(header), revision, parser_hash), strict=True))
        manifest.update(producer_build_id='d' * 64, ir_version=1)
        for name, value in [('catalog', catalog), ('index', index)]:
            raw = cut.json_bytes(value)
            (root / 'bulk' / f'{name}.json').write_bytes(raw)
            manifest[name] = {'bytes': len(raw), 'sha256': cut.sha(raw)}
        manifest['catalog_digest'] = manifest['catalog']['sha256']
        manifest_raw = cut.json_bytes(manifest)
        (root / 'bulk/manifest.json').write_bytes(manifest_raw)
        summary = {key: manifest[key] for key in ARCHIVE_FIELDS if key != 'parser_source_sha256'}
        summary['lowered_validated'] = 1
        (root / 'coverage/summary.json').write_text(json.dumps(summary))
        (root / 'coverage/records.tsv').write_text('oracle_id\tstatus\na\tlowered_validated\nb\tunparsed\n')
        (root / 'coverage/blockers.tsv').write_text('oracle_id\tcode\n')
        (root / 'tiers.json').write_text(json.dumps([['a', [True, False, False, tier_change]]]))
        (root / 'admission.json').write_text(json.dumps({
            'bulk_manifest_sha256': cut.sha(manifest_raw), 'catalog_digest': manifest['catalog_digest'],
            'decks': [{'id': 1, 'status': 'rejected'}],
        }))
        return archive

    def test_d1_and_d2(self):
        with TemporaryDirectory() as path:
            root = Path(path)
            r5, same, r6 = (root / name for name in ('r5', 'same', 'r6'))
            old = self.bundle(r5)
            self.bundle(same)
            new = self.bundle(r6, r6=True)
            self.assertEqual(cut.compare('d1', r5, same)['lowered'], 1)
            self.assertEqual(cut.compare('d2', r5, r6, [old, new])['tiers'], [1, 0, 0, 0])

    def test_d2_rejects_changes_outside_the_allowlist(self):
        with TemporaryDirectory() as path:
            root = Path(path)
            r5 = root / 'r5'
            old = self.bundle(r5)
            for field, expected in [('definition_change', 'definition fields'), ('index_change', 'index fields'),
                                    ('tier_change', 'tier decisions')]:
                with self.subTest(field=field):
                    after = root / field
                    new = self.bundle(after, r6=True, **{field: True})
                    with self.assertRaisesRegex(ValueError, expected):
                        cut.compare('d2', r5, after, [old, new])

    def test_promotion_checks_the_entire_d0_receipt(self):
        with TemporaryDirectory() as path:
            root = Path(path)
            candidate, r5 = root / 'r6.jsonl', root / 'r5.jsonl'
            header = {'schemaVersion': 5}
            raw = cut.json_bytes(header)
            candidate.write_bytes(raw + b'\n')
            revision = '1' * 40
            report = {
                'status': 'candidate',
                'd0': {'status': 'passed', 'elapsedMs': 5},
                'registryEntry': {'parser_revision': '', 'manifest_sha256': cut.sha(raw)},
                'sizes': {'bytes': 42, 'rows': 2, 'lines': 3, 'largestLine': {'line': 3, 'bytes': 25}},
                'printedSummary': {'records': 2},
                'legacy': {'fileSha256': 'a' * 64},
                'rulesSha256': 'b' * 64,
            }
            with patch.object(cut, 'verify_promotion_d0', return_value=report) as verify:
                receipt = deepcopy(report)
                receipt['d0']['elapsedMs'] = 9
                entry, _, actual_header = cut.promotion_inputs(root, r5, candidate, receipt, revision)
                verify.assert_called_once_with(root, r5, candidate, False)
                self.assertEqual(entry['parser_revision'], revision)
                self.assertEqual(actual_header, header)
                changes = [
                    ('sizes', {'bytes': 1, 'rows': 1}),
                    ('sizes', {'lines': 1}),
                    ('sizes', {'largestLine': {'line': 1, 'bytes': 25}}),
                    ('printedSummary', {}),
                    ('registryEntry', {'manifest_sha256': '0' * 64}),
                    ('legacy', {'fileSha256': '0' * 64}),
                    ('d0', {'unexpected': True}),
                ]
                for field, values in changes:
                    with self.subTest(field=field, values=values):
                        bad = deepcopy(receipt)
                        if field == 'printedSummary':
                            bad[field] = values
                        else:
                            bad[field].update(values)
                        with self.assertRaisesRegex(ValueError, f'D0 receipt {field} differs'):
                            cut.promotion_inputs(root, r5, candidate, bad, revision)

    def test_fixture_order_is_independent_of_previous_r6_provenance(self):
        with TemporaryDirectory() as path:
            root = Path(path)
            source = root / 'source'
            source.mkdir()
            r5 = {'cut': 'r5', 'manifest_file': 'r5.json', 'manifest_sha256': '5' * 64, 'record_sha256': []}
            r6 = {'cut': 'r6', 'manifest_file': 'r6.json', 'manifest_sha256': '6' * 64, 'record_sha256': []}
            rows = []
            for line, oid in enumerate(('b', 'a'), 2):
                raw = cut.json_bytes({'status': 'excluded', 'oracleIdentity': oid})
                pin = cut.sha(raw)
                r5['record_sha256'].append(pin)
                rows.append({'oracle_id': oid, 'archive': r5['manifest_file'], 'file': f'{oid}.json',
                             'archive_line': line, 'record_sha256': pin, 'source_sha256': pin, 'ast_sha256': None})
                (source / f'{oid}.json').write_bytes(raw + b'\n')
            header = b'{}\n'
            (source / r5['manifest_file']).write_bytes(header)
            (source / r6['manifest_file']).write_bytes(header)

            def sim(args, **_):
                command, *operands = args[2:]
                if command == 'verify':
                    return CompletedProcess(args, 0, stdout=b'{"verified_records":2}', stderr=b'')
                _, provenance_path, *_ = operands
                provenance_path = Path(provenance_path)
                out = provenance_path.parent
                registry_path = out / 'accepted-archives.json'
                registry = json.loads(registry_path.read_bytes())
                provenance = json.loads(provenance_path.read_bytes())
                (out / 'phase1-r6').mkdir()
                extracted = []
                for row in sorted(rows, key=lambda record: record['oracle_id']):
                    file = f'phase1-r6/{row["file"]}'
                    (out / file).write_bytes((out / row['file']).read_bytes())
                    extracted.append({**row, 'archive': r6['manifest_file'], 'file': file})
                provenance['records'].extend(extracted)
                registry[1]['record_sha256'] = sorted(row['record_sha256'] for row in extracted)
                cut.pretty(provenance_path, provenance)
                cut.pretty(registry_path, registry)
                return CompletedProcess(args, 0, stdout=cut.json_bytes({'extracted': extracted}), stderr=b'')

            with patch.object(cut.subprocess, 'run', side_effect=sim):
                empty, promoted = root / 'empty', root / 'promoted'
                cut.pretty(source / 'accepted-archives.json', [r5, r6])
                cut.pretty(source / 'provenance.json', {'records': rows})
                cut.fixtures(root / 'sim', source, empty, ('r5', 'r6'))
                previous = [{**row, 'archive': r6['manifest_file']} for row in rows]
                cut.pretty(source / 'accepted-archives.json', [r5, {**r6, 'record_sha256': r5['record_sha256']}])
                cut.pretty(source / 'provenance.json', {'records': rows + previous})
                cut.fixtures(root / 'sim', source, promoted, ('r5', 'r6'))
            self.assertEqual((empty / 'provenance.json').read_bytes(), (promoted / 'provenance.json').read_bytes())
            output = json.loads((promoted / 'provenance.json').read_bytes())['records']
            self.assertEqual(output[:2], rows)
            self.assertEqual([row['oracle_id'] for row in output[2:]], ['a', 'b'])


if __name__ == '__main__':
    unittest.main()
