import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest


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


if __name__ == '__main__':
    unittest.main()
