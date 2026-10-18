"""Pure portable input guards. No native workload, build or approval on import."""
from pathlib import Path
import hashlib
import importlib.util
import json
import os
import re
import stat
import signal
import subprocess
import time

WORKLOAD_SHA = '75f1665a6c41dfc285cb4cb46b74923dd1760d753783ec0a71b1d0b2b4834d45'
WORKLOAD_REL = 'packages/game-engine/tests/fixtures/search-workload.json'


def need(value, why):
    if not value:
        raise ValueError(why)


def plain(path, directory=False):
    p = Path(path)
    need(p.is_absolute() and '..' not in p.parts, 'absolute plain path')
    for q in (p, *p.parents):
        need(not stat.S_ISLNK(q.lstat().st_mode), 'symlink ancestry')
    need(p.is_dir() if directory else p.is_file(), 'plain directory/file')
    return p


def relative(value):
    need(type(value) is str and value and not any(c in value for c in ['\\', ':', '\0']), 'plain relative path')
    p = Path(value)
    need(not p.is_absolute() and p.as_posix() == value and
         all(x not in ('', '.', '..') for x in value.split('/')), 'safe relative path')
    return p


def sha(path):
    with plain(path).open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def read(path):
    def pairs(values):
        out = {}
        for key, value in values:
            need(key not in out, 'duplicate JSON key')
            out[key] = value
        return out
    def bad(value):
        raise ValueError('nonfinite JSON ' + value)
    return json.loads(plain(path).read_bytes(), object_pairs_hook=pairs, parse_constant=bad)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False,
                      allow_nan=False).encode()


def pin(row):
    need(type(row) is dict and set(row) == {'path', 'sha256'} and
         type(row['sha256']) is str and re.fullmatch('[0-9a-f]{64}', row['sha256']), 'exact file pin')
    p = plain(row['path'])
    need(sha(p) == row['sha256'], 'pinned file changed: ' + str(p))
    return p


def module(row, name):
    p = pin(row)  # Authenticate BEFORE executing imported code.
    spec = importlib.util.spec_from_file_location(name, p)
    out = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(out)
    return out


def save(path, value):
    temporary = path.with_name(path.name + '.tmp')
    # Directory is owned/exclusive for this run; no untrusted shared path.
    with temporary.open('w') as out:
        json.dump(value, out, indent=2, allow_nan=False)
        out.write('\n')
        out.flush()
        os.fsync(out.fileno())
    temporary.chmod(0o600)
    temporary.replace(path)


def private_parent(path):
    p = plain(path, directory=True)
    need(stat.S_IMODE(p.stat().st_mode) == 0o700, 'private0700 parent')
    return p


def fresh(path, parent):
    parent = private_parent(parent)
    p = Path(path)
    need(p.is_absolute() and p.parent == parent and relative(p.name).name == p.name and
         not p.exists() and not p.is_symlink(), 'fresh direct-child output, no retries/overwrite')
    return p


def controller_environment(environ):
    """Controller user-bus locators only; native payload environment is separate."""
    result = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}
    for key in ('DBUS_SESSION_BUS_ADDRESS', 'XDG_RUNTIME_DIR'):
        if key in environ:
            result[key] = environ[key]
    return result


def run_owned(command, cwd, stdout, stderr, timeout, cancelled=lambda: False):
    """One attempt only. Deferred cancellation acquires Popen lease first.

    The pod controller has150s (its unchanged child envelope is120s); each
    independent replay has300s. These are not primary-root latency limits.
    """
    need(timeout in (150, 300), 'declared pod-controller/replay timeout')
    child = None
    reaped = False
    usage = None
    began = time.monotonic()
    failure = None
    def group_alive():
        try:
            os.killpg(child.pid, 0)
            return True
        except ProcessLookupError:
            return False
    try:
        with stdout.open('xb') as out, stderr.open('xb') as err:
            need(not cancelled(), 'cancelled before spawn')
            child = subprocess.Popen(command, cwd=cwd, stdin=subprocess.DEVNULL,
                stdout=out, stderr=err, start_new_session=True,
                env=controller_environment(os.environ))
            # A signal during Popen only records a flag; now we own the lease.
            while True:
                pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                if pid:
                    reaped = True
                    child.returncode = os.waitstatus_to_exitcode(status)
                    break
                need(not cancelled(), 'interrupted owned attempt')
                need(time.monotonic() - began <= timeout, 'owned attempt deadline')
                time.sleep(.02)
        need(not cancelled() and time.monotonic() - began <= timeout, 'exit after cancellation/deadline')
        need(child.returncode == 0, 'owned attempt nonzero')
    except BaseException as error:
        failure = type(error).__name__ + ': ' + str(error)
    finally:
        if child is not None:
            for sig in (signal.SIGTERM, signal.SIGKILL):
                try:
                    os.killpg(child.pid, sig)
                except ProcessLookupError:
                    pass
                until = time.monotonic() + 5
                while time.monotonic() < until:
                    if not reaped:
                        pid, status, got = os.wait4(child.pid, os.WNOHANG)
                        if pid:
                            reaped = True
                            usage = got
                            child.returncode = os.waitstatus_to_exitcode(status)
                    if reaped and not group_alive():
                        break
                    time.sleep(.02)
            if not reaped or group_alive():
                failure = (failure or '') + '; owned group cleanup/reap unproved'
        else:
            failure = failure or 'no child lease acquired'
    if cancelled():
        failure = (failure + '; ' if failure else '') + 'interrupted through cleanup boundary'
    return {'status': 'passed' if failure is None else 'failed', 'failure': failure,
            'exit_code': child.returncode if child else None, 'direct_child_reaped': reaped,
            'owned_group_empty': bool(child and reaped and not group_alive()),
            'outer_elapsed_seconds': time.monotonic() - began,
            'wait4_user_seconds': usage.ru_utime if usage else None,
            'wait4_system_seconds': usage.ru_stime if usage else None,
            'wait4_peak_rss_bytes': usage.ru_maxrss * 1024 if usage else None}


def production_profile(settings, artifact, executable_sha):
    keys = {'rustc', 'profile', 'target', 'rustflags', 'opt_level', 'debug',
            'debug_assertions', 'overflow_checks'}
    need(set(settings) == keys and all(type(settings[k]) is str for k in keys - {'overflow_checks'}), 'eight build settings')
    need(settings['rustc'].startswith('rustc ') and not any(c in settings['rustc'] for c in '\n\r\0') and
         re.fullmatch('[A-Za-z0-9_.-]+', settings['target']) and '\0' not in settings['rustflags'], 'canonical compiler settings')
    need(settings['profile'] == 'release' and settings['opt_level'] == '3' and
         settings['debug'] == settings['debug_assertions'] == 'false' and
         settings['overflow_checks'] is True, 'release opt3 overflow settings')
    need(artifact.get('reason') == 'compiler-artifact' and
         artifact.get('target', {}).get('name') == 'mtg-sim' and
         artifact['target'].get('kind') == ['bin'] and artifact.get('features') == [], 'production SIM baseline artifact')
    profile = artifact.get('profile', {})
    need(profile.get('test') is False and profile.get('opt_level') == '3' and
         type(profile.get('debuginfo')) is int and profile['debuginfo'] == 0 and
         profile.get('debug_assertions') is False and profile.get('overflow_checks') is True,
         'actual production profile.test=false, not a test executable')
    need(sha(Path(artifact['executable'])) == executable_sha, 'actual Cargo executable bytes')


def check_search(authority_path, authority_sha, caller):
    authority_path = pin({'path': str(authority_path), 'sha256': authority_sha})
    a = read(authority_path)
    need(a['schema'] == 'final-search-host-authority-v1', 'portable authority schema')
    root = plain(a['stage_root'], directory=True)
    need(re.fullmatch('[0-9a-f]{64}', a['build_id']) is not None, 'BuildID codec')
    for path, expected in a['host_sources'].items():
        pin({'path': path, 'sha256': expected})
    for p in [Path(__file__).absolute(), Path(caller).absolute()]:
        need(a['host_sources'].get(str(p)) == sha(p), 'actual caller/input guard pinned')
    fixed = module(a['fixed_driver'], 'portable_authenticated_fixed')
    need(a['host_sources'].get(a['fixed_driver']['path']) == a['fixed_driver']['sha256'], 'fixed helper map binding')
    helper = fixed.load_host(a['measurement_helper_sha256'])
    need(a['host_sources'].get(str(Path(a['fixed_driver']['path']).with_name('measurement_host.py'))) == a['measurement_helper_sha256'], 'unchanged OS helper pin')
    native_path = pin(a['native_manifest'])
    native = read(native_path)
    files = native['files']
    need(native['status'] == 'compiled-source-closure-verified' and native['build_id'] == a['build_id'], 'actual compiled native manifest')
    need(type(a['native_files']) is int and len(files) == a['native_files'] and
         type(a['native_bytes']) is int and sum(v['bytes'] for v in files) == a['native_bytes'], 'explicit full manifest count/bytes')
    names = [v['path'] for v in files]
    need(names == sorted(set(names)) == fixed.inventory(root), 'complete ordered native namespace')
    archive_path = pin(a['archive_manifest'])
    archive = read(archive_path)
    need(archive['engine_build_id'] == a['build_id'], 'immutable archive build binding')
    archive_root = archive_path.parent
    # Some retained manifests address rows beneath files/, while historical
    # release archives address their own root. The SHA-pinned authority declares
    # the container; row names remain exact, never guessed or prefix-stripped.
    files_prefix = a['archive_files_prefix']
    need(type(files_prefix) is str, 'explicit archive file container')
    file_root = archive_root / relative(files_prefix) if files_prefix else archive_root
    plain(file_root, directory=True)
    prefix = relative(a['archive_native_prefix'])
    def archived(path):
        rel = str(path.relative_to(file_root))
        expected = archive['files'][rel]
        need(type(expected['bytes']) is int and path.stat().st_size == expected['bytes'] and
             sha(path) == expected['sha256'], 'actual immutable archive file ' + rel)
    archived(native_path)
    for item in files:
        rel = relative(item['path'])
        live, stored = root / rel, file_root / prefix / rel
        need(type(item['bytes']) is int and item['bytes'] >= 0 and
             live.stat().st_size == stored.stat().st_size == item['bytes'] and
             sha(live) == sha(stored) == item['sha256'], 'native source/archive bytes ' + str(rel))
        archived(stored)
    executable = pin(a['cli'])
    archived(executable)
    artifact_path = pin(a['compiler_artifact'])
    archived(artifact_path)
    production_profile(a['build_settings'], read(artifact_path), a['cli']['sha256'])
    need(fixed.compiled_build_id(root, files, a['build_settings']) == a['build_id'], 'independently recomputed BuildID')
    workload_path = pin(a['workload'])
    need(workload_path == root / WORKLOAD_REL and a['workload']['sha256'] == WORKLOAD_SHA and
         sha(workload_path) == sha(file_root / prefix / WORKLOAD_REL) == WORKLOAD_SHA, 'exact original frozen workload bytes')
    private_parent(a['run_parent'])
    need(type(a['prerequisites']) is list and len(a['prerequisites']) >= 2, 'explicit completed predecessor gate/audit evidence')
    predecessor = {}
    for evidence in a['prerequisites']:
        path = pin(evidence['file'])
        proof = read(path)
        need(evidence['expected_status'] == proof['status'] == 'passed', 'completed prerequisite status')
        if 'expected_build_id' in evidence:
            need(proof['build_id'] == evidence['expected_build_id'], 'prerequisite explicit BuildID')
        need(evidence['role'] not in predecessor, 'unique prerequisite role')
        predecessor[evidence['role']] = {'path': str(path), 'sha256': sha(path)}
    need('functional_gates' in predecessor and 'independent_audit' in predecessor, 'both gate and independent audit prerequisite roles')
    proof = {'authority_path': str(authority_path), 'authority_sha256': authority_sha,
             'native_manifest_sha256': sha(native_path), 'archive_manifest_sha256': sha(archive_path),
             'workload_sha256': WORKLOAD_SHA, 'executable': str(executable),
             'host_source_sha256': hashlib.sha256(canonical(a['host_sources'])).hexdigest(),
             'predecessors': predecessor}
    return a, native, proof
