"""Private TEST-HOST supplementary evidence, never product performance acceptance.

No local imports. Source is compiled from separately authenticated bytes by the
shrinking driver (including its local loader chain), never via cached bytecode.
The parent creates/reviews a NEW supplement only after fresh functional gates and
complete compiled source/executable identities. This module creates no authority.

Supplement exact contract is enforced by authority_guard below. All identity
objects copy the existing normal authority verbatim; normal_inputs pins its exact
file bytes. output names ONE fresh absolute attempt directory, not a reusable
parent. host_sources is the exact seven-file map, including this helper; the
three diagnostic_sources must also match the current compiled native manifest.
Repin the supplement SHA and helper SHA CLI arguments after any helper edit, and
repin changed host_scripts in the NEW normal inputs. Never rewrite old inputs.

CLI adds all four: --proof-phase-profile --profile-authority ABS_NEW_SUPPLEMENT
--profile-authority-sha256 SHA256 --profile-helper-sha256 SHA256, alongside ALL
existing shrinking arguments (including --execute and normal input/loader pins).
No authority generation or execution approval is implied by this candidate.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import stat

SCHEMA = 'generic-proof-phase-events-v1'
CHILD = 'generic-proof-phase-v1'
MAX_RECORDS, MAX_LINE, MAX_BYTES = 16_384, 1024, 16 * 1024 * 1024
U64 = (1 << 64) - 1
SOURCE_BASE = 'packages/game-engine/crates/sim/src/match_games/ordinary_diagnostic/'
SOURCES = tuple(SOURCE_BASE + name for name in (
    'generic_state_spool.rs', 'generic_state_spool_tests.rs', 'shrinking_choice_costs.rs'))
HOSTS = ('run-shrinking-choices-v1.py', 'run-definition-diversity-v1.py',
         'run-fixed-work.py', 'measurement_host.py', 'run-current-pods-v1.py',
         'run-ordinary-diagnostic-v1.py', 'generic_proof_profile.py')
POLICY = {'timeout_seconds': 120, 'memory_bytes': 2**31,
          'direct_rss_bytes': 2**31, 'swap_bytes': 0, 'tasks': 128,
          'systemd_runtime_max_seconds': 125, 'cpu_count': 1,
          'memory_events_zero': ['max', 'oom', 'oom_kill', 'oom_group_kill'],
          'attempts': 1, 'automatic_retries': 0, 'total_cells': 105,
          'all_repetitions': 630, 'living': [4, 3, 2]}
OPERAND_PHASES = ('canonical_export', 'rewind_and_bind', 'strict_pass1',
                  'typed_pass2_decode_spool', 'spool_finalize',
                  'generic_state_assembly', 'history_cleanup',
                  'exported_state_digest_validation', 'operand_snapshot_cleanup')
EQUALITY_PHASES = ('complete_generic_byte_eof_equality',
                   'production_state_digest_validation', 'operand_a_cleanup', 'operand_b_cleanup')
ROOT_PHASES = ('root_snapshot_export', 'root_standalone_restore', 'root_snapshot_cleanup')
PHASES = set(OPERAND_PHASES + EQUALITY_PHASES + ROOT_PHASES + ('state_equal', 'restored_disposal'))
BYTE_PHASES = {'canonical_export', 'strict_pass1', 'typed_pass2_decode_spool',
               'spool_finalize', 'generic_state_assembly', 'history_cleanup',
               'complete_generic_byte_eof_equality', 'root_snapshot_export', 'root_standalone_restore'}


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
    """No relaxed normal authority: this is an additional exact-key supplement.

    normal_inputs/test_executable/native_manifest/compiler_artifact are {path,
    sha256}. diagnostic_sources/host_sources map exact paths to SHA256. Remaining
    constants are literals below; a typo or extra field fails before launch.
    """
    hexsha(args.profile_authority_sha256)
    supplement_path = plain(args.profile_authority, mode=0o600)
    raw = bounded_bytes(supplement_path, 64 * 1024, mode=0o600)
    need(sha_bytes(raw) == args.profile_authority_sha256, 'profile supplement pin')
    a = decode(raw)
    shape(a, 'schema normal_inputs stage_root engine_build_id test_executable native_manifest compiler_artifact host_sources diagnostic_sources output entrypoint sidecar_schema collection_scope instrumentation switch policy')
    need(a['schema'] == 'shrinking-proof-phase-authority-v1', 'supplement schema')
    need(a['normal_inputs'] == {'path': str(args.authority), 'sha256': args.authority_sha256}, 'exact normal inputs binding')
    pin(a['normal_inputs'])
    need(a['stage_root'] == normal['stage_root'] == str(stage), 'stage binding')
    hexsha(a['engine_build_id'])
    need(a['engine_build_id'] == normal['engine_build_id'], 'Build binding')
    for key in ('test_executable', 'native_manifest', 'compiler_artifact'):
        shape(a[key], 'path sha256')
        need(a[key] == normal[key], 'current compiled identity binding')
        # The unchanged normal authority verifies the complete executable and
        # manifests. Read only the small manifest here; do not cap executable size.
    native = decode(pin(a['native_manifest']))
    need(native['status'] == 'compiled-source-closure-verified' and native['build_id'] == a['engine_build_id'], 'compiled source binding')
    rows = {row['path']: row for row in native['files']}
    need(type(a['diagnostic_sources']) is dict and set(a['diagnostic_sources']) == set(SOURCES), 'profile source namespace')
    for rel, expected in a['diagnostic_sources'].items():
        hexsha(expected)
        raw_source = bounded_bytes(stage / rel, 4 * 1024 * 1024)
        need(rows[rel]['sha256'] == expected == sha_bytes(raw_source)
             and type(rows[rel]['bytes']) is int and rows[rel]['bytes'] == len(raw_source), 'current compiled profile source')
    expected_hosts = {str(here / name) for name in HOSTS}
    need(type(a['host_sources']) is dict and set(a['host_sources']) == expected_hosts, 'profile helper namespace')
    for path, expected in a['host_sources'].items():
        pin({'path': path, 'sha256': expected}, 4 * 1024 * 1024)
        if Path(path).name != 'generic_proof_profile.py':
            need(normal['host_scripts'].get(path) == expected, 'normal host pin binding')
    need(a['host_sources'][str(here / 'generic_proof_profile.py')] == args.profile_helper_sha256, 'profile helper CLI binding')
    need(a['host_sources'][str(here / 'run-definition-diversity-v1.py')] == args.common_driver_sha256
         and a['host_sources'][str(here / 'run-fixed-work.py')] == args.fixed_driver_sha256
         and a['host_sources'][str(here / 'measurement_host.py')] == args.helper_sha256, 'loader CLI bindings')
    need(a['entrypoint'] == entry and a['sidecar_schema'] == SCHEMA
         and a['collection_scope'] == 'root_proofs_only' and a['instrumentation'] is True
         and a['switch'] == {'MTG_SHRINKING_PROOF_PHASE_V1': '1'}, 'explicit instrumentation contract')
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


def expected_events():
    """Actual SIM enum/scopes/order, including inclusive StateEqual parent."""
    def pair(living, operand, phase):
        return [(kind, living, operand, phase) for kind in ('begin', 'end')]
    events = []
    for living in (4, 3, 2):
        for phase in ROOT_PHASES:
            events.extend(pair(living, None, phase))
        events.append(('begin', living, None, 'state_equal'))
        for operand in ('a', 'b'):
            for phase in OPERAND_PHASES:
                events.extend(pair(living, operand, phase))
        for phase in EQUALITY_PHASES:
            events.extend(pair(living, None, phase))
        events.append(('end', living, None, 'state_equal'))
        events.extend(pair(living, None, 'restored_disposal'))
    return events


def validate_stream(stream, build, partial=False):
    """Bounded streaming facts. Partial never means accepted, even with terminal.

    Only ended ok phases count. A torn last line is excluded, never decoded.
    Error strings and raw bytes/paths from rejected input are never returned.
    Times are overlapping wall intervals, not additive CPU or memory costs.
    """
    expected = expected_events()
    result = {'status': 'partial', 'acceptance': False, 'records': 0,
              'bytes': 0, 'torn_tail_bytes': 0, 'terminal': None,
              'completed_phases': [], 'inclusive_intervals_overlap': True}
    digest = hashlib.sha256()
    stack, receipts = [], {}
    previous, position, failed = 0, 0, False
    try:
        while True:
            raw = stream.readline(MAX_LINE + 1)
            if not raw:
                break
            need(len(raw) <= MAX_LINE, 'line bound')
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
            need(type(e) is dict and e.get('schema') == SCHEMA and integer(e.get('seq')) == seq, 'schema/sequence')
            need(result['terminal'] is None, 'record after terminal')
            kind = e.get('kind')
            if seq == 0:
                shape(e, 'schema kind seq build_id collection_scope instrumentation')
                need(kind == 'header' and e['build_id'] == build
                     and e['collection_scope'] == 'root_proofs_only' and e['instrumentation'] is True, 'header binding')
            elif kind == 'terminal':
                shape(e, 'schema kind seq at_ns collection_outcome')
                need(integer(e['at_ns']) >= previous, 'monotonic terminal time')
                need(e['collection_outcome'] in ('root_collection_complete', 'error'), 'terminal outcome')
                result['terminal'] = e['collection_outcome']
                if e['collection_outcome'] == 'root_collection_complete':
                    need(not failed and not stack and position == len(expected), 'complete all root phases')
                else:
                    failed = True
            else:
                shape(e, 'schema kind seq scope phase at_ns duration_ns outcome bytes')
                need(kind in ('begin', 'end') and type(e['phase']) is str and e['phase'] in PHASES, 'declared phase/kind')
                scope = e['scope']
                shape(scope, 'living equality_ordinal operand')
                living = integer(scope['living'])
                need(living in (4, 3, 2) and integer(scope['equality_ordinal']) == 1
                     and scope['operand'] in ('a', 'b', None), 'declared scope')
                operand, phase = scope['operand'], e['phase']
                need((operand is not None) == (phase in OPERAND_PHASES), 'phase scope')
                at = integer(e['at_ns'])
                need(at >= previous, 'monotonic phase time')
                previous = at
                identity = (living, operand, phase)
                if kind == 'begin':
                    need(not failed and position < len(expected)
                         and expected[position] == (kind, *identity), 'phase sequence')
                    need(e['duration_ns'] is None and e['outcome'] is None and e['bytes'] is None, 'begin nulls')
                    stack.append((identity, at))
                    position += 1
                else:
                    need(stack and stack[-1][0] == identity, 'paired phase end')
                    _, begin = stack.pop()
                    need(integer(e['duration_ns']) <= at - begin, 'phase duration')
                    need(e['outcome'] in ('ok', 'error'), 'phase outcome')
                    if e['outcome'] == 'ok':
                        need(not failed and position < len(expected)
                             and expected[position] == (kind, *identity), 'successful phase sequence')
                        if phase in BYTE_PHASES:
                            count = integer(e['bytes'])
                            need(count > 0, 'successful byte receipt')
                            receipts[(living, operand, phase)] = count
                            prior = {'strict_pass1': 'canonical_export', 'typed_pass2_decode_spool': 'canonical_export',
                                     'history_cleanup': 'generic_state_assembly', 'root_standalone_restore': 'root_snapshot_export'}.get(phase)
                            if prior:
                                need(count == receipts[(living, operand, prior)], 'byte receipt binding')
                            if phase == 'complete_generic_byte_eof_equality':
                                need(count == receipts[(living, 'a', 'generic_state_assembly')]
                                     == receipts[(living, 'b', 'generic_state_assembly')], 'operand byte equality')
                        else:
                            need(e['bytes'] is None, 'inapplicable bytes')
                        result['completed_phases'].append({**scope, 'phase': phase, 'begin_at_ns': begin,
                                                           'end_at_ns': at, 'duration_ns': e['duration_ns'], 'bytes': e['bytes']})
                        position += 1
                    else:
                        need(e['bytes'] is None, 'error has no successful bytes')
                        failed = True
            result['records'] += 1
        need(result['records'] > 0, 'missing header')
        need(not failed and not stack and position == len(expected)
             and result['terminal'] == 'root_collection_complete' and not result['torn_tail_bytes'], 'incomplete/error collection')
        result['status'] = 'complete_collection_only'
    except (ValueError, KeyError, TypeError, UnicodeError, RecursionError) as error:
        if not partial:
            raise ValueError('invalid supplementary profile') from error
        result['status'] = 'failed_or_partial'
        result['validation_error'] = 'invalid_or_incomplete_profile'  # no private payload
    result['sha256'] = digest.hexdigest()
    result['unclosed_begins'] = [{'living': identity[0], 'equality_ordinal': 1,
                                 'operand': identity[1], 'phase': identity[2], 'begin_at_ns': at}
                                for identity, at in stack]
    result['hash_scope'] = 'all_file_bytes' if result['status'] == 'complete_collection_only' else 'bounded_inspected_prefix_only'
    return result


def sidecar(output):
    base = plain(output / 'execution', directory=True, mode=0o700)
    child = plain(base / CHILD, directory=True, mode=0o700)
    need({p.name for p in child.iterdir()} == {'events.ndjson'}, 'sidecar namespace/failure marker')
    path = plain(child / 'events.ndjson', mode=0o600)
    need(path.stat().st_size <= MAX_BYTES, 'sidecar size')
    return path


def validate_attempt(output, build):
    # No failure marker, orphan proof bytes, or extra sidecar can be ignored.
    plain(output, directory=True, mode=0o700)
    need({p.name for p in output.iterdir()} == {'tmp', 'execution'}, 'attempt namespace')
    tmp = plain(output / 'tmp', directory=True, mode=0o700)
    need(not any(tmp.iterdir()), 'proof scratch cleanup')
    base = plain(output / 'execution', directory=True, mode=0o700)
    files = {'host-report.json', 'payload-spec.json', 'launcher.stdout', 'launcher.stderr',
             'payload-ready.json', 'release', 'payload-result.json', 'workload.stdout',
             'workload.stderr', 'native-report.json'}
    need({p.name for p in base.iterdir()} == files | {CHILD}, 'execution namespace/failure marker')
    for name in files:
        plain(base / name, mode=0o600)
    path = sidecar(output)
    with path.open('rb') as stream:
        return validate_stream(stream, build)


def inspect_failed(output, build):
    result = {'scope': 'post_failure_owner_closed_OUTSIDE_120s_NOT_PASS',
              'status': 'failed_or_partial', 'acceptance': False}
    try:
        path = sidecar(output)
        with path.open('rb') as stream:
            result['profile'] = validate_stream(stream, build, partial=True)
        result['profile']['collection_observation'] = result['profile']['status']
        result['profile']['status'] = 'failed_or_partial'
    except (OSError, ValueError):
        result['profile'] = {'status': 'missing_or_unsafe_profile', 'acceptance': False}
    return result
