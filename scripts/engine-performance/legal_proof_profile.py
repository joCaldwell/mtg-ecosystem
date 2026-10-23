"""Private LEGAL test-host evidence; stdlib-only, authenticated source, no pyc.

The parent must create and independently review NEW normal inputs and a NEW
legal-proof-phase-authority-v1 supplement after fresh functional/compiled gates.
This module creates no authorities. Required CLI additions (all four):
--proof-phase-profile --profile-authority ABS --profile-authority-sha256 SHA
--profile-helper-sha256 SHA. Existing CLI pins/--execute remain mandatory.

Supplement exact keys and constants are enforced below. Identity objects copy
normal inputs verbatim; normal_inputs pins those exact bytes. diagnostic_sources
is the four-file CORE test-host map, host_sources the five-file local loader map.
output binds ONE fresh private attempt outside stage_root; case/sweep bind the
unchanged legal workload. Repin helper and driver/normal-input/supplement SHAs
following every edit. Never edit baseline authorities or retry an attempt.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import stat

SCHEMA = 'legal-proof-phase-events-v1'
CHILD = 'legal-proof-phase-v1'
MAX_RECORDS, MAX_LINE, MAX_BYTES = 16_384, 1024, 16 * 1024 * 1024
U64 = (1 << 64) - 1
SOURCE_BASE = 'packages/game-engine/crates/core/src/engine/'
SOURCES = tuple(SOURCE_BASE + name for name in (
    'search_reference_spool.rs', 'search_reference_spool_tests.rs',
    'work_optimization_tests.rs', 'legal_branch_tests.rs'))
HOSTS = ('run-legal-branch-v1.py', 'run-dense-diagnostic-v1.py',
         'run-fixed-work.py', 'measurement_host.py', 'legal_proof_profile.py')
CASES = tuple(f'{family}_{n}' for family in ('ordinary', 'dense') for n in (4, 3, 2))
POLICY = {'timeout_seconds': 120, 'memory_bytes': 2**31,
          'direct_rss_bytes': 2**31, 'swap_bytes': 0, 'tasks': 128,
          'systemd_runtime_max_seconds': 125, 'cpu_count': 1,
          'memory_events_zero': ['max', 'oom', 'oom_kill', 'oom_group_kill'],
          'attempts': 1, 'automatic_retries': 0, 'sweeps': 24,
          'retained_counts': [64, 256, 1024, 10000], 'warmups': 1,
          'measured_repetitions': 5, 'query_quantums': [1, 64, U64],
          'firstwrite_quantums': [1, U64], 'lazy_zero': True,
          'every_full_prefix_proved': True, 'cold_quantum1_units': [1, 5, 31, 129],
          'cold_quantum64_first_incomplete': True,
          'sampling_seed': 'reference-policy-seed',
          'firstwrite_policy': 'unchanged_legal_branch_source_contract'}
ROOT_PHASES = ('root_prepare', 'initial_generic_state_oracle',
               'first_write_action_selection', 'budget_first_write',
               'first_write_generic_state_oracle', 'snapshot_proof',
               'export_and_receipt', 'standalone_restore_inclusive',
               'read_receipt_and_state_digest_validation',
               'independent_accepted_input_replay', 'replay_generic_state_oracle',
               'final_root_digest')
GROUPS = ('reference_resume', 'actual_resume', 'metadata_conversion',
          'reference_serialize_write_hash', 'actual_serialize_compare_hash',
          'finish_eof_count_digest', 'sparse_cold_proof', 'checked_directory_cleanup')
SEARCH_PHASES = ('quantum_setup', 'lazy_zero', 'sparse_cold_save',
                 'sparse_saved_envelope_comparison', 'sparse_cold_restore',
                 'restored_scratch', 'completed_result') + GROUPS
PHASES = set(ROOT_PHASES + SEARCH_PHASES + ('query_proof',))
BYTE_PHASES = set(GROUPS[3:6]) | {'export_and_receipt'}
SCOPE_KEYS = 'case viewer query_kind quantum prefix_ordinal units_done history_entries proof_kind'


def need(ok, reason):
    if not ok:
        raise ValueError(reason)


def shape(value, keys):
    need(type(value) is dict and set(value) == set(keys.split()), 'exact fields')


def integer(value):
    need(type(value) is int and 0 <= value <= U64, 'u64 integer')
    return value


def sha_bytes(raw):
    return hashlib.sha256(raw).hexdigest()


def hexsha(value):
    need(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None, 'SHA256 codec')


def plain_name(value):
    need(type(value) is str and value.startswith('/') and '\0' not in value
         and all(p not in ('.', '..', '') for p in value.split('/')[1:]), 'plain absolute path')
    return Path(value)


def plain(path, directory=False, mode=None):
    path = plain_name(str(path))
    for ancestor in path.parents:
        need(stat.S_ISDIR(ancestor.lstat().st_mode), 'plain directory ancestry')
    info = path.lstat()
    need((stat.S_ISDIR if directory else stat.S_ISREG)(info.st_mode), 'plain file/directory')
    if mode is not None:
        need(stat.S_IMODE(info.st_mode) == mode, 'private exact mode')
        need(info.st_uid == os.geteuid(), 'private owner')
    if not directory:
        need(info.st_nlink == 1, 'single-link evidence file')
    return path


def bounded_bytes(path, limit, mode=None):
    path = plain(path, mode=mode)
    need(path.stat().st_size <= limit, 'bounded file')
    with path.open('rb') as stream:
        raw = stream.read(limit + 1)
    need(len(raw) <= limit, 'bounded file')
    return raw


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw.decode('utf-8'), object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def pin(row, limit=16 * 1024 * 1024):
    shape(row, 'path sha256')
    hexsha(row['sha256'])
    raw = bounded_bytes(plain_name(row['path']), limit)
    need(sha_bytes(raw) == row['sha256'], 'source/input pin')
    return raw


def authority_guard(args, normal, here, stage, entry, fresh=True):
    """Additional exact supplement; normal compiled/Cargo/resource checks stay mandatory."""
    hexsha(args.profile_authority_sha256)
    supplement_path = plain(args.profile_authority, mode=0o600)
    raw = bounded_bytes(supplement_path, 64 * 1024, mode=0o600)
    need(sha_bytes(raw) == args.profile_authority_sha256, 'profile supplement pin')
    a = decode(raw)
    shape(a, 'schema normal_inputs stage_root engine_build_id test_executable native_manifest compiler_artifact build_settings test_target host_sources diagnostic_sources output entrypoint case sweep legal_branch_contract_sha256 sidecar_schema collection_scope instrumentation switch policy')
    need(a['schema'] == 'legal-proof-phase-authority-v1', 'supplement schema')
    need(a['normal_inputs'] == {'path': str(args.authority), 'sha256': args.authority_sha256}, 'exact normal inputs binding')
    need(decode(pin(a['normal_inputs'])) == normal, 'normal input bytes binding')
    need(a['stage_root'] == normal['stage_root'] == str(stage), 'stage binding')
    hexsha(a['engine_build_id'])
    need(a['engine_build_id'] == normal['engine_build_id'], 'Build binding')
    for key in ('test_executable', 'native_manifest', 'compiler_artifact'):
        shape(a[key], 'path sha256')
        plain_name(a[key]['path'])
        hexsha(a[key]['sha256'])
        need(a[key] == normal[key], 'current compiled identity binding')
    for key in ('build_settings', 'test_target'):
        need(canonical(a[key]) == canonical(normal[key]), 'Cargo settings/target binding')
    # Normal authority_check verifies the executable, Cargo artifact, whole native
    # inventory and reconstructed Build ID. This additional small manifest ties
    # the four instrumented sources explicitly to that compiled closure.
    native = decode(pin(a['native_manifest']))
    need(native['status'] == 'compiled-source-closure-verified'
         and native['build_id'] == a['engine_build_id'], 'compiled source binding')
    rows = {row['path']: row for row in native['files']}
    need(len(rows) == len(native['files']), 'unique compiled paths')
    need(type(a['diagnostic_sources']) is dict and set(a['diagnostic_sources']) == set(SOURCES), 'profile source namespace')
    for rel, expected in a['diagnostic_sources'].items():
        hexsha(expected)
        source = bounded_bytes(stage / rel, 4 * 1024 * 1024)
        need(rows[rel]['sha256'] == expected == sha_bytes(source)
             and type(rows[rel]['bytes']) is int and rows[rel]['bytes'] == len(source), 'current compiled profile source')
    need(a['diagnostic_sources'][SOURCE_BASE + 'legal_branch_tests.rs'] == normal['legal_branch_source_sha256'], 'normal legal source binding')
    expected_hosts = {str(here / name) for name in HOSTS}
    need(type(a['host_sources']) is dict and set(a['host_sources']) == expected_hosts, 'profile helper namespace')
    for path, expected in a['host_sources'].items():
        pin({'path': path, 'sha256': expected}, 4 * 1024 * 1024)
        if Path(path).name != 'legal_proof_profile.py':
            need(normal['host_scripts'].get(path) == expected, 'normal host pin binding')
    for name, expected in (('legal_proof_profile.py', args.profile_helper_sha256),
                           ('run-dense-diagnostic-v1.py', args.dense_driver_sha256),
                           ('run-fixed-work.py', args.fixed_driver_sha256),
                           ('measurement_host.py', args.helper_sha256)):
        need(a['host_sources'][str(here / name)] == expected, 'loader CLI bindings')
    need(a['entrypoint'] == entry and a['case'] == args.case and args.case in CASES
         and a['sweep'] == args.sweep and args.sweep in ('selected_detail', 'all_first_write')
         and a['legal_branch_contract_sha256'] == normal['legal_branch_contract_sha256'], 'exact legal work/firstwrite contract')
    need(a['sidecar_schema'] == SCHEMA and a['collection_scope'] == 'root_proofs_only'
         and a['instrumentation'] is True
         and a['switch'] == {'LEGAL_BRANCH_PROOF_PHASE_V1': '1', 'LEGAL_BRANCH_OUTER_GUARD': '120s-2gib-v1'}, 'explicit instrumentation contract')
    need(canonical(a['policy']) == canonical(POLICY), 'unchanged work/caps/no retry contract')
    need(a['output'] == str(args.output), 'single new output binding')
    output = plain_name(a['output'])
    plain(output.parent, directory=True, mode=0o700)
    need(not output.is_relative_to(stage), 'private output outside source workspace')
    need(not args.authority.is_relative_to(output) and not supplement_path.is_relative_to(output), 'inputs outside attempt')
    if fresh:
        need(not os.path.lexists(output), 'fresh exclusive attempt')
    else:
        plain(output, directory=True, mode=0o700)
    return a


def scope(case, **fields):
    result = dict.fromkeys(SCOPE_KEYS.split())
    result.update(case=case, proof_kind='root')
    result.update(fields)
    return result


def sampled_prefixes(final, quantum):
    """O(64) receipts, even for a large history. ALL prefixes still execute."""
    values = {0, final}
    values.update(1 << k for k in range(64) if (1 << k) < final)
    values.update(p for p in ((1, 5, 31, 129) if quantum == 1 else (1,) if quantum == 64 else ()) if p < final)
    need(len(values) <= 70, 'sample context bound')
    return sorted(values)


def cold_count(prefix, final, quantum):
    return sum(p <= prefix and p < final for p in (1, 5, 31, 129)) if quantum == 1 else int(quantum == 64 and prefix >= 1 and final > 1)


def expected_counts(prefix, final, quantum):
    cold = cold_count(prefix, final, quantum)
    return [prefix, prefix, 1 + prefix + cold,
            1 + prefix + 2 * cold, 1 + prefix + 2 * cold,
            1 + prefix + 2 * cold, cold, int(prefix == final)]


def expected_events(case, histories):
    """Exact integrated Rust order. Histories is lazily bound by setup records.

    Validated normal ROOT_PROOF histories bind it on success. Partial inspection
    may learn only a declared setup history and never certifies work or success.
    Legal roots have 100*living objects, no stack/effects: reference observation
    consumes 4+objects units, then every history entry, four decks, one result.
    Resume scopes use the START cursor; scratch/checkpoints use RETURNED cursor.
    An unsampled final resume has no pair, but its completed_result always does.
    """
    def pair(phase, s):
        yield ('begin', phase, s)
        yield ('end', phase, s)
    def comparison(s):
        for phase in GROUPS[3:6]:
            yield from pair(phase, s)
    def scratch(s):
        yield from pair('metadata_conversion', s)
        yield from comparison(s)
    root = scope(case)
    for phase in ('root_prepare', 'initial_generic_state_oracle'):
        yield from pair(phase, root)
    living = int(case[-1])
    seats = {4: (0, 1, 2, 3), 3: (0, 1, 3), 2: (0, 3)}[living]
    for viewer in range(4):
        for kind in ('observe', 'decision', 'search'):
            if kind == 'search' and viewer not in seats:
                continue
            query = scope(case, viewer=viewer, query_kind=kind)
            yield ('begin', 'query_proof', query)
            if kind == 'search':
                for quantum in (1, 64, U64):
                    h = integer(histories[viewer])
                    units = integer(100 * living + h + 9)
                    final = (units - 1) // quantum + 1
                    base = dict(query, quantum=quantum, history_entries=h)
                    zero = dict(base, prefix_ordinal=0, units_done=0)
                    yield from pair('quantum_setup', zero)
                    yield ('begin', 'lazy_zero', zero)
                    yield from scratch(zero)
                    yield ('end', 'lazy_zero', zero)
                    yield ('checkpoint', None, zero)
                    for p in sampled_prefixes(final, quantum)[1:]:
                        start = dict(base, prefix_ordinal=p, units_done=(p - 1) * quantum)
                        returned = dict(start, units_done=min(p * quantum, units))
                        cold = p < final and ((quantum == 1 and p in (1, 5, 31, 129)) or (quantum == 64 and p == 1))
                        # Native switches to final sampling only AFTER both resumes.
                        resume_sampled = p & (p - 1) == 0 or (quantum == 1 and p in (1, 5, 31, 129)) or (quantum == 64 and p == 1)
                        if resume_sampled:
                            yield from pair('reference_resume', start)
                            yield from pair('actual_resume', start)
                        if p == final:
                            yield ('begin', 'completed_result', returned)
                            yield from scratch(returned)
                            yield ('end', 'completed_result', returned)
                            yield from pair('checked_directory_cleanup', returned)
                        else:
                            yield from scratch(returned)
                            if cold:
                                yield ('begin', 'sparse_cold_proof', returned)
                                yield from pair('sparse_cold_save', returned)
                                yield ('begin', 'sparse_saved_envelope_comparison', returned)
                                yield from comparison(returned)
                                yield ('end', 'sparse_saved_envelope_comparison', returned)
                                yield from pair('sparse_cold_restore', returned)
                                yield ('begin', 'restored_scratch', returned)
                                yield from scratch(returned)
                                yield ('end', 'restored_scratch', returned)
                                yield ('end', 'sparse_cold_proof', returned)
                        yield ('checkpoint', None, returned)
            yield ('end', 'query_proof', query)
    yield from pair('first_write_action_selection', root)
    for quantum in (1, U64):
        yield from pair('budget_first_write', dict(root, proof_kind='firstwrite', quantum=quantum))
    yield from pair('first_write_generic_state_oracle', dict(root, proof_kind='firstwrite'))
    for proof_kind in ('root', 'firstwrite'):
        s = dict(root, proof_kind=proof_kind)
        yield ('begin', 'snapshot_proof', s)
        for phase in ('export_and_receipt', 'standalone_restore_inclusive',
                      'read_receipt_and_state_digest_validation', 'checked_directory_cleanup'):
            yield from pair(phase, s)
        yield ('end', 'snapshot_proof', s)
        yield from pair('independent_accepted_input_replay', s)
        yield from pair('replay_generic_state_oracle', s)
    yield from pair('final_root_digest', root)


def validate_stream(stream, build, case, histories=None, partial=False):
    """Streaming bounded receipts, never acceptance. No private payload returned.

    Eight counters include unsampled execution. Sampled ends are lower bounds,
    never the completed-work denominator. Inclusive clocks overlap. Complete
    collection concerns roots only, not 24 sweeps or product performance.
    """
    hexsha(build)
    need(case in CASES, 'declared case')
    known = {} if histories is None else {i: integer(v) for i, v in enumerate(histories)}
    if histories is not None:
        need(len(histories) == 4, 'four histories')
    expected = expected_events(case, known)
    result = {'status': 'failed_or_partial', 'acceptance': False, 'records': 0,
              'bytes': 0, 'torn_tail_bytes': 0, 'terminal': None,
              'completed_phases': [], 'checkpoints': [],
              'inclusive_intervals_overlap': True,
              'sampled_receipts_are_not_sampled_execution': True}
    digest = hashlib.sha256()
    stack, sampled, prior_totals = [], [[0, 0, 0] for _ in GROUPS], [[0, 0, 0] for _ in GROUPS]
    prior_sampled = [[0, 0, 0] for _ in GROUPS]
    previous, failed, receipt = 0, False, None
    try:
        while True:
            raw = stream.readline(MAX_LINE + 1)
            if not raw:
                break
            need(len(raw) <= MAX_LINE, 'newline-inclusive line bound')
            need(result['bytes'] + len(raw) <= MAX_BYTES, 'file bound')
            result['bytes'] += len(raw)
            digest.update(raw)
            if not raw.endswith(b'\n'):
                need(partial, 'torn final line')
                result['torn_tail_bytes'] = len(raw)
                break
            seq = result['records']
            need(seq < MAX_RECORDS, 'record bound')
            e = decode(raw)
            need(type(e) is dict and e.get('schema') == SCHEMA
                 and integer(e.get('seq')) == seq, 'schema/sequence')
            need(result['terminal'] is None, 'record after terminal')
            kind = e.get('kind')
            if seq == 0:
                shape(e, 'schema kind seq build_id collection_scope instrumentation')
                need(kind == 'header' and e['build_id'] == build
                     and e['collection_scope'] == 'root_proofs_only'
                     and e['instrumentation'] is True, 'header binding')
            else:
                at = integer(e.get('at_ns'))
                need(at >= previous, 'monotonic time')
                previous = at
                if kind == 'terminal':
                    shape(e, 'schema kind seq at_ns collection_outcome')
                    need(e['collection_outcome'] in ('root_collection_complete', 'error'), 'terminal outcome')
                    result['terminal'] = e['collection_outcome']
                    if e['collection_outcome'] == 'root_collection_complete':
                        need(not failed and not stack and next(expected, None) is None, 'all root phases complete')
                    else:
                        failed = True
                else:
                    s = e.get('scope')
                    shape(s, SCOPE_KEYS)
                    need(s['case'] == case and s['proof_kind'] in ('root', 'firstwrite'), 'declared proof context')
                    for key in ('viewer', 'quantum', 'prefix_ordinal', 'units_done', 'history_entries'):
                        if s[key] is not None:
                            integer(s[key])
                    need(s['query_kind'] in (None, 'observe', 'decision', 'search'), 'query kind')
                    # Learn a partial scope only at the first real Search setup.
                    if kind == 'begin' and e.get('phase') == 'quantum_setup':
                        v = integer(s['viewer'])
                        need(v < 4, 'viewer')
                        h = integer(s['history_entries'])
                        need(v not in known or known[v] == h, 'normal root history binding')
                        known[v] = h
                        sampled = [[0, 0, 0] for _ in GROUPS]
                        prior_totals = [[0, 0, 0] for _ in GROUPS]
                        prior_sampled = [[0, 0, 0] for _ in GROUPS]
                    if kind == 'checkpoint':
                        shape(e, 'schema kind seq scope at_ns last_completed_prefix totals')
                        need(not failed and len(stack) == 1 and stack[0][0] == 'query_proof', 'checkpoint outside prefix work')
                        need(next(expected, None) == (kind, None, s), 'checkpoint sequence/context')
                        p = integer(e['last_completed_prefix'])
                        need(p == s['prefix_ordinal'], 'checkpoint cursor')
                        q = integer(s['quantum'])
                        units = integer(100 * int(case[-1]) + integer(s['history_entries']) + 9)
                        final = (units - 1) // q + 1
                        counts = expected_counts(p, final, q)
                        rows = e['totals']
                        need(type(rows) is list and len(rows) <= 8, 'eight compact totals')
                        ids, totals = [], [[0, 0, 0] for _ in GROUPS]
                        for row in rows:
                            need(type(row) is list and len(row) == 4, 'compact tuple')
                            group, calls, duration, count = [integer(v) for v in row]
                            need(group < 8 and calls > 0, 'nonempty declared group')
                            ids.append(group)
                            totals[group] = [calls, duration, count]
                        need(ids == [i for i, n in enumerate(counts) if n], 'ordered exact nonzero groups')
                        for i, total in enumerate(totals):
                            need(total[0] == counts[i], 'all-prefix completed count')
                            need(total[1] <= at, 'completed nonoverlapping group time')
                            if i not in (3, 4, 5):
                                need(total[2] == 0, 'group has no bytes')
                            for k in range(3):
                                delta = total[k] - prior_totals[i][k]
                                observed = sampled[i][k] - prior_sampled[i][k]
                                need(delta >= observed, 'all-prefix totals retain sampled facts')
                                if total[0] == sampled[i][0]:
                                    need(total[k] == sampled[i][k], 'fully observed totals')
                        need(totals[3][2] == totals[4][2] == totals[5][2], 'full comparator byte equality')
                        prior_totals = totals
                        prior_sampled = [row[:] for row in sampled]
                        result['checkpoints'].append({**s, 'at_ns': at, 'totals': rows})
                    else:
                        shape(e, 'schema kind seq scope phase at_ns duration_ns outcome bytes')
                        phase = e['phase']
                        need(kind in ('begin', 'end') and type(phase) is str and phase in PHASES, 'phase/kind')
                        if kind == 'begin':
                            need(not failed and next(expected, None) == (kind, phase, s), 'phase sequence/context')
                            need(e['duration_ns'] is None and e['outcome'] is None and e['bytes'] is None, 'begin nulls')
                            stack.append((phase, s, at))
                        else:
                            need(stack and stack[-1][:2] == (phase, s), 'paired phase scope')
                            _, _, begin = stack.pop()
                            duration = integer(e['duration_ns'])
                            need(duration <= at - begin, 'duration interval')
                            need(e['outcome'] in ('ok', 'error'), 'phase outcome')
                            if e['outcome'] == 'ok':
                                need(not failed and next(expected, None) == (kind, phase, s), 'successful phase sequence')
                                if phase in BYTE_PHASES:
                                    count = integer(e['bytes'])
                                    need(count > 0, 'successful byte receipt')
                                    if phase == GROUPS[3]:
                                        receipt = count
                                    elif phase in GROUPS[4:6]:
                                        need(count == receipt, 'comparator byte receipt binding')
                                else:
                                    need(e['bytes'] is None, 'inapplicable bytes')
                                if phase in GROUPS and s['query_kind'] == 'search' and s['quantum'] is not None:
                                    total = sampled[GROUPS.index(phase)]
                                    for k, add in enumerate((1, duration, e['bytes'] or 0)):
                                        total[k] = integer(total[k] + add)
                                result['completed_phases'].append({**s, 'phase': phase, 'begin_at_ns': begin,
                                                                  'end_at_ns': at, 'duration_ns': duration, 'bytes': e['bytes']})
                            else:
                                need(e['bytes'] is None, 'error has no successful bytes')
                                failed = True
            result['records'] += 1
        need(result['records'] > 0 and not failed and not stack
             and result['terminal'] == 'root_collection_complete'
             and not result['torn_tail_bytes'], 'incomplete/error collection')
        result['status'] = 'complete_collection_only'
    except (ValueError, KeyError, TypeError, UnicodeError, RecursionError, ZeroDivisionError) as error:
        if not partial:
            raise ValueError('invalid supplementary profile') from error
        result['validation_error'] = 'invalid_or_incomplete_profile'
    result['sha256'] = digest.hexdigest()
    result['unclosed_begins'] = [{**s, 'phase': phase, 'begin_at_ns': at} for phase, s, at in stack]
    result['hash_scope'] = 'all_file_bytes' if result['status'] == 'complete_collection_only' else 'bounded_inspected_prefix_only'
    return result


EXECUTION_FILES = {'host-report.json', 'payload-spec.json', 'launcher.stdout', 'launcher.stderr',
                   'payload-ready.json', 'release', 'payload-result.json', 'workload.stdout', 'workload.stderr'}


def validate_prelaunch(output):
    plain(output, directory=True, mode=0o700)
    need({p.name for p in output.iterdir()} == {'scratch'}, 'fresh prelaunch namespace')
    scratch = plain(output / 'scratch', directory=True, mode=0o700)
    need(not any(scratch.iterdir()), 'no preexisting proof/sidecar/failure files')


def sidecar(output):
    base = plain(output / 'scratch', directory=True, mode=0o700)
    child = plain(base / CHILD, directory=True, mode=0o700)
    need({p.name for p in child.iterdir()} == {'events.ndjson'}, 'sidecar namespace/failure marker')
    path = plain(child / 'events.ndjson', mode=0o600)
    need(path.stat().st_size <= MAX_BYTES, 'sidecar size')
    return path


def validate_attempt(output, build, case, normal_audit):
    plain(output, directory=True, mode=0o700)
    need({p.name for p in output.iterdir()} == {'scratch', 'execution'}, 'attempt namespace')
    scratch = plain(output / 'scratch', directory=True, mode=0o700)
    need({p.name for p in scratch.iterdir()} == {CHILD}, 'proof scratch cleanup/failure marker')
    base = plain(output / 'execution', directory=True, mode=0o700)
    need({p.name for p in base.iterdir()} == EXECUTION_FILES, 'execution namespace/failure marker')
    for name in EXECUTION_FILES:
        plain(base / name, mode=0o600)
    need(normal_audit['root_proof_verified'] is True and normal_audit['native_final_received'] is True
         and normal_audit['completed_sweeps'] == 24, 'full normal audit required')
    path = sidecar(output)
    with path.open('rb') as stream:
        return validate_stream(stream, build, case, normal_audit['root']['histories'])


def inspect_failed(output, build, case):
    """Caller must prove failed attempt's owner closure; never promotes a pass."""
    result = {'scope': 'post_failure_owner_closed_OUTSIDE_120s_NOT_PASS',
              'status': 'failed_or_partial', 'acceptance': False}
    try:
        path = sidecar(output)
        with path.open('rb') as stream:
            result['profile'] = validate_stream(stream, build, case, partial=True)
        result['profile']['collection_observation'] = result['profile']['status']
        result['profile']['status'] = 'failed_or_partial'
    except (OSError, ValueError):
        result['profile'] = {'status': 'missing_or_unsafe_profile', 'acceptance': False}
    return result
