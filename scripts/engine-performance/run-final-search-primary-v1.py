#!/usr/bin/env python3
"""Portable primary60 host. Explicit authenticated authority and --quiescent only.

Original600s native suite guard; native5s/root,3s p95,wholeRSS2GiB unchanged.
No build/authority approval or execution on import. Never retries an attempt.
"""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import os
import signal
import subprocess
import time

ROOT = None
INPUTS = None
AUTHORITY = None
FROZEN = None
NATIVE_MANIFEST = None
LIMIT = 2 * 1024**3
CPU_IDS = [0, 1, 2, 3]
STOP = False
BENCHMARK_ENV = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}
IDLE_SECONDS = 600
IDLE_SAMPLE_SECONDS = 2
CPU_TICKS = os.sysconf('SC_CLK_TCK')
IDLE_CPU_DENOMINATOR = 100  # Aggregate <=1% of ONE core, not four cores.
CPU_FIELDS = ('user_ticks', 'system_ticks', 'waited_child_user_ticks', 'waited_child_system_ticks')
os.umask(0o077)


def require(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    with Path(path).open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def read(path):
    return INPUTS.read(path)


def stopped(_signal, _frame):
    global STOP
    STOP = True


def heavy(name, command):
    names = {'cargo', 'rustc', 'mtg-sim', 'mtg-server', 'component', 'node',
             'node-MainThread', 'cc1', 'cc1plus', 'cc', 'clang', 'clang++',
             'ld', 'ld.bfd', 'ld.lld', 'rust-lld', 'lld', 'mold', 'collect2'}
    project_python = name.startswith('python') and (
        '/mtg-engine-plan/' in command or '/tmp/mtg-' in command)
    # Modern Node names the main process node-MainThread; worker/process names
    # can use other node-* suffixes. Match that bounded prefix, not arbitrary
    # names merely containing "node". Unrelated user jobs are observed only.
    return name in names or name == 'nodejs' or name.startswith('node-') or project_python


def node_process(name):
    return name in {'node', 'nodejs'} or name.startswith('node-')


def process_row(pid, name, stat):
    fields = stat.rsplit(') ', 1)[1].split()
    # Linux proc_pid_stat(5), fields14..17 and22. No argv enters evidence.
    cpu = {key: int(fields[index]) for key, index in
           [('user_ticks', 11), ('system_ticks', 12),
            ('waited_child_user_ticks', 13), ('waited_child_system_ticks', 14)]}
    require(all(value >= 0 for value in cpu.values()), 'nonnegative process CPU counters')
    return {'pid': pid, 'name': name, 'state': fields[0],
            'start_ticks': int(fields[19]), **cpu, 'cpu_ticks': sum(cpu.values())}


def inventory(exclude=()):
    excluded = {os.getpid(), *exclude}
    rows = []
    for path in Path('/proc').iterdir():
        if not path.name.isdigit() or int(path.name) in excluded:
            continue
        try:
            name = (path / 'comm').read_text().strip()
            stat = (path / 'stat').read_text()
            command = (path / 'cmdline').read_bytes().replace(b'\0', b' ').decode(errors='replace')
            row = process_row(int(path.name), name, stat)
            if row['state'] not in {'T', 't', 'Z'} and heavy(name, command):
                rows.append(row)
        except (OSError, IndexError, ValueError):
            pass
    return sorted(rows, key=lambda row: row['pid'])


def uptime_ticks():
    return int(float(Path('/proc/uptime').read_text().split()[0]) * CPU_TICKS)


class IdleBaseline:
    """Only proved idle, old Node daemons may coexist with this measurement.

    Every adjacent inventory window AND the complete elapsed window is checked.
    This is sampled quiescence, not a claim that arbitrary sub-sample processes
    cannot exist. Waited-child CPU is charged to catch short completed children.
    """
    def __init__(self, first, second, first_uptime, second_uptime,
                 sample_elapsed_ns, started_ns, ticks=CPU_TICKS):
        require(isinstance(ticks, int) and ticks > 0, 'positive kernel clock tick rate')
        for row in first + second:
            require(all(type(row[key]) is int and row[key] >= 0 for key in CPU_FIELDS) and
                    row['cpu_ticks'] == sum(row[key] for key in CPU_FIELDS), 'exact raw process CPU counters')
        require(sample_elapsed_ns >= IDLE_SAMPLE_SECONDS * 1_000_000_000,
                'idle baseline requires two seconds between samples')
        before = {row['pid']: row for row in first}
        after = {row['pid']: row for row in second}
        require(len(before) == len(first) and len(after) == len(second) and
                before.keys() == after.keys(), 'heavy process inventory changed during idle preflight')
        for pid, old in before.items():
            new = after[pid]
            require(node_process(old['name']) and old['name'] == new['name'] and
                    old['state'] == new['state'] == 'S' and
                    old['start_ticks'] == new['start_ticks'],
                    'only sleeping same-identity Node daemons can baseline')
            require(first_uptime - old['start_ticks'] >= IDLE_SECONDS * ticks and
                    second_uptime - new['start_ticks'] >= IDLE_SECONDS * ticks,
                    'recent Node process cannot baseline')
            require(all(old[key] == new[key] for key in CPU_FIELDS),
                    'active Node process cannot baseline')
        self.baseline = after
        self.previous = after
        self.started_ns = self.previous_ns = started_ns
        self.ticks = ticks
        self.data = {'policy': 'node-idle600s-zero2s-self-and-waited-child-aggregate-1pct-one-core-v1',
                     'clock_ticks_per_second': ticks, 'minimum_age_seconds': IDLE_SECONDS,
                     'preflight_sample_elapsed_ns': sample_elapsed_ns,
                     'preflight_uptime_ticks': [first_uptime, second_uptime],
                     'aggregate_cpu_limit_percent_of_one_core': 1,
                     'aggregate_cpu_denominator': IDLE_CPU_DENOMINATOR,
                     'first_preflight_processes': first,
                     'baseline_processes': second, 'samples': [], 'met': True}

    def sample(self, rows, now_ns):
        current = {row['pid']: row for row in rows}
        errors = []
        if any(not all(type(row[key]) is int and row[key] >= 0 for key in CPU_FIELDS) or
               row['cpu_ticks'] != sum(row[key] for key in CPU_FIELDS) for row in rows):
            errors.append('invalid raw process CPU counters')
        if len(current) != len(rows) or current.keys() != self.baseline.keys():
            errors.append('new heavy process or missing idle baseline process')
        for pid, old in self.baseline.items():
            new = current.get(pid)
            if new is None:
                continue
            if new['start_ticks'] != old['start_ticks'] or new['name'] != old['name']:
                errors.append('baseline PID/starttime/name changed')
            if new['state'] in {'T', 't', 'Z', 'X', 'x'}:
                errors.append('baseline process is no longer alive and running or sleeping')
            if any(new[key] < self.previous[pid][key] for key in CPU_FIELDS):
                errors.append('baseline CPU counter decreased')
        elapsed_ns = now_ns - self.started_ns
        window_ns = now_ns - self.previous_ns
        if window_ns <= 0 or elapsed_ns <= 0:
            errors.append('nonpositive inventory elapsed window')
        delta = sum(row['cpu_ticks'] for row in current.values()) - sum(row['cpu_ticks'] for row in self.previous.values())
        total = sum(row['cpu_ticks'] for row in current.values()) - sum(row['cpu_ticks'] for row in self.baseline.values())
        if delta < 0 or total < 0 or delta * IDLE_CPU_DENOMINATOR * 1_000_000_000 > window_ns * self.ticks:
            errors.append('aggregate baseline CPU exceeds1% one core in sampled window')
        if total * IDLE_CPU_DENOMINATOR * 1_000_000_000 > elapsed_ns * self.ticks:
            errors.append('aggregate baseline CPU exceeds1% one core overall')
        per_process = [{'pid': pid, 'start_ticks': row['start_ticks'],
                        'window_cpu_ticks': row['cpu_ticks'] - self.previous[pid]['cpu_ticks']
                        if pid in self.previous else None,
                        'total_cpu_ticks': row['cpu_ticks'] - self.baseline[pid]['cpu_ticks']
                        if pid in self.baseline else None}
                       for pid, row in sorted(current.items())]
        proof = {'elapsed_ns': elapsed_ns, 'window_ns': window_ns,
                 'aggregate_window_cpu_ticks': delta, 'aggregate_total_cpu_ticks': total,
                 'per_process_deltas': per_process,
                 'processes': rows, 'errors': errors, 'met': not errors}
        self.data['samples'].append(proof)
        self.data['met'] = self.data['met'] and not errors
        self.previous, self.previous_ns = current, now_ns
        require(not errors, '; '.join(errors))
        return proof


def idle_preflight():
    began = time.monotonic_ns()
    first_uptime = uptime_ticks()
    first = inventory()
    time.sleep(IDLE_SAMPLE_SECONDS)
    require(not STOP, 'cancelled during idle preflight')
    second_uptime = uptime_ticks()
    second = inventory()
    now = time.monotonic_ns()
    return IdleBaseline(first, second, first_uptime, second_uptime, now - began, now)


def topology():
    rows = []
    allowed = sorted(os.sched_getaffinity(0))
    for cpu in CPU_IDS:
        base = Path(f'/sys/devices/system/cpu/cpu{cpu}/topology')
        rows.append((int((base / 'physical_package_id').read_text()),
                     int((base / 'core_id').read_text())))
    validate_topology(allowed, rows)
    return {'allowed_cpu_ids': allowed, 'worker_cpu_ids': CPU_IDS,
            'physical_package_and_core_ids': rows}


def validate_topology(allowed, rows):
    require(all(cpu in allowed for cpu in CPU_IDS), 'all four worker CPUs must be allowed')
    require(len(rows) == 4 and len(set(rows)) == 4, 'four distinct physical cores required')


def load_inputs(expected):
    import importlib.util
    path = Path(__file__).absolute().with_name('host_inputs.py')
    require(not path.is_symlink() and sha(path) == expected, 'input guard SHA before import')
    spec = importlib.util.spec_from_file_location('authenticated_primary_inputs', path)
    out = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(out)
    return out


def frozen(args):
    global ROOT, AUTHORITY, NATIVE_MANIFEST, FROZEN
    AUTHORITY, NATIVE_MANIFEST, proof = INPUTS.check_search(args.authority, args.authority_sha256, Path(__file__))
    ROOT = Path(AUTHORITY['stage_root'])
    FROZEN = proof
    return proof


def predecessor():
    return FROZEN['predecessors']


def summarize(report, out, expected_build, external_peak):
    require(report['build_id'] == expected_build, 'measured current build')
    require(type(report['numeric_gate_met']) is bool, 'typed native gate flag')
    require(type(external_peak) is int and external_peak > 0, 'positive actual whole-process kernel peak')
    jobs = report['jobs']
    require(report['attempted_jobs'] == report['reported_job_latencies'] == len(jobs) == 60,
            'all sixty latency denominators required; faults cannot disappear')
    require({(job['index'], job['repetition']) for job in jobs} ==
            {(index, repeat) for index in range(12) for repeat in range(5)}, 'exact fixed sixty roots')
    for job in jobs:
        require(all(type(job[key]) is int and job[key] >= 0 for key in ['index', 'repetition', 'elapsed_micros', 'maximum_process_rss_bytes']), 'typed job identities and metrics')
        name = f"root-{job['index']:02}-repeat-{job['repetition']}.json"
        require(job['evidence'] == name, 'fixed safe evidence filename')
        evidence = read(out / name)
        root = evidence['report']
        require(evidence['build_id'] == expected_build and root['request']['root_index'] == job['index'],
                'root evidence identity')
        require(all(type(cpu) is int for cpu in root['request']['worker_cpu_ids'] + root['actual_worker_cpu_ids']), 'typed physical CPU IDs')
        require(root['request']['worker_cpu_ids'] == root['actual_worker_cpu_ids'] == CPU_IDS and
                job['worker_cpu_ids_verified'], 'four actual designated worker CPUs')
        require(root['elapsed_micros'] == job['elapsed_micros'] and
                root['maximum_process_rss_bytes'] == job['maximum_process_rss_bytes'], 'reported lifecycle metrics')
        require(root['minimum_met'] and job['minimum_met'] and root['aggregates'] and
                all(row['minimum_met'] and row['useful_distinct_worlds'] >= 8 for row in root['aggregates']),
                'eight useful distinct worlds for every candidate')
    latencies = sorted(job['elapsed_micros'] for job in jobs)
    require(report['p95_nearest_rank'] == 57 and report['p95_elapsed_micros'] == latencies[56],
            'nearest-rank p95 over all sixty jobs')
    peak = max(external_peak, *(job['maximum_process_rss_bytes'] for job in jobs))
    return {'p95_elapsed_micros': latencies[56], 'whole_process_peak_upper_bound_bytes': peak,
            'whole_process_memory_met': peak <= LIMIT,
            'all_hard_latency_met': max(latencies) <= 5_000_000,
            'p95_met': latencies[56] <= 3_000_000,
            'native_numeric_gate_met': report['numeric_gate_met']}


def reap(child, reaped):
    # The child is our own session leader. Always stop leftover descendants,
    # even after the direct benchmark exits normally. Never detach a job.
    usage = None
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        try:
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            pass
        if not reaped:
            until = time.monotonic() + 5
            while time.monotonic() < until:
                pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                if pid:
                    reaped = True
                    child.returncode = os.waitstatus_to_exitcode(status)
                    break
                time.sleep(.02)
    # Never enter an unbounded wait after both bounded TERM/KILL windows.
    # A stuck/unreaped child is an explicit failure with its PID retained;
    # no successful cleanup/retry/acceptance claim is made.
    return reaped, usage


def spawn_benchmark(inputs, out, stdout, stderr):
    return subprocess.Popen([inputs['executable'], 'search-benchmark', str(out), '--quiescent'],
                            cwd=ROOT, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                            start_new_session=True, env=dict(BENCHMARK_ENV))


def finalize_report(result, wrapper):
    if STOP:
        result.update(status='failed', error='interrupted through cleanup/final publication')
    INPUTS.save(wrapper, result)
    if STOP and result['status'] == 'measurement_passed_replay_pending':
        result.update(status='failed', error='interrupted during final publication')
        INPUTS.save(wrapper, result)


def _main():
    signal.signal(signal.SIGTERM, stopped)
    signal.signal(signal.SIGINT, stopped)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--authority', type=Path, required=True)
    parser.add_argument('--authority-sha256', required=True)
    parser.add_argument('--inputs-sha256', required=True)
    parser.add_argument('--run-directory', type=Path, required=True)
    parser.add_argument('--quiescent', action='store_true', required=True)
    args = parser.parse_args()
    global INPUTS
    INPUTS = load_inputs(args.inputs_sha256)
    # Validate all source/executable/prerequisite inputs BEFORE any output.
    inputs = frozen(args)
    prerequisites = predecessor()
    out = INPUTS.fresh(args.run_directory, AUTHORITY['run_parent'])
    wrap = out.with_name(out.name + '-wrapper.json')
    require(not wrap.exists() and not wrap.is_symlink(), 'fresh wrapper path')
    expected_build, expected_simulator = AUTHORITY['build_id'], AUTHORITY['cli']['sha256']
    require(AUTHORITY['primary_wrapper'] == {'path': str(Path(__file__).absolute()), 'sha256': sha(Path(__file__))}, 'actual primary wrapper authority')
    physical = topology()
    idle = idle_preflight()
    initial = idle.data['baseline_processes']
    require(not STOP, 'stop active heavy/HTTP/client workloads first')
    driver_sha = sha(Path(__file__))
    result = {'status': 'prepared', 'classification': 'current-build declared-quiescent primary60; replay pending',
              'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'driver_sha256': driver_sha, 'build_id': expected_build,
              'executable_sha256': expected_simulator, 'frozen_inputs': inputs,
              'predecessor_proofs': prerequisites, 'physical_cpu_topology': physical,
              'authority': {'path': str(args.authority), 'sha256': args.authority_sha256},
              'guard_scopes': {'native_suite_seconds': 600, 'native_root_hard_micros': 5000000,
                               'metadata_audit_after_native_suite': True},
              'runtime_environment': dict(BENCHMARK_ENV),
              'product_memory_bytes': LIMIT, 'observed_process_rss_bytes': 0,
              'observed_process_hwm_bytes': 0, 'other_heavy_process_observations': [],
              'initial_other_heavy_processes': initial, 'idle_baseline': idle.data, 'kernel': os.uname().release,
              'cpu_before': Path('/proc/stat').read_text().splitlines()[0],
              'meminfo_before': Path('/proc/meminfo').read_text()}
    def dump():
        temporary = wrap.with_name(wrap.name + '.tmp')
        temporary.write_text(json.dumps(result, indent=2) + '\n')
        temporary.replace(wrap)
    child, reaped, usage = None, False, None
    started, last_inventory = time.monotonic(), 0
    try:
        dump()
        with out.with_name(out.name + '.stdout.json').open('xb') as stdout, \
                out.with_name(out.name + '.stderr.log').open('xb') as stderr:
            require(not STOP, 'measurement cancelled before spawn')
            child = spawn_benchmark(inputs, out, stdout, stderr)
            result.update(status='running', pid=child.pid)
            dump()
            while True:
                pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                if pid:
                    reaped = True
                    child.returncode = os.waitstatus_to_exitcode(status)
                    break
                require(not STOP, 'measurement interrupted')
                require(time.monotonic() - started <= 600, 'suite guard600s; per-root5s unchanged')
                try:
                    rows = dict(row.split(':', 1) for row in Path(f'/proc/{child.pid}/status').read_text().splitlines() if ':' in row)
                    for key, name in [('observed_process_rss_bytes', 'VmRSS'), ('observed_process_hwm_bytes', 'VmHWM')]:
                        result[key] = max(result[key], int(rows.get(name, '0 kB').split()[0]) * 1024)
                except OSError:
                    pass
                require(max(result['observed_process_rss_bytes'], result['observed_process_hwm_bytes']) <= LIMIT, 'whole process2GiB observed limit')
                if time.monotonic() - last_inventory >= 1:
                    other = inventory((child.pid,))
                    try:
                        idle.sample(other, time.monotonic_ns())
                    except RuntimeError:
                        result['other_heavy_process_observations'].append({'elapsed_seconds': time.monotonic() - started, 'processes': other})
                        raise
                    result['evidence_files_written'] = len(list(out.glob('root-*.json')))
                    last_inventory = time.monotonic()
                    dump()
                time.sleep(.05)
        idle.sample(inventory(), time.monotonic_ns())
        require(child.returncode == 0 and not STOP, 'native benchmark nonzero/interrupted')
        require(time.monotonic() - started <= 600, 'native suite guard600s after child exit')
        result.update(exit_code=child.returncode, wall_seconds=time.monotonic() - started,
                      wait4_peak_rss_bytes=usage.ru_maxrss * 1024,
                      user_cpu_seconds=usage.ru_utime, system_cpu_seconds=usage.ru_stime)
        result.update(summarize(read(out / 'report.json'), out, expected_build,
                                max(result['wait4_peak_rss_bytes'], result['observed_process_rss_bytes'], result['observed_process_hwm_bytes'])))
        require(frozen(args) == inputs and predecessor() == prerequisites and sha(Path(__file__)) == driver_sha,
                'source/release/predecessor/driver immutable through measurement')
        result['source_end_verified'] = True
        result['report_sha256'] = sha(out / 'report.json')
        result['final_other_heavy_processes'] = inventory()
        idle.sample(result['final_other_heavy_processes'], time.monotonic_ns())
        result['quiescence_met'] = idle.data['met'] and not result['other_heavy_process_observations']
        passed = all(result[key] for key in ['native_numeric_gate_met', 'whole_process_memory_met',
                                             'all_hard_latency_met', 'p95_met', 'quiescence_met'])
        result['status'] = 'measurement_passed_replay_pending' if passed else 'measurement_failed'
    except BaseException as error:
        result.update(status='failed', error=type(error).__name__ + ': ' + str(error))
    finally:
        if child is not None:
            reaped, cleanup_usage = reap(child, reaped)
            if usage is None:
                usage = cleanup_usage
            result['exit_code'] = child.returncode
        if usage is not None:
            result['wait4_peak_rss_bytes'] = usage.ru_maxrss * 1024
            result['user_cpu_seconds'] = usage.ru_utime
            result['system_cpu_seconds'] = usage.ru_stime
        result['direct_child_reaped'] = reaped
        group_empty = False
        if child is not None and reaped:
            try:
                os.killpg(child.pid, 0)
            except ProcessLookupError:
                group_empty = True
        result['owned_group_empty'] = group_empty
        if not group_empty:
            result['status'] = 'failed'
            result['cleanup_failure'] = 'owned process group emptiness unproved'
        result['wrapper_elapsed_seconds'] = time.monotonic() - started
        result['cpu_after'] = Path('/proc/stat').read_text().splitlines()[0]
        result['meminfo_after'] = Path('/proc/meminfo').read_text()
        finalize_report(result, wrap)
    print(json.dumps({key: result.get(key) for key in ['status', 'wall_seconds', 'p95_elapsed_micros',
                     'whole_process_peak_upper_bound_bytes', 'native_numeric_gate_met', 'quiescence_met',
                     'direct_child_reaped']}), flush=True)
    return 0 if result['status'] == 'measurement_passed_replay_pending' else 2


def main():
    global STOP
    STOP = False
    previous = {sig: signal.getsignal(sig) for sig in [signal.SIGTERM, signal.SIGINT]}
    try:
        return _main()
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == '__main__':
    raise SystemExit(main())
