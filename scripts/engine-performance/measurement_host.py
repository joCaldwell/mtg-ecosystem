#!/usr/bin/env python3
"""Private diagnostic OS adapter; never builds or changes game/native sources.

Linux user-systemd/cgroup-v2 authority. A small gated payload keeps the cgroup
alive before/after the actual command, allowing kernel limit/counter validation.
Only the newly generated exact unit is ever stopped. Trusted private filesystem
parents and no concurrent same-UID adversary are the finite host threat model.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import platform
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import decimal
import sys
import time
import uuid

MEMORY_BYTES = 2 * 1024**3
TASKS = 128
MAX_JSON_BYTES = 128 * 1024**2
CGROUP_ROOT = Path('/sys/fs/cgroup')


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def no_duplicates(pairs):
    result = {}
    for k, v in pairs:
        if k in result:
            raise ValueError('duplicate JSON key')
        result[k] = v
    return result


def read_json(path: Path):
    if path.stat().st_size > MAX_JSON_BYTES:
        raise ValueError('JSON exceeds bound')
    return json.loads(path.read_text(), object_pairs_hook=no_duplicates,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def save_json(path: Path, value):
    temp = path.with_name(path.name + '.tmp')
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, 'w') as f:
            json.dump(value, f, sort_keys=True, indent=2, allow_nan=False)
            f.write('\n')
            f.flush()
            os.fsync(f.fileno())
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


def real_path(path: Path, *, private=False):
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError('absolute canonical path required')
    for part in (path, *path.parents):
        mode = part.lstat().st_mode
        if stat.S_ISLNK(mode):
            raise ValueError('symlink ancestry')
    if private and stat.S_IMODE(path.stat().st_mode) != 0o700:
        raise ValueError('private parent must be 0700')
    return path


def physical_cpus():
    """Lowest allowed sibling per (package, physical core), same native rule."""
    cores = {}
    for cpu in sorted(os.sched_getaffinity(0)):
        base = Path(f'/sys/devices/system/cpu/cpu{cpu}/topology')
        key = (int((base/'physical_package_id').read_text()), int((base/'core_id').read_text()))
        cores.setdefault(key, cpu)
    return sorted(cores.values())


def validate_cpus(cpus):
    if not isinstance(cpus, list) or not cpus or len(cpus) > 64:
        raise ValueError('CPU list must contain 1..64 physical cores')
    if any(type(c) is not int for c in cpus) or len(set(cpus)) != len(cpus):
        raise ValueError('CPU IDs must be distinct integers')
    if not set(cpus) <= set(physical_cpus()):
        raise ValueError('CPU list must use available physical representatives')


def _ctl(args, timeout=3):
    # This control subprocess has no descendant workload; it is separately reaped.
    p = subprocess.Popen(['systemctl', '--user', *args], stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, start_new_session=True)
    try:
        out, err = p.communicate(timeout=timeout)
    except BaseException:
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        p.wait(timeout=3)
        raise
    if len(out) > 65536 or len(err) > 65536:
        raise RuntimeError('systemctl output bound')
    return p.returncode, out.decode(errors='strict')


def _show(unit):
    code, output = _ctl(['show', '--no-pager', '--property=ControlGroup,MainPID,MemoryMax,MemorySwapMax,TasksMax,RuntimeMaxUSec,ActiveState,SubState', unit])
    if code:
        raise RuntimeError('unit show failed')
    return dict(line.split('=', 1) for line in output.splitlines() if '=' in line)


def _group(properties):
    raw = properties.get('ControlGroup', '')
    if not raw.startswith('/') or '..' in Path(raw).parts or raw == '/':
        raise RuntimeError('invalid control group')
    return CGROUP_ROOT / raw.lstrip('/')


def _counters(group):
    scalar = lambda n: (group/n).read_text().strip()
    pairs = lambda n: {k: int(v) for k, v in (line.split() for line in scalar(n).splitlines())}
    return {'memory_max': scalar('memory.max'), 'memory_swap_max': scalar('memory.swap.max'),
            'pids_max': scalar('pids.max'), 'memory_peak_bytes': int(scalar('memory.peak')),
            'memory_current_bytes': int(scalar('memory.current')), 'memory_events': pairs('memory.events'),
            'cpu_stat': pairs('cpu.stat'), 'cgroup_events': pairs('cgroup.events')}


def duration_us(text):
    parts = re.findall(r"([0-9]+(?:\.[0-9]+)?)(us|ms|s|min|h|d)", text)
    if not parts or re.sub(r"([0-9]+(?:\.[0-9]+)?)(us|ms|s|min|h|d)|\s+", "", text):
        raise RuntimeError("invalid systemd duration")
    scale = {"us":1,"ms":1000,"s":1000000,"min":60000000,"h":3600000000,"d":86400000000}
    value = sum(decimal.Decimal(n)*scale[u] for n,u in parts)
    if value != int(value):
        raise RuntimeError("fractional microsecond duration")
    return int(value)


def validate_limits(properties, counters, pid, cpus):
    if properties.get('MainPID') != str(pid):
        raise RuntimeError('service PID differs from gated payload')
    expected = {'MemoryMax': str(MEMORY_BYTES), 'MemorySwapMax': '0', 'TasksMax': str(TASKS)}
    if any(properties.get(k) != v for k, v in expected.items()):
        raise RuntimeError('systemd limit mismatch')
    if (counters['memory_max'], counters['memory_swap_max'], counters['pids_max']) != (str(MEMORY_BYTES), '0', str(TASKS)):
        raise RuntimeError('kernel limit mismatch')
    if sorted(os.sched_getaffinity(pid)) != sorted(cpus):
        raise RuntimeError('payload CPU affinity mismatch')


def assert_only_payload(group, pid):
    if {int(p) for p in (group/'cgroup.procs').read_text().split()} != {pid}:
        raise RuntimeError('workload left unexpected live descendants')


def _cleanup(unit, launcher, group):
    errors = []
    empty = False
    def is_empty():
        if group is None:
            return False
        try:
            return not group.exists() or _counters(group)['cgroup_events'].get('populated') == 0
        except FileNotFoundError:
            return True
    try:
        # Unit name is newly generated internally, never supplied by callers.
        for args in (['kill', '--kill-whom=all', '--signal=KILL', unit], ['stop', unit]):
            try:
                _ctl(args)
            except Exception as e:
                errors.append(type(e).__name__)
        if group is not None:
            try:
                empty = is_empty()
                if not empty:
                    # Exact group already obtained from our random owned unit.
                    (group/'cgroup.kill').write_text('1')
                until = time.monotonic()+5
                while not empty and time.monotonic() < until:
                    empty = is_empty()
                    if not empty:
                        time.sleep(.025)
            except Exception as e:
                errors.append(type(e).__name__)
        else:
            try:
                prop = _show(unit)
                empty = prop.get('MainPID', '0') == '0' and not prop.get('ControlGroup')
            except Exception:
                code, _ = _ctl(['is-active', unit])
                empty = code in (3, 4)
    finally:
        # Even failed kernel/control reads cannot skip reaping the owned launcher.
        try:
            launcher.wait(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(launcher.pid, signal.SIGKILL)
            launcher.wait(timeout=3)
        try:
            _ctl(['reset-failed', unit])
        except Exception:
            pass
    return {'unit_empty': bool(empty), 'launcher_reaped': launcher.poll() is not None, 'errors': errors}


def _bounded_audit(audit, deadline):
    remaining = deadline-time.monotonic()
    if remaining <= 0:
        raise TimeoutError("deadline before audit")
    if signal.getitimer(signal.ITIMER_REAL) != (0.0, 0.0):
        raise RuntimeError("preexisting timer is incompatible with bounded audit")
    def timeout(_signum, _frame):
        raise TimeoutError("external deadline during final audit")
    previous = signal.signal(signal.SIGALRM, timeout)
    try:
        signal.setitimer(signal.ITIMER_REAL, remaining)
        return audit()
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)


def run_bounded(*, command: list[str], cwd: Path, env: dict[str, str], directory: Path,
                label: str, cpus: list[int], timeout_seconds: float = 120.0,
                expected_executable_sha256: str, audit=None) -> dict:
    """Return and retain report, including failures. No resource-limit bypass.

    `audit()` runs AFTER workload exit, within the SAME monotonic deadline; it
    must validate complete outputs/identities. Caller must check status=='passed'.
    No fallback unsandboxed run. Linux/systemd failures remain failed attempts.
    """
    validate_cpus(cpus)
    if not re.fullmatch(r'[a-z0-9-]{1,48}', label):
        raise ValueError('invalid label')
    if not math.isfinite(timeout_seconds) or not 0 < timeout_seconds <= 120:
        raise ValueError('deadline must be 0..120 seconds')
    if not command or any(not isinstance(a, str) or '\0' in a for a in command):
        raise ValueError('invalid argv')
    exe = real_path(Path(command[0]))
    real_path(cwd)
    real_path(directory.parent, private=True)
    if directory.exists() or directory.is_symlink():
        raise ValueError('attempt must be previously absent')
    if sha(exe) != expected_executable_sha256:
        raise ValueError('executable identity mismatch')
    directory.mkdir(mode=0o700)
    os.chmod(directory, 0o700)
    unit = 'mtg-measure-' + label + '-' + uuid.uuid4().hex + '.service'
    started = time.monotonic()
    deadline = started + timeout_seconds
    report = {'schema': 'bounded-measurement-v1', 'status': 'running', 'unit': unit,
              'command': command, 'cwd': str(cwd), 'cpu_ids': cpus, 'deadline_seconds': timeout_seconds,
              'executable_sha256': expected_executable_sha256, 'helper_sha256': sha(Path(__file__).absolute()),
              'limits': {'memory_bytes': MEMORY_BYTES, 'postexit_child_rss_guard_bytes': MEMORY_BYTES, 'swap_bytes': 0, 'tasks': TASKS, 'systemd_runtime_max_seconds': math.ceil(timeout_seconds)+5},
              'memory_accounting': 'kernel memory.peak: peak charged memory for the cgroup including payload, page cache and kernel charges; NOT an upper bound on process RSS (shared-page ownership differs). wait4 ru_maxrss is independently guarded and separately labeled, never tree-summed RSS.',
              'deadline_scope': 'startup, workload, final audit, cleanup; cleanup continues bounded after a timeout failure',
              'python': {'executable': str(Path(sys.executable).resolve()), 'sha256': sha(Path(sys.executable).resolve()), 'version': sys.version},
              'kernel': platform.uname()._asdict(),
              'scope': 'entire workload process tree plus gated Python payload; no build; diagnostic only'}
    save_json(directory/'host-report.json', report)
    spec = {'command': command, 'cwd': str(cwd), 'env': env}
    save_json(directory/'payload-spec.json', spec)
    launcher = None
    group = None
    previous = {}
    interrupted = []
    def stop(signum, _frame):
        # Defer raising until after Popen returns/lease ownership is established.
        interrupted.append(signum)
    try:
        for sig in (signal.SIGTERM, signal.SIGINT):
            previous[sig] = signal.signal(sig, stop)
        args = ['systemd-run', '--user', '--pipe', '--quiet', '--no-ask-password', '--expand-environment=no',
                '--unit='+unit, '--property=Type=exec', '--property=MemoryMax='+str(MEMORY_BYTES),
                '--property=MemorySwapMax=0', '--property=TasksMax='+str(TASKS),
                '--property=KillMode=control-group', '--property=SendSIGKILL=yes',
                '--property=TimeoutStopSec=2s', '--property=UMask=0077',
                '--property=RuntimeMaxSec='+str(math.ceil(timeout_seconds)+5),
                '--property=CPUAffinity='+' '.join(map(str, cpus)), '--', sys.executable,
                str(Path(__file__).absolute()), '--payload', str(directory)]
        with open(directory/'launcher.stdout','xb', opener=lambda p,f: os.open(p,f,0o600)) as out, \
             open(directory/'launcher.stderr','xb', opener=lambda p,f: os.open(p,f,0o600)) as err:
            launcher = subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=out, stderr=err, start_new_session=True)
            ready_path = directory/'payload-ready.json'
            while not ready_path.exists():
                if interrupted:
                    raise InterruptedError('host signal')
                if time.monotonic() >= deadline:
                    raise TimeoutError('external row deadline')
                if launcher.poll() is not None:
                    raise RuntimeError('service startup failure')
                time.sleep(.02)
            ready = read_json(ready_path)
            properties = _show(unit)
            if duration_us(properties.get('RuntimeMaxUSec','')) != (math.ceil(timeout_seconds)+5)*1000000:
                raise RuntimeError('systemd orphan lifetime backstop mismatch')
            report['systemd_properties'] = properties
            group = _group(properties)
            initial = _counters(group)
            validate_limits(properties, initial, ready['pid'], cpus)
            report.update(kernel_initial=initial, control_group=str(group), payload=ready)
            if interrupted:
                raise InterruptedError('host signal')
            (directory/'release').touch(mode=0o600, exist_ok=False)
            while not (directory/'payload-result.json').exists():
                if interrupted:
                    raise InterruptedError('host signal')
                if time.monotonic() >= deadline:
                    raise TimeoutError('external row deadline')
                if launcher.poll() is not None:
                    raise RuntimeError('service disappeared')
                time.sleep(.02)
            result = read_json(directory/'payload-result.json')
            report['workload'] = result
            final = _counters(group)
            report['kernel_final'] = final
            validate_limits(_show(unit), final, ready['pid'], cpus)
            assert_only_payload(group, ready['pid'])
            rss=result.get('ru_maxrss_bytes')
            if type(rss) is not int or not 0 < rss <= MEMORY_BYTES:
                raise RuntimeError('postexit direct-child RSS high-water guard failed')
            if result['exit_code'] != 0:
                raise RuntimeError('workload nonzero exit')
            if final['memory_peak_bytes'] > MEMORY_BYTES or any(final['memory_events'].get(k, 0) for k in ('oom','oom_kill','oom_group_kill','max')):
                raise RuntimeError('memory bound hit')
            if sha(exe) != expected_executable_sha256:
                raise RuntimeError('executable changed during row')
            if audit is not None:
                report['audit'] = _bounded_audit(audit, deadline)
            if sha(exe) != expected_executable_sha256 or sha(Path(__file__).absolute()) != report['helper_sha256'] or sha(Path(sys.executable).resolve()) != report['python']['sha256']:
                raise RuntimeError('execution authority changed during audit')
            if interrupted:
                raise InterruptedError('host signal')
            if time.monotonic() >= deadline:
                raise TimeoutError('row deadline includes final audits')
            report['status'] = 'passed'
    except BaseException as e:
        report.update(status='failed', failure={'type': type(e).__name__, 'reason': str(e)[:2048]})
    finally:
        if group is not None and 'kernel_final' not in report:
            try:
                report['kernel_final'] = _counters(group)
            except Exception as e:
                report['kernel_read_failure'] = type(e).__name__
        if launcher is not None:
            try:
                report['cleanup'] = _cleanup(unit, launcher, group)
            except Exception as e:
                report['cleanup'] = {'unit_empty': False, 'launcher_reaped': launcher.poll() is not None, 'failure': type(e).__name__}
            if not report['cleanup'].get('unit_empty') or not report['cleanup'].get('launcher_reaped'):
                report['status'] = 'failed'
        report['total_elapsed_seconds_including_cleanup'] = time.monotonic() - started
        if time.monotonic() >= deadline and report['status'] == 'passed':
            report.update(status='failed',failure={'type':'TimeoutError','reason':'deadline includes cleanup'})
        report['measurement_elapsed_seconds'] = report.get('workload', {}).get('wall_seconds')
        if interrupted:
            report.update(status='failed',failure={'type':'InterruptedError','reason':'host signal including cleanup boundary'})
        report['received_signals'] = interrupted
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        save_json(directory/'host-report.json', report)
    return report


def payload(directory):
    """Private service-only wrapper. No benchmark semantics or identity claims."""
    os.umask(0o077)
    spec = read_json(directory/'payload-spec.json')
    save_json(directory/'payload-ready.json', {'pid': os.getpid(), 'affinity': sorted(os.sched_getaffinity(0))})
    while not (directory/'release').exists():
        time.sleep(.01)
    started = time.monotonic()
    with open(directory/'workload.stdout','xb', opener=lambda p,f: os.open(p,f,0o600)) as out, \
         open(directory/'workload.stderr','xb', opener=lambda p,f: os.open(p,f,0o600)) as err:
        child = subprocess.Popen(spec['command'], cwd=spec['cwd'], env=spec['env'],
                                 stdin=subprocess.DEVNULL, stdout=out, stderr=err)
        _, status, usage = os.wait4(child.pid, 0)
        code = os.waitstatus_to_exitcode(status)
        child.returncode = code
    save_json(directory/'payload-result.json', {'exit_code': code,
        'wall_seconds': time.monotonic()-started, 'user_seconds': usage.ru_utime,
        'system_seconds': usage.ru_stime, 'ru_maxrss_bytes': usage.ru_maxrss*1024,
        'accounting': 'wait4 direct-child usage including threads and waited-descendant accounting; ru_maxrss is a process high-water/max, NOT tree-summed RSS. Actual native target is one mtg-sim process with threads; cgroup counters separately account all charged memory.'})
    while True:
        time.sleep(.1)


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--payload', type=Path, required=True)
    payload(p.parse_args().payload)
