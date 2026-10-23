"""Synthetic host metadata and independently emitted bytes; no native execution."""
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
spec = importlib.util.spec_from_file_location('legal_profile_tests', HERE / 'legal_proof_profile.py')
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)
BUILD = 'b' * 64


def encode(rows):
    return b''.join(json.dumps(row, separators=(',', ':')).encode() + b'\n' for row in rows)


def events(case='ordinary_4', histories=(10, 11, 12, 13)):
    """Model native execution of EVERY prefix, independently of parser grammar.

    A fixed duration is charged even when an event is unsampled. Checkpoints
    therefore include more calls/time/bytes than their durable phase receipts.
    Fixtures are synthetic, never timing or engine correctness evidence.
    """
    rows = [{'schema': p.SCHEMA, 'kind': 'header', 'seq': 0, 'build_id': BUILD,
             'collection_scope': 'root_proofs_only', 'instrumentation': True}]
    clock = 0
    totals = [[0, 0, 0] for _ in range(8)]
    def emit(kind, s=None, phase=None, duration=None, count=None):
        nonlocal clock
        clock += 10
        e = {'schema': p.SCHEMA, 'kind': kind, 'seq': len(rows), 'at_ns': clock}
        if kind == 'checkpoint':
            e.update(scope=s.copy(), last_completed_prefix=s['prefix_ordinal'],
                     totals=[[i, *t] for i, t in enumerate(totals) if t[0]])
        elif kind == 'terminal':
            e['collection_outcome'] = 'root_collection_complete'
        else:
            e.update(scope=s.copy(), phase=phase, duration_ns=duration,
                     outcome='ok' if kind == 'end' else None, bytes=count)
        rows.append(e)
    def run(phase, s, sampled=True, children=None):
        nonlocal clock
        if sampled:
            emit('begin', s, phase)
        begin = clock
        clock += 2
        if children:
            children()
        clock += 3
        duration = clock - begin
        count = 100 if phase in ('reference_serialize_write_hash', 'actual_serialize_compare_hash',
                                 'finish_eof_count_digest', 'export_and_receipt') else None
        if phase in p.GROUPS and s['query_kind'] == 'search':
            total = totals[p.GROUPS.index(phase)]
            total[0] += 1
            total[1] += duration
            total[2] += count or 0
        if sampled:
            emit('end', s, phase, duration, count)
    def comparison(s, sampled):
        for phase in ('reference_serialize_write_hash', 'actual_serialize_compare_hash', 'finish_eof_count_digest'):
            run(phase, s, sampled)
    def scratch(s, sampled):
        run('metadata_conversion', s, sampled)
        comparison(s, sampled)
    def cold(s):
        run('sparse_cold_save', s)
        run('sparse_saved_envelope_comparison', s, children=lambda: comparison(s, True))
        run('sparse_cold_restore', s)
        run('restored_scratch', s, children=lambda: scratch(s, True))
    root = dict.fromkeys('case viewer query_kind quantum prefix_ordinal units_done history_entries proof_kind'.split())
    root.update(case=case, proof_kind='root')
    run('root_prepare', root)
    run('initial_generic_state_oracle', root)
    living = int(case[-1])
    seats = {4: [0, 1, 2, 3], 3: [0, 1, 3], 2: [0, 3]}[living]
    for viewer in range(4):
        for kind in ('observe', 'decision', 'search'):
            if kind == 'search' and viewer not in seats:
                continue
            query = dict(root, viewer=viewer, query_kind=kind)
            emit('begin', query, 'query_proof')
            begin = clock
            if kind == 'search':
                for quantum in (1, 64, (1 << 64) - 1):
                    totals = [[0, 0, 0] for _ in range(8)]
                    base = dict(query, quantum=quantum, history_entries=histories[viewer])
                    s = dict(base, prefix_ordinal=0, units_done=0)
                    run('quantum_setup', s)
                    run('lazy_zero', s, children=lambda: scratch(s, True))
                    emit('checkpoint', s)
                    units = 100 * living + 4 + histories[viewer] + 4 + 1
                    done, ordinal, cold_saves = 0, 0, 0
                    while done < units:
                        ordinal += 1
                        sample = ordinal & (ordinal - 1) == 0 or (quantum == 1 and ordinal in (1, 5, 31, 129)) or (quantum == 64 and cold_saves == 0)
                        s = dict(base, prefix_ordinal=ordinal, units_done=done)
                        run('reference_resume', s, sample)
                        run('actual_resume', s, sample)
                        done = min(done + quantum, units)
                        s = dict(s, units_done=done)
                        if done == units:
                            run('completed_result', s, children=lambda: scratch(s, True))
                            break
                        scratch(s, sample)
                        if (quantum == 1 and done in (1, 5, 31, 129)) or (quantum == 64 and cold_saves == 0):
                            run('sparse_cold_proof', s, children=lambda: cold(s))
                            cold_saves += 1
                        if sample:
                            emit('checkpoint', s)
                    run('checked_directory_cleanup', s)
                    emit('checkpoint', s)
            emit('end', query, 'query_proof', clock - begin)
    run('first_write_action_selection', root)
    for q in (1, (1 << 64) - 1):
        run('budget_first_write', dict(root, proof_kind='firstwrite', quantum=q))
    run('first_write_generic_state_oracle', dict(root, proof_kind='firstwrite'))
    for proof_kind in ('root', 'firstwrite'):
        s = dict(root, proof_kind=proof_kind)
        def snapshot():
            for phase in ('export_and_receipt', 'standalone_restore_inclusive',
                          'read_receipt_and_state_digest_validation', 'checked_directory_cleanup'):
                run(phase, s)
        run('snapshot_proof', s, children=snapshot)
        run('independent_accepted_input_replay', s)
        run('replay_generic_state_oracle', s)
    run('final_root_digest', root)
    emit('terminal')
    return rows


def private_write(path, raw):
    path.write_bytes(raw)
    path.chmod(0o600)
    return path


class Streams(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = events()
        cls.raw = encode(cls.fixture)

    def check(self, rows=None, raw=None, partial=False, case='ordinary_4', histories=(10, 11, 12, 13)):
        return p.validate_stream(io.BytesIO(raw if raw is not None else encode(rows) if rows is not None else self.raw),
                                 BUILD, case, histories, partial)

    def reject(self, edit):
        rows = copy.deepcopy(self.fixture)
        edit(rows)
        with self.assertRaises(ValueError):
            self.check(rows)

    def test_all_six_actual_scoped_worklists_and_root_only_completion(self):
        for case in p.CASES:
            r = self.check(events(case), case=case)
            self.assertEqual(r['status'], 'complete_collection_only')
            self.assertFalse(r['acceptance'])
            self.assertEqual({e['phase'] for e in r['completed_phases']}, p.PHASES)
            self.assertLess(r['records'], p.MAX_RECORDS)
        self.assertEqual(self.check()['sha256'], hashlib.sha256(self.raw).hexdigest())

    def test_actual_rust_enum_scope_schema_and_sampler(self):
        src = (HERE.parents[1] / p.SOURCES[0]).read_text()
        body = re.search(r'pub\(in crate::engine\) enum ProofPhase \{(.*?)\n\}', src, re.S)[1]
        names = re.findall(r'^    ([A-Z][A-Za-z0-9]*),$', body, re.M)
        self.assertEqual({re.sub(r'(?<!^)(?=[A-Z])', '_', n).lower() for n in names}, p.PHASES)
        body = re.search(r'struct ProofScope \{(.*?)\n\}', src, re.S)[1]
        self.assertEqual(set(re.findall(r'^    (\w+):', body, re.M)), set(p.SCOPE_KEYS.split()))
        self.assertIn('const PROFILE_SCHEMA: &str = "' + p.SCHEMA + '";', src)
        self.assertIn('base.join("' + p.CHILD + '")', src)
        self.assertIn('ordinal == 0 || ordinal.is_power_of_two() || cold || final_result', src)
        self.assertIn('p.prefix(ordinal, reference.units_done, cold, false)', src)
        self.assertIn('p.scope.units_done = Some(reference.units_done)', src)
        # Current finite root preparation never creates stack/effect inhabitants;
        # source binds the unit model, rather than a measured shortcut.
        reference = src[src.index('impl Reference {'):src.index('fn scratch(')]
        self.assertIn('.checked_add(1)', reference)
        self.assertIn('self.decks.len() < 4', reference)
        self.assertIn('reference-policy-seed', (HERE.parents[1] / p.SOURCES[2]).read_text())

    def test_resume_start_cursor_vs_returned_cursor_and_unsampled_final(self):
        rows = self.fixture
        begin = next(e for e in rows if e.get('phase') == 'reference_resume')
        self.assertEqual(begin['scope']['units_done'], 0)
        completed = next(e for e in rows if e.get('phase') == 'completed_result')
        self.assertEqual(completed['scope']['units_done'], 419)
        self.assertEqual(completed['scope']['prefix_ordinal'], 419)
        self.assertFalse(any(e.get('phase') == 'reference_resume' and e['scope']['prefix_ordinal'] == 419 for e in rows))
        for phase in ('reference_resume', 'actual_resume', 'completed_result', 'metadata_conversion'):
            def edit(r):
                e = next(e for e in r if e.get('phase') == phase and e['kind'] == 'begin' and e['scope']['prefix_ordinal'])
                e['scope']['units_done'] += 1
            self.reject(edit)

    def test_scope_none_fields_viewers_quantums_proof_identity_order(self):
        edits = [lambda e: e['scope'].__setitem__('quantum', 1),
                 lambda e: e['scope'].__setitem__('query_kind', 'search'),
                 lambda e: e['scope'].__setitem__('viewer', 0),
                 lambda e: e['scope'].__setitem__('proof_kind', 'firstwrite'),
                 lambda e: e['scope'].__setitem__('history_entries', 0),
                 lambda e: e['scope'].__setitem__('private_state', 'PRIVATE')]
        for edit in edits:
            self.reject(lambda r: edit(r[1]))
        self.reject(lambda r: next(e for e in r if e.get('phase') == 'quantum_setup')['scope'].__setitem__('quantum', 2))
        self.reject(lambda r: next(e for e in r if e.get('phase') == 'quantum_setup')['scope'].__setitem__('history_entries', 99))
        self.reject(lambda r: r[1].__setitem__('phase', 'final_root_digest'))
        self.reject(lambda r: r[1]['scope'].__setitem__('case', 'dense_4'))

    def test_header_build_exact_keys_types_and_no_private_payload(self):
        for key, value in [('schema', 'generic-proof-phase-events-v1'), ('build_id', 'c'*64),
                           ('kind', 'terminal'), ('instrumentation', 1), ('seq', False),
                           ('collection_scope', 'whole_program'), ('private_payload', 'PRIVATE')]:
            self.reject(lambda r, key=key, value=value: r[0].__setitem__(key, value))
        for raw in (b'{"schema":"x","schema":"x"}\n', b'{"value":NaN}\n', b'{"private":"SECRET"}\n'):
            with self.assertRaises(ValueError):
                self.check(raw=raw)
            result = self.check(raw=raw, partial=True)
            self.assertNotIn('SECRET', json.dumps(result))

    def test_exact_pairs_phase_bytes_time_sequence_u64_and_nulls(self):
        edits = [lambda r: r[1].__setitem__('seq', 2), lambda r: r[1].__setitem__('at_ns', True),
                 lambda r: r[2].__setitem__('at_ns', 0), lambda r: r[2].__setitem__('duration_ns', 1000),
                 lambda r: r[2].__setitem__('duration_ns', p.U64+1), lambda r: r[2].__setitem__('duration_ns', 1.0),
                 lambda r: r[1].__setitem__('outcome', 'ok'), lambda r: r[1].__setitem__('bytes', 1),
                 lambda r: r[2].__setitem__('bytes', 1), lambda r: r[2].__setitem__('outcome', 'failed'),
                 lambda r: r[2].__setitem__('phase', 'initial_generic_state_oracle'),
                 lambda r: r[1]['scope'].__setitem__('viewer', True)]
        for edit in edits:
            self.reject(edit)
        for value in (None, 99, False, 100.0):
            def edit(r):
                next(e for e in r if e.get('phase') == 'actual_serialize_compare_hash' and e['kind'] == 'end')['bytes'] = value
            self.reject(edit)

    def test_checkpoint_all_prefix_counts_unsampled_duration_byte_totals(self):
        result = self.check()
        checkpoint = next(e for e in result['checkpoints'] if e['quantum'] == 1 and e['prefix_ordinal'] == 256)
        self.assertEqual(checkpoint['totals'][0][1], 256)
        sampled_resumes = sum(e['phase'] == 'reference_resume' and e['viewer'] == 0 and e['quantum'] == 1 and e['prefix_ordinal'] <= 256 for e in result['completed_phases'])
        self.assertGreater(256, sampled_resumes)
        for change in ('drop', 'calls', 'sample_calls', 'duration', 'bytes', 'order', 'extra', 'cursor', 'bool', 'overflow'):
            def edit(r):
                e = next(e for e in r if e['kind'] == 'checkpoint' and e['scope']['prefix_ordinal'] == 256)
                if change == 'drop': e['totals'].pop()
                elif change == 'calls': e['totals'][0][1] -= 1
                elif change == 'sample_calls': e['totals'][0][1] = sampled_resumes
                elif change == 'duration': e['totals'][0][2] = 0
                elif change == 'bytes': e['totals'][3][3] += 1
                elif change == 'order': e['totals'].reverse()
                elif change == 'extra': e['totals'].append([8, 1, 0, 0])
                elif change == 'cursor': e['last_completed_prefix'] -= 1
                elif change == 'bool': e['totals'][0][0] = False
                else: e['totals'][0][1] = p.U64+1
            self.reject(edit)

    def test_missing_duplicate_checkpoint_cold_phase_or_quantum_rejected(self):
        for phase in ('sparse_cold_save', 'reference_resume', 'completed_result', 'checked_directory_cleanup', 'quantum_setup'):
            self.reject(lambda r, phase=phase: r.pop(next(i for i, e in enumerate(r) if e.get('phase') == phase)))
        self.reject(lambda r: r.pop(next(i for i, e in enumerate(r) if e['kind'] == 'checkpoint')))
        self.reject(lambda r: r.insert(10, copy.deepcopy(r[9])))

    def checkpoint_delta(self, rows, group, duration):
        """Prefix 3 has no receipt; prefixes 2 -> 4 have 190ns elapsed."""
        checkpoints = [e for e in rows if e['kind'] == 'checkpoint'
                       and e['scope']['viewer'] == 0 and e['scope']['quantum'] == 1]
        before, after = [next(e for e in checkpoints if e['last_completed_prefix'] == n)
                         for n in (2, 4)]
        self.assertEqual(after['at_ns'] - before['at_ns'], 190)
        prior = next(t[2] for t in before['totals'] if t[0] == group)
        current = next(t[2] for t in after['totals'] if t[0] == group)
        for e in checkpoints:
            if e['last_completed_prefix'] >= 4:
                next(t for t in e['totals'] if t[0] == group)[2] += duration - (current - prior)

    def test_checkpoint_191ns_group_work_in_190ns_rejected(self):
        for group in range(6):
            with self.subTest(group=group):
                rows = copy.deepcopy(self.fixture)
                self.checkpoint_delta(rows, group, 191)
                with self.assertRaises(ValueError): self.check(rows)
                result = self.check(rows, partial=True)
                self.assertEqual(result['status'], 'failed_or_partial')

    def test_checkpoint_duration_equal_to_elapsed_is_valid(self):
        for group in range(6):
            with self.subTest(group=group):
                rows = copy.deepcopy(self.fixture)
                self.checkpoint_delta(rows, group, 190)
                self.assertEqual(self.check(rows)['status'], 'complete_collection_only')

    def set_duration(self, rows, end, duration):
        """Keep sampled group facts and cumulative receipts mutually consistent."""
        old = end['duration_ns']
        end['duration_ns'] = duration
        if end['phase'] in p.GROUPS and end['scope']['query_kind'] == 'search':
            group = p.GROUPS.index(end['phase'])
            for e in rows[end['seq'] + 1:]:
                if (e['kind'] == 'checkpoint'
                        and all(e['scope'][k] == end['scope'][k] for k in ('viewer', 'quantum'))):
                    next(t for t in e['totals'] if t[0] == group)[2] += duration - old

    def test_zero_snapshot_parent_with_positive_children_rejected(self):
        for proof in ('root', 'firstwrite'):
            with self.subTest(proof=proof):
                rows = copy.deepcopy(self.fixture)
                end = next(e for e in rows if e.get('phase') == 'snapshot_proof'
                           and e['kind'] == 'end' and e['scope']['proof_kind'] == proof)
                end['duration_ns'] = 0
                with self.assertRaises(ValueError): self.check(rows)

    def test_all_actual_nested_parents_require_inclusive_duration(self):
        for phase in ('query_proof', 'lazy_zero', 'completed_result', 'sparse_cold_proof',
                      'sparse_saved_envelope_comparison', 'restored_scratch'):
            with self.subTest(phase=phase):
                rows = copy.deepcopy(self.fixture)
                end = next(e for e in rows if e.get('phase') == phase and e['kind'] == 'end'
                           and e['scope']['query_kind'] == 'search')
                self.set_duration(rows, end, 0)
                with self.assertRaises(ValueError): self.check(rows)

    def test_inclusive_parent_exact_clock_boundary_and_one_ns_short(self):
        # StandaloneRestoreInclusive has no instrumented children. The other
        # snapshot phases are sequential siblings, outside that restore phase.
        for phase in ('snapshot_proof', 'lazy_zero', 'completed_result', 'sparse_cold_proof',
                      'sparse_saved_envelope_comparison', 'restored_scratch', 'query_proof'):
            with self.subTest(phase=phase):
                rows = copy.deepcopy(self.fixture)
                begin = next(e for e in rows if e.get('phase') == phase and e['kind'] == 'begin'
                             and (phase != 'query_proof' or e['scope']['query_kind'] == 'search'))
                end = next(e for e in rows[begin['seq'] + 1:] if e.get('phase') == phase
                           and e['kind'] == 'end' and e['scope'] == begin['scope'])
                child = rows[begin['seq'] + 1]
                self.assertEqual(child['kind'], 'begin')
                bound = end['at_ns'] - child['at_ns']
                self.set_duration(rows, end, bound)
                self.assertEqual(self.check(rows)['status'], 'complete_collection_only')
                self.set_duration(rows, end, bound - 1)
                with self.assertRaises(ValueError): self.check(rows)

    def test_zero_resolution_clocks_and_zero_duration_leaf_phases_valid(self):
        rows = copy.deepcopy(self.fixture)
        for e in rows:
            if 'at_ns' in e: e['at_ns'] = 0
            if e['kind'] == 'end': e['duration_ns'] = 0
            if e['kind'] == 'checkpoint':
                for t in e['totals']: t[2] = 0
        self.assertEqual(self.check(rows)['status'], 'complete_collection_only')
        rows = copy.deepcopy(self.fixture)
        for e in rows:
            if e['kind'] == 'end' and e.get('phase') == 'standalone_restore_inclusive':
                e['duration_ns'] = 0
        self.assertEqual(self.check(rows)['status'], 'complete_collection_only')

    def test_power_of_two_final_sampling_multiple_viewers_and_quantums_valid(self):
        histories = (103, 615, 0, 1)  # 512/1024 units: sampled finals in q1 and q64.
        rows = events(histories=histories)
        self.assertEqual(self.check(rows, histories=histories)['status'], 'complete_collection_only')

    def test_resequenced_omissions_duplicates_and_terminal_cannot_bypass_grammar(self):
        for phase in ('root_prepare', 'query_proof', 'quantum_setup', 'sparse_cold_save',
                      'budget_first_write', 'snapshot_proof', 'checked_directory_cleanup', 'final_root_digest'):
            rows = copy.deepcopy(self.fixture)
            rows = [e for e in rows if e.get('phase') != phase]
            for seq, e in enumerate(rows): e['seq'] = seq
            with self.assertRaises(ValueError): self.check(rows)
        rows = [self.fixture[0], dict(self.fixture[-1], seq=1)]
        with self.assertRaises(ValueError): self.check(rows)
        rows = copy.deepcopy(self.fixture)
        checkpoint = next(i for i, e in enumerate(rows) if e['kind'] == 'checkpoint')
        rows.insert(checkpoint + 1, copy.deepcopy(rows[checkpoint]))
        for seq, e in enumerate(rows): e['seq'] = seq
        with self.assertRaises(ValueError): self.check(rows)

    def test_terminal_never_acceptance_and_error_unwind(self):
        for edit in (lambda r: r.pop(), lambda r: r.__setitem__(slice(1, -1), []),
                     lambda r: r[-1].__setitem__('collection_outcome', 'error'),
                     lambda r: r[-1].__setitem__('collection_outcome', 'product_pass'),
                     lambda r: r.append(copy.deepcopy(r[-1])),
                     lambda r: r[2].update(outcome='error', bytes=None)):
            self.reject(edit)
        rows = copy.deepcopy(self.fixture[:3])
        rows[2].update(outcome='error', bytes=None)
        rows.append({'schema': p.SCHEMA, 'kind': 'terminal', 'seq': 3, 'at_ns': 100, 'collection_outcome': 'error'})
        result = self.check(rows, partial=True)
        self.assertEqual(result['terminal'], 'error')
        self.assertEqual(result['completed_phases'], [])
        self.assertEqual(result['status'], 'failed_or_partial')

    def test_partial_torn_inflight_and_complete_collection_are_not_passes(self):
        for raw in (encode(self.fixture[:4]), encode(self.fixture[:4]) + b'{"torn":', self.raw[:-1], b''):
            with self.assertRaises(ValueError):
                self.check(raw=raw)
            result = self.check(raw=raw, partial=True, histories=None)
            self.assertFalse(result['acceptance'])
            self.assertEqual(result['status'], 'failed_or_partial')
        self.assertEqual(self.check(raw=encode(self.fixture[:4]) + b'{"torn":', partial=True)['torn_tail_bytes'], 8)
        self.assertEqual(self.check(partial=True, histories=None)['status'], 'complete_collection_only')

    def test_stream_only_bounds_newline_inclusive_and_constant_sampler(self):
        class LinesOnly(io.BytesIO):
            def read(self, *args): raise AssertionError('must stream')
            def readline(inner, size=-1):
                self.assertEqual(size, p.MAX_LINE + 1)
                return super(LinesOnly, inner).readline(size)
        p.validate_stream(LinesOnly(self.raw), BUILD, 'ordinary_4', [10,11,12,13])
        for field, value in [('MAX_LINE', 100), ('MAX_BYTES', 200), ('MAX_RECORDS', 3)]:
            with patch.object(p, field, value), self.assertRaises(ValueError):
                self.check()
        header, tail = self.raw.split(b'\n', 1)
        exact = header + b' ' * (1023 - len(header)) + b'\n' + tail
        self.assertEqual(self.check(raw=exact)['status'], 'complete_collection_only')
        with self.assertRaises(ValueError): self.check(raw=header + b' ' * (1024-len(header)) + b'\n' + tail)
        with self.assertRaises(ValueError): self.check(raw=b'x'*1024+b'\n')
        self.assertLessEqual(len(p.sampled_prefixes(p.U64, 1)), 70)
        self.assertEqual(p.sampled_prefixes(1, p.U64), [0, 1])
        with self.assertRaises(ValueError): self.check(histories=[p.U64]*4)


class Files(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name) / 'attempt'
        self.output.mkdir(mode=0o700)
        scratch = self.output / 'scratch'
        scratch.mkdir(mode=0o700)
        self.child = scratch / p.CHILD
        self.child.mkdir(mode=0o700)
        self.path = private_write(self.child / 'events.ndjson', encode(events()))
        self.base = self.output / 'execution'
        self.base.mkdir(mode=0o700)
        for name in p.EXECUTION_FILES:
            private_write(self.base / name, b'synthetic only')
        self.normal = {'root_proof_verified': True, 'native_final_received': True,
                       'completed_sweeps':24, 'root':{'histories':[10,11,12,13]}}

    def validate(self):
        return p.validate_attempt(self.output, BUILD, 'ordinary_4', self.normal)

    def test_private_success_and_complete_terminal_after_failed_owner(self):
        self.assertEqual(self.validate()['status'], 'complete_collection_only')
        result = p.inspect_failed(self.output, BUILD, 'ordinary_4')
        self.assertIn('OUTSIDE_120s_NOT_PASS', result['scope'])
        self.assertEqual(result['profile']['status'], 'failed_or_partial')
        self.assertEqual(result['profile']['collection_observation'], 'complete_collection_only')
        self.assertFalse(result['profile']['acceptance'])
        self.normal['completed_sweeps'] = 23
        with self.assertRaises(ValueError): self.validate()

    def test_modes_failure_markers_orphans_and_extra_sidecars(self):
        for path in (self.path, self.child, self.base, self.output, self.output/'scratch'):
            mode = path.stat().st_mode & 0o777
            path.chmod(0o644 if path.is_file() else 0o755)
            with self.assertRaises(ValueError): self.validate()
            path.chmod(mode)
        for directory in (self.child, self.base, self.output, self.output/'scratch'):
            marker = private_write(directory/'failure', b'failure')
            with self.assertRaises(ValueError): self.validate()
            marker.unlink()
        self.path.unlink()
        self.assertEqual(p.inspect_failed(self.output, BUILD, 'ordinary_4')['profile']['status'], 'missing_or_unsafe_profile')

    def test_fresh_prelaunch_rejects_collisions_sidecars_and_failure_markers(self):
        output = Path(self.temp.name)/'fresh'
        output.mkdir(mode=0o700)
        scratch = output/'scratch'
        scratch.mkdir(mode=0o700)
        p.validate_prelaunch(output)
        for parent, name in ((output, 'execution'), (output, 'failure'), (scratch, p.CHILD), (scratch, 'orphan-proof')):
            marker = private_write(parent/name, b'collision')
            with self.assertRaises(ValueError): p.validate_prelaunch(output)
            marker.unlink()
        scratch.chmod(0o755)
        with self.assertRaises(ValueError): p.validate_prelaunch(output)

    def test_symlinks_raw_components_hardlinks_and_oversize(self):
        for value in ('relative', str(self.output)+'/./execution', str(self.output)+'/../attempt'):
            with self.assertRaises(ValueError): p.plain(value, directory=True)
        alias = Path(self.temp.name)/'alias'
        alias.symlink_to(self.output, target_is_directory=True)
        with self.assertRaises(ValueError): p.plain(alias/'execution', directory=True)
        held = self.child/'held'
        self.path.rename(held)
        self.path.symlink_to(held)
        with self.assertRaises(ValueError): self.validate()
        self.path.unlink()
        held.rename(self.path)
        os.link(self.path, self.base/'link')
        with self.assertRaises(ValueError): p.sidecar(self.output)
        (self.base/'link').unlink()
        with patch.object(p, 'MAX_BYTES', 2), self.assertRaises(ValueError): self.validate()


class Authorities(unittest.TestCase):
    """Made-up non-executable metadata, never actual execution authorities."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.stage = self.base/'stage'
        self.stage.mkdir(mode=0o700)
        self.here = self.stage/'scripts'
        self.here.mkdir(mode=0o700)
        for name in p.HOSTS: private_write(self.here/name, b'# synthetic source\n')
        rows = []
        for rel in p.SOURCES:
            path = self.stage/rel
            path.parent.mkdir(parents=True, exist_ok=True)
            private_write(path, b'// never compiled\n')
            rows.append({'path':rel, 'sha256':p.sha_bytes(path.read_bytes()), 'bytes':path.stat().st_size})
        def pin(path, obj):
            private_write(path, json.dumps(obj).encode())
            return {'path':str(path), 'sha256':p.sha_bytes(path.read_bytes())}
        native = pin(self.base/'synthetic-native', {'status':'compiled-source-closure-verified', 'build_id':BUILD, 'files':rows})
        self.normal = {'stage_root':str(self.stage), 'engine_build_id':BUILD,
                       'test_executable':{'path':str(self.base/'NOT-EXECUTABLE'), 'sha256':'0'*64},
                       'compiler_artifact':{'path':str(self.base/'NOT-ARTIFACT'), 'sha256':'1'*64},
                       'native_manifest':native, 'build_settings':{'synthetic':True},
                       'test_target':{'name':'mtg_core','kind':['lib']},
                       'legal_branch_source_sha256':rows[-1]['sha256'], 'legal_branch_contract_sha256':'c'*64,
                       'host_scripts':{str(self.here/n):p.sha_bytes((self.here/n).read_bytes()) for n in p.HOSTS[:-1]}}
        normal_pin = pin(self.base/'synthetic-inputs', self.normal)
        self.args = SimpleNamespace(authority=Path(normal_pin['path']), authority_sha256=normal_pin['sha256'],
                                    profile_authority=self.base/'synthetic-supplement', profile_authority_sha256=None,
                                    profile_helper_sha256=p.sha_bytes((self.here/p.HOSTS[-1]).read_bytes()),
                                    output=self.base/'new-attempt', case='ordinary_4', sweep='selected_detail')
        for flag, name in [('dense_driver_sha256',p.HOSTS[1]),('fixed_driver_sha256',p.HOSTS[2]),('helper_sha256',p.HOSTS[3])]:
            setattr(self.args, flag, self.normal['host_scripts'][str(self.here/name)])
        self.a = {'schema':'legal-proof-phase-authority-v1', 'normal_inputs':normal_pin,
                  **{k:self.normal[k] for k in ('stage_root','engine_build_id','test_executable','compiler_artifact','native_manifest','build_settings','test_target','legal_branch_contract_sha256')},
                  'host_sources':{str(self.here/n):p.sha_bytes((self.here/n).read_bytes()) for n in p.HOSTS},
                  'diagnostic_sources':{r['path']:r['sha256'] for r in rows}, 'output':str(self.args.output),
                  'entrypoint':'synthetic-entry', 'case':self.args.case, 'sweep':self.args.sweep,
                  'sidecar_schema':p.SCHEMA, 'collection_scope':'root_proofs_only', 'instrumentation':True,
                  'switch':{'LEGAL_BRANCH_PROOF_PHASE_V1':'1','LEGAL_BRANCH_OUTER_GUARD':'120s-2gib-v1'},
                  'policy':copy.deepcopy(p.POLICY)}
        self.write()

    def write(self):
        private_write(self.args.profile_authority, json.dumps(self.a).encode())
        self.args.profile_authority_sha256 = p.sha_bytes(self.args.profile_authority.read_bytes())

    def guard(self, fresh=True):
        return p.authority_guard(self.args,self.normal,self.here,self.stage,'synthetic-entry',fresh)

    def test_exact_binding_and_fresh_single_output(self):
        self.assertEqual(self.guard(), self.a)
        self.assertFalse(self.args.output.exists())
        self.args.output.mkdir(mode=0o700)
        with self.assertRaises(ValueError): self.guard()
        self.guard(False)
        self.args.output.chmod(0o755)
        with self.assertRaises(ValueError): self.guard(False)

    def test_all_caps_source_build_cargo_case_firstwrite_helper_output_pins(self):
        original = copy.deepcopy(self.a)
        edits = [lambda a:a.__setitem__('unknown',1), lambda a:a.__setitem__('schema','wrong'),
                 lambda a:a.__setitem__('engine_build_id','d'*64), lambda a:a.__setitem__('case','dense_4'),
                 lambda a:a.__setitem__('sweep','all_first_write'), lambda a:a.__setitem__('entrypoint','wrong'),
                 lambda a:a.__setitem__('instrumentation',1), lambda a:a.__setitem__('collection_scope','sweeps'),
                 lambda a:a.__setitem__('legal_branch_contract_sha256','d'*64),
                 lambda a:a['normal_inputs'].__setitem__('sha256','0'*64),
                 lambda a:a['test_executable'].__setitem__('sha256','2'*64),
                 lambda a:a['compiler_artifact'].__setitem__('path','/wrong'),
                 lambda a:a['build_settings'].__setitem__('synthetic',False),
                 lambda a:a['test_target'].__setitem__('name','mtg-sim'),
                 lambda a:a['diagnostic_sources'].pop(p.SOURCES[1]),
                 lambda a:a['diagnostic_sources'].__setitem__(p.SOURCES[0],'0'*64),
                 lambda a:a['host_sources'].__setitem__(str(self.here/p.HOSTS[-1]),'0'*64),
                 lambda a:a['switch'].__setitem__('LEGAL_BRANCH_OUTER_GUARD','relaxed'),
                 lambda a:a.__setitem__('output',str(self.base/'other'))]
        edits += [lambda a,key=key:a['policy'].__setitem__(key,None) for key in p.POLICY]
        for edit in edits:
            self.a = copy.deepcopy(original)
            edit(self.a)
            self.write()
            with self.assertRaises((ValueError,KeyError)): self.guard()

    def test_source_and_supplement_mode_bytes_and_input_changes(self):
        self.args.profile_authority_sha256 = '0'*64
        with self.assertRaises(ValueError): self.guard()
        self.write()
        self.args.profile_authority.chmod(0o644)
        with self.assertRaises(ValueError): self.guard()
        self.write()
        private_write(self.stage/p.SOURCES[0],b'changed')
        with self.assertRaises(ValueError): self.guard()

    def test_output_in_workspace_preexisting_symlink_and_private_parent(self):
        for output in (self.stage/'attempt', self.base/'already'):
            if output.name == 'already': output.symlink_to(self.stage, target_is_directory=True)
            self.args.output=output
            self.a['output']=str(output)
            self.write()
            with self.assertRaises(ValueError): self.guard()
        self.args.output=self.base/'new-attempt'
        self.a['output']=str(self.args.output)
        self.write()
        self.base.chmod(0o755)
        with self.assertRaises(ValueError): self.guard()


if __name__ == '__main__':
    unittest.main()
