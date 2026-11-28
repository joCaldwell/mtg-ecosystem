import importlib.util
import io
import json
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location('regressions', Path(__file__).with_name('check-printed-regressions.py'))
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
HEADER = {'recordType': 'manifest', 'schemaVersion': 5, 'inputRecords': 1, 'legacy': {'fileSha256': 'r5'}}


def row(blocks, lines):
    """One source row with printed blocks and the legacy lines they read."""
    return {'sourceIndex': 0, 'oracleIdentity': 'a', 'sourceSha256': 's', 'source': {'name': 'Card'},
            'faces': [{'sourceFaceIndex': 0, 'result': {'lines': lines}}],
            'printed': {'faces': [{'sourceFaceIndex': 0, 'status': 'read', 'lines': [], 'blocks': blocks}]}}


def read(*nodes):
    return {'status': 'read', 'nodes': list(nodes)}


def node(path, start, end, basis='recorded'):
    return {'path': path, 'basis': basis, 'span': [[0, start, end]]}


def archive(record):
    return io.BytesIO((json.dumps(HEADER) + '\n' + json.dumps(record) + '\n').encode())


def compare(old, new):
    return guard.compare(archive(old), archive(new), archive(new))


class PrintedRegressions(unittest.TestCase):
    def test_a_newly_read_block_is_gained(self):
        old = row([{'status': 'failed'}], [{'ok': False}])
        new = row([read(node('', 0, 4))], [{'ok': True, 'ability': {'kind': 'spell'}}])
        receipt = compare(old, new)
        self.assertEqual(receipt['regressions'], [])
        self.assertEqual(receipt['gainedBlocks'], [{'sourceIndex': 0, 'name': 'Card', 'face': 0, 'block': 0}])

    def test_a_reparsed_block_is_listed_not_compared(self):
        old = row([read(node('', 0, 4), node('/what', 1, 3))], [{'ok': True, 'ability': {'kind': 'spell', 'n': 1}}])
        new = row([read(node('', 0, 4))], [{'ok': True, 'ability': {'kind': 'spell', 'n': 2}}])
        receipt = compare(old, new)
        self.assertEqual(receipt['regressions'], [])
        self.assertEqual(len(receipt['reparsedBlocks']), 1)

    def test_lost_evidence_for_the_same_ast_is_a_regression(self):
        lines = [{'ok': True, 'ability': {'kind': 'spell'}}]
        old = row([read(node('', 0, 4), node('/what', 1, 3))], lines)
        self.assertEqual(compare(old, row([read(node('', 0, 4), node('/what', 1, 3, 'unlocated'))], lines))['regressions'][0]['path'],
                         '/what')
        self.assertEqual(compare(old, row([read(node('', 0, 4))], lines))['regressions'][0]['reason'], 'AST node inventory changed')

    def test_a_block_that_stops_reading_is_a_regression(self):
        old = row([read(node('', 0, 4))], [{'ok': True, 'ability': {'kind': 'spell'}}])
        new = row([{'status': 'failed'}], [{'ok': False}])
        self.assertEqual(compare(old, new)['regressions'], [{'sourceIndex': 0, 'reason': 'read blocks changed'}])


if __name__ == '__main__':
    unittest.main()
