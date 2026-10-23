"""Synthetic supplementary metadata/bytes only: no native, authority, or profile run."""
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

HERE = Path(__file__).absolute().parent
spec = importlib.util.spec_from_file_location('profile_helper_tests', HERE / 'generic_proof_profile.py')
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)
BUILD = 'b' * 64


def events():
    result = [{'schema': p.SCHEMA, 'kind': 'header', 'seq': 0, 'build_id': BUILD,
               'collection_scope': 'root_proofs_only', 'instrumentation': True}]
    starts = []
    for kind, living, operand, phase in p.expected_events():
        at = len(result) * 10
        if kind == 'begin':
            starts.append(at)
            duration = None
        else:
            duration = at - starts.pop() - 1
        count = 100 if phase != 'spool_finalize' else 80
        result.append({'schema': p.SCHEMA, 'kind': kind, 'seq': len(result),
                       'scope': {'living': living, 'equality_ordinal': 1, 'operand': operand},
                       'phase': phase, 'at_ns': at, 'duration_ns': duration,
                       'outcome': 'ok' if kind == 'end' else None,
                       'bytes': count if kind == 'end' and phase in p.BYTE_PHASES else None})
    result.append({'schema': p.SCHEMA, 'kind': 'terminal', 'seq': len(result),
                   'at_ns': len(result) * 10, 'collection_outcome': 'root_collection_complete'})
    return result


def encode(rows):
    return b''.join(json.dumps(row, separators=(',', ':')).encode() + b'\n' for row in rows)


def private_write(path, raw):
    path.write_bytes(raw)
    path.chmod(0o600)
    return path


class Streams(unittest.TestCase):
    def check(self, rows=None, raw=None, partial=False):
        return p.validate_stream(io.BytesIO(encode(events() if rows is None else rows) if raw is None else raw), BUILD, partial)

    def reject(self, mutate):
        rows = events()
        mutate(rows)
        with self.assertRaises(ValueError):
            self.check(rows)

    def test_complete_all_18_phases_each_root_and_both_operands(self):
        raw = encode(events())
        result = self.check(raw=raw)
        self.assertEqual(result['status'], 'complete_collection_only')
        self.assertFalse(result['acceptance'])
        self.assertEqual(result['sha256'], hashlib.sha256(raw).hexdigest())
        self.assertEqual(len(result['completed_phases']), 81)
        self.assertEqual({row['phase'] for row in result['completed_phases']}, p.PHASES)

    def test_actual_source_enum_and_wire_contract(self):
        # Source inspection only. Prevent synthetic fixtures defining their own
        # incompatible schema or silently missing a newly integrated enum label.
        src = (HERE.parents[1] / p.SOURCES[0]).read_text()
        body = re.search(r'pub\(super\) enum ProofPhase \{(.*?)\n\}', src, re.S)[1]
        names = re.findall(r'^    ([A-Z][A-Za-z0-9]*),$', body, re.M)
        snake = {re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower() for name in names}
        self.assertEqual(snake, p.PHASES)
        self.assertIn('const PROFILE_SCHEMA: &str = "' + p.SCHEMA + '";', src)
        self.assertIn('equality_ordinal: 1,', src)
        self.assertIn('base.join("' + p.CHILD + '")', src)

    def test_header_build_schema_fields_and_bool(self):
        for key, value in [('build_id', 'c' * 64), ('schema', 'other'), ('kind', 'terminal'),
                           ('instrumentation', 1), ('collection_scope', 'all_work'), ('private_payload', 'secret')]:
            with self.subTest(key=key):
                self.reject(lambda rows: rows[0].__setitem__(key, value))

    def test_phase_scope_sequence_time_outcome_and_numeric_aliases(self):
        edits = [lambda r: r[1].__setitem__('seq', True),
                 lambda r: r[2].__setitem__('seq', 99),
                 lambda r: r[1].__setitem__('at_ns', 1.0),
                 lambda r: r[2].__setitem__('at_ns', 0),
                 lambda r: r[2].__setitem__('duration_ns', p.U64 + 1),
                 lambda r: r[2].__setitem__('duration_ns', 999),
                 lambda r: r[1]['scope'].__setitem__('living', True),
                 lambda r: r[1]['scope'].__setitem__('living', 2),
                 lambda r: r[1]['scope'].__setitem__('equality_ordinal', 2),
                 lambda r: r[1]['scope'].__setitem__('operand', 'a'),
                 lambda r: r[1]['scope'].__setitem__('path', '/private/secret'),
                 lambda r: r[1].__setitem__('phase', 'unknown'),
                 lambda r: r[2].__setitem__('phase', 'root_standalone_restore'),
                 lambda r: r[2].__setitem__('outcome', 'failed'),
                 lambda r: r[1].__setitem__('bytes', 3),
                 lambda r: r[2].__setitem__('bytes', False),
                 lambda r: r[2].__setitem__('bytes', None),
                 lambda r: r[6].__setitem__('bytes', 100),
                 lambda r: r[4].__setitem__('bytes', 99),
                 lambda r: r[2].__setitem__('unrecognized', 4)]
        for i, edit in enumerate(edits):
            with self.subTest(case=i):
                self.reject(edit)

    def test_operand_snapshot_and_equality_byte_binding(self):
        for phase in ('strict_pass1', 'typed_pass2_decode_spool', 'history_cleanup', 'complete_generic_byte_eof_equality'):
            def edit(rows):
                next(r for r in rows if r.get('phase') == phase and r['kind'] == 'end')['bytes'] += 1
            with self.subTest(phase=phase):
                self.reject(edit)

    def test_duplicate_nonfinite_unknown_and_no_payload_leak(self):
        for raw in (b'{"schema":"x","schema":"y"}\n', b'{"at_ns":NaN}\n',
                    encode(events()).replace(b'"at_ns":10', b'"at_ns":Infinity', 1),
                    b'{"private":"SUPER_SECRET_STATE"}\n'):
            with self.assertRaises(ValueError):
                self.check(raw=raw)
            partial = self.check(raw=raw, partial=True)
            self.assertNotIn('SUPER_SECRET_STATE', json.dumps(partial))

    def test_terminal_alone_and_missing_error_and_after_terminal(self):
        self.reject(lambda r: r.__setitem__(slice(1, -1), []))
        self.reject(lambda r: r.pop())
        self.reject(lambda r: r[-1].__setitem__('collection_outcome', 'error'))
        self.reject(lambda r: r.append(copy.deepcopy(r[-1])))
        self.reject(lambda r: r[-1].__setitem__('collection_outcome', 'product_pass'))

    def test_failed_phase_cannot_be_followed_by_success(self):
        self.reject(lambda r: r[2].update(outcome='error', bytes=None))
        rows = events()[:3]
        rows[2].update(outcome='error', bytes=None)
        rows.append({'schema': p.SCHEMA, 'kind': 'terminal', 'seq': 3,
                     'at_ns': 40, 'collection_outcome': 'error'})
        result = self.check(rows, partial=True)
        self.assertEqual(result['terminal'], 'error')
        self.assertEqual(result['completed_phases'], [])
        self.assertEqual(result['status'], 'failed_or_partial')

    def test_torn_missing_and_inflight_only_keep_completed_facts(self):
        for raw in (encode(events()[:4]), encode(events()[:4]) + b'{"torn":',
                    encode(events())[:-1]):
            with self.assertRaises(ValueError):
                self.check(raw=raw)
            result = self.check(raw=raw, partial=True)
            self.assertFalse(result['acceptance'])
            self.assertGreater(len(result['completed_phases']), 0)
            self.assertEqual(result['hash_scope'], 'bounded_inspected_prefix_only')
        self.assertEqual(self.check(raw=encode(events()[:4]) + b'{"torn":', partial=True)['torn_tail_bytes'], 8)
        self.assertEqual(self.check(raw=encode(events()[:4]), partial=True)['unclosed_begins'][0]['phase'], 'root_standalone_restore')
        self.assertEqual(self.check(raw=b'', partial=True)['records'], 0)

    def test_all_stream_bounds_without_unbounded_read(self):
        class LinesOnly(io.BytesIO):
            def read(self, *args):
                raise AssertionError('must stream')
            def readline(self, size=-1):
                self_test.assertEqual(size, p.MAX_LINE + 1)
                return super().readline(size)
        self_test = self
        p.validate_stream(LinesOnly(encode(events())), BUILD)
        for limit, value in [('MAX_RECORDS', 3), ('MAX_LINE', 100), ('MAX_BYTES', 200)]:
            with patch.object(p, limit, value), self.assertRaises(ValueError):
                self.check()
        with self.assertRaises(ValueError):
            self.check(raw=b'x' * 1025)


class Files(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name) / 'attempt'
        self.output.mkdir(mode=0o700)
        (self.output / 'tmp').mkdir(mode=0o700)
        self.base = self.output / 'execution'
        self.base.mkdir(mode=0o700)
        self.child = self.base / p.CHILD
        self.child.mkdir(mode=0o700)
        self.path = private_write(self.child / 'events.ndjson', encode(events()))
        for name in ('host-report.json', 'payload-spec.json', 'launcher.stdout', 'launcher.stderr',
                     'payload-ready.json', 'release', 'payload-result.json', 'workload.stdout',
                     'workload.stderr', 'native-report.json'):
            private_write(self.base / name, b'fixture only')

    def test_private_success_and_explicit_partial_scope(self):
        self.assertEqual(p.validate_attempt(self.output, BUILD)['status'], 'complete_collection_only')
        result = p.inspect_failed(self.output, BUILD)
        self.assertFalse(result['acceptance'])
        self.assertIn('OUTSIDE_120s_NOT_PASS', result['scope'])
        # Even an intact successful terminal after failed closure is no pass.
        self.assertFalse(result['profile']['acceptance'])
        self.assertEqual(result['profile']['status'], 'failed_or_partial')
        self.assertEqual(result['profile']['collection_observation'], 'complete_collection_only')

    def test_modes_symlinks_namespace_markers_and_orphan_scratch(self):
        for path, mode in ((self.path, 0o644), (self.child, 0o755), (self.base, 0o755), (self.output, 0o755)):
            old = path.stat().st_mode & 0o777
            path.chmod(mode)
            with self.assertRaises(ValueError):
                p.validate_attempt(self.output, BUILD)
            path.chmod(old)
        for location in (self.child, self.base, self.output, self.output / 'tmp'):
            marker = private_write(location / 'failure-marker', b'failed')
            with self.assertRaises(ValueError):
                p.validate_attempt(self.output, BUILD)
            marker.unlink()
        held = self.child / 'held'
        self.path.rename(held)
        self.path.symlink_to(held)
        with self.assertRaises(ValueError):
            p.validate_attempt(self.output, BUILD)
        self.path.unlink()
        held.rename(self.path)
        self.path.unlink()
        self.assertEqual(p.inspect_failed(self.output, BUILD)['profile']['status'], 'missing_or_unsafe_profile')

    def test_raw_dot_parent_and_symlink_ancestry_and_hardlink(self):
        for path in (str(self.output) + '/./execution', str(self.output) + '/../attempt/execution', 'relative/path'):
            with self.assertRaises(ValueError):
                p.plain(path, directory=True)
        alias = Path(self.temp.name) / 'alias'
        alias.symlink_to(self.output, target_is_directory=True)
        with self.assertRaises(ValueError):
            p.plain(alias / 'execution', directory=True)
        os.link(self.path, self.base / 'link')
        with self.assertRaises(ValueError):
            p.sidecar(self.output)


class Authorities(unittest.TestCase):
    """Made-up bytes, identities, and metadata; never runnable authorities."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.stage = self.base / 'stage'
        self.stage.mkdir(mode=0o700)
        self.here = self.stage / 'scripts'
        self.here.mkdir(mode=0o700)
        for name in p.HOSTS:
            private_write(self.here / name, b'# synthetic non-executable source\n')
        rows = []
        for rel in p.SOURCES:
            path = self.stage / rel
            path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            private_write(path, b'// synthetic, never compiled\n')
            rows.append({'path': rel, 'sha256': p.sha_bytes(path.read_bytes()), 'bytes': path.stat().st_size})
        def row(path, obj):
            private_write(path, json.dumps(obj).encode())
            return {'path': str(path), 'sha256': p.sha_bytes(path.read_bytes())}
        native = row(self.base / 'synthetic-native', {'status': 'compiled-source-closure-verified', 'build_id': BUILD, 'files': rows})
        self.normal = {'stage_root': str(self.stage), 'engine_build_id': BUILD,
                       'test_executable': {'path': str(self.base / 'NOT-EXECUTABLE'), 'sha256': '0' * 64},
                       'native_manifest': native, 'compiler_artifact': {'path': str(self.base / 'NOT-ARTIFACT'), 'sha256': '1' * 64},
                       'host_scripts': {str(self.here / name): p.sha_bytes((self.here / name).read_bytes()) for name in p.HOSTS[:-1]}}
        normal_pin = row(self.base / 'synthetic-inputs', self.normal)
        self.args = SimpleNamespace(authority=Path(normal_pin['path']), authority_sha256=normal_pin['sha256'],
                                    output=self.base / 'new-attempt', profile_authority=self.base / 'synthetic-supplement',
                                    profile_authority_sha256=None,
                                    profile_helper_sha256=p.sha_bytes((self.here / p.HOSTS[-1]).read_bytes()))
        for flag, name in [('common_driver_sha256', p.HOSTS[1]), ('fixed_driver_sha256', p.HOSTS[2]), ('helper_sha256', p.HOSTS[3])]:
            setattr(self.args, flag, self.normal['host_scripts'][str(self.here / name)])
        self.a = {'schema': 'shrinking-proof-phase-authority-v1', 'normal_inputs': normal_pin,
                  **{key: self.normal[key] for key in ('stage_root', 'engine_build_id', 'test_executable', 'native_manifest', 'compiler_artifact')},
                  'host_sources': {str(self.here / name): p.sha_bytes((self.here / name).read_bytes()) for name in p.HOSTS},
                  'diagnostic_sources': {r['path']: r['sha256'] for r in rows}, 'output': str(self.args.output),
                  'entrypoint': 'synthetic-entry', 'sidecar_schema': p.SCHEMA, 'collection_scope': 'root_proofs_only',
                  'instrumentation': True, 'switch': {'MTG_SHRINKING_PROOF_PHASE_V1': '1'}, 'policy': copy.deepcopy(p.POLICY)}
        self.write()

    def write(self):
        private_write(self.args.profile_authority, json.dumps(self.a).encode())
        self.args.profile_authority_sha256 = p.sha_bytes(self.args.profile_authority.read_bytes())

    def guard(self, fresh=True):
        return p.authority_guard(self.args, self.normal, self.here, self.stage, 'synthetic-entry', fresh)

    def test_exact_binding_fresh_output_and_no_directory_creation(self):
        self.assertEqual(self.guard(), self.a)
        self.assertFalse(self.args.output.exists())
        self.args.output.mkdir(mode=0o700)
        with self.assertRaises(ValueError):
            self.guard()
        self.guard(fresh=False)
        self.args.output.chmod(0o755)
        with self.assertRaises(ValueError):
            self.guard(fresh=False)

    def test_strict_supplement_pins_source_header_output_and_caps(self):
        original = copy.deepcopy(self.a)
        edits = [lambda a: a.__setitem__('unknown', 1),
                 lambda a: a.__setitem__('engine_build_id', 'c' * 64),
                 lambda a: a['normal_inputs'].__setitem__('sha256', '0' * 64),
                 lambda a: a['test_executable'].__setitem__('sha256', '0' * 63 + '1'),
                 lambda a: a['compiler_artifact'].__setitem__('path', '/wrong'),
                 lambda a: a.__setitem__('instrumentation', 1),
                 lambda a: a.__setitem__('output', str(self.base / 'different')),
                 lambda a: a.__setitem__('sidecar_schema', 'wrong'),
                 lambda a: a['switch'].__setitem__('MTG_SHRINKING_PROOF_PHASE_V1', '0'),
                 lambda a: a['diagnostic_sources'].__setitem__(p.SOURCES[0], '0' * 64),
                 lambda a: a['diagnostic_sources'].pop(p.SOURCES[1]),
                 lambda a: a['host_sources'].__setitem__(str(self.here / 'generic_proof_profile.py'), '0' * 64)]
        for key in p.POLICY:
            edits.append(lambda a, key=key: a['policy'].__setitem__(key, None))
        for i, edit in enumerate(edits):
            with self.subTest(case=i):
                self.a = copy.deepcopy(original)
                edit(self.a)
                self.write()
                with self.assertRaises((ValueError, KeyError)):
                    self.guard()

    def test_changed_pinned_bytes_and_supplement_mode(self):
        self.args.profile_authority_sha256 = '0' * 64
        with self.assertRaises(ValueError):
            self.guard()
        self.write()
        self.args.profile_authority.chmod(0o644)
        with self.assertRaises(ValueError):
            self.guard()
        self.write()
        private_write(self.stage / p.SOURCES[0], b'changed compiled source')
        with self.assertRaises(ValueError):
            self.guard()


if __name__ == '__main__':
    unittest.main()
