#!/usr/bin/env python3
"""Select, inspect and bound per-worktree Cargo caches and their native locks.

`check` admits a native command. When the target exceeds its budget it evicts
inside that target only, while holding the target's lock: incremental state
first, then profile directories the gate does not use, then the whole target.
Only the checkout's own target/engine is evicted, and only while its lock is
verifiably held. A target is a reproducible cache: nothing may reference a file
inside it, and generated/ output and external evidence are never touched."""
import argparse
import contextlib
import fcntl
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
GIB = 1024 ** 3
# Inside the worktree: removing the checkout reclaims it and no cache is shared.
DEFAULT_TARGET = Path('target/engine')
DEFAULT_MAX_GIB = '16'
# The single debug/test profile directory every gate command shares.
GATE_PROFILE = 'debug'


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()[:16]


def toplevel(root=ROOT):
    return Path(subprocess.check_output(['git', '-C', str(root), 'rev-parse', '--show-toplevel'], text=True).strip())


def cache_path(root=ROOT, environ=None):
    env = os.environ if environ is None else environ
    if env.get('CARGO_TARGET_DIR'):
        return Path(os.path.abspath(Path(env['CARGO_TARGET_DIR']).expanduser()))
    return (toplevel(root) / DEFAULT_TARGET).absolute()


def lock_directory(environ=None):
    env = os.environ if environ is None else environ
    if env.get('MTG_ENGINE_LOCK_DIR'):
        return Path(os.path.abspath(Path(env['MTG_ENGINE_LOCK_DIR']).expanduser()))
    return Path.home() / '.local/share/mtg-ecosystem/engine-verification/locks'


def lock_path(target, environ=None):
    """One advisory lock per canonical target: aliases of one cache share it."""
    return lock_directory(environ) / f'cargo-{digest(str(Path(target).resolve()))}.lock'


def worktree_lock(worktree, environ=None):
    """Lock of a worktree's default target, independent of CARGO_TARGET_DIR."""
    env = {key: value for key, value in (os.environ if environ is None else environ).items() if key != 'CARGO_TARGET_DIR'}
    return lock_path(cache_path(worktree, env), env)


def lock_state(filename):
    """Probe without creating: idle, busy or absent (never used)."""
    try:
        descriptor = os.open(filename, os.O_RDONLY | os.O_NOFOLLOW)
    except FileNotFoundError:
        return 'absent'
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return 'idle'
    except BlockingIOError:
        return 'busy'
    finally:
        os.close(descriptor)


def gib_setting(env, name, default):
    value = float(env.get(name, default))
    if not math.isfinite(value) or value < 0:
        raise ValueError(f'{name} must be a finite nonnegative GiB value')
    return int(value * GIB)


def storage_module():
    # Reuse the read-only allocated-byte walker; it never follows symlinks.
    spec = importlib.util.spec_from_file_location('engine_storage', ROOT / 'scripts/engine-storage.py')
    storage = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(storage)
    return storage


def status(root=ROOT, environ=None):
    env = os.environ if environ is None else environ
    path = cache_path(root, env)
    storage = storage_module()
    ancestor = path
    while not ancestor.exists():
        ancestor = ancestor.parent
    used = storage.usage(path.resolve()) if path.exists() else 0
    free = shutil.disk_usage(ancestor).free
    maximum = gib_setting(env, 'ENGINE_CACHE_MAX_GIB', DEFAULT_MAX_GIB)
    minimum = gib_setting(env, 'ENGINE_CACHE_MIN_FREE_GIB', '16')
    errors = []
    if path.exists() and not path.is_dir():
        errors.append('CARGO_TARGET_DIR exists but is not a directory')
    if used > maximum:
        errors.append('cache exceeds ENGINE_CACHE_MAX_GIB; run check under the target lock to evict it')
    if free < minimum:
        errors.append('free disk below ENGINE_CACHE_MIN_FREE_GIB; reclaim reviewed disposable output before compiling')
    lock = lock_path(path, env)
    # The launcher exports its selection; only a different path is an override.
    explicit = bool(env.get('CARGO_TARGET_DIR')) and path != cache_path(root, {})
    return {'target_dir': str(path), 'explicit_target': explicit,
            'lock': str(lock), 'lock_state': lock_state(lock),
            'allocated_bytes': used, 'filesystem_free_bytes': free, 'maximum_bytes': maximum,
            'minimum_free_bytes': minimum, 'incremental': env.get('CARGO_INCREMENTAL', 'profile default'),
            'build_jobs': env.get('CARGO_BUILD_JOBS', 'cargo default'),
            'errors': errors, 'policy': 'over budget, check evicts within this target under its lock; nothing outside it is touched'}


def owned_target(root, target):
    """The target, resolved, only if it is this checkout's own Cargo cache.

    Ownership is positive: the path must be exactly `<checkout>/target/engine`
    with no symlinked component below the checkout root and must carry Cargo's
    CACHEDIR.TAG. An explicit CARGO_TARGET_DIR elsewhere (a shared target,
    generated/ output, external evidence) is never evicted."""
    top = toplevel(root)
    if Path(os.path.abspath(Path(target))) != top / DEFAULT_TARGET:
        return None
    entry = top
    for part in DEFAULT_TARGET.parts:
        entry = entry / part
        if entry.is_symlink() or not entry.is_dir():
            return None
    expected = top.resolve() / DEFAULT_TARGET
    if entry.resolve() != expected or not (expected / 'CACHEDIR.TAG').is_file():
        return None
    return expected


def eviction_stages(target):
    """Candidate groups in eviction order; every path lies inside target."""
    profiles = [entry for entry in sorted(target.iterdir()) if entry.is_dir() and not entry.is_symlink()]
    nested = [child for entry in profiles for child in sorted(entry.iterdir())
              if child.is_dir() and not child.is_symlink() and (child / 'deps').is_dir()]
    incremental = [entry / 'incremental' for entry in [*profiles, *nested] if (entry / 'incremental').is_dir()]
    stale = [entry for entry in profiles if entry.name != GATE_PROFILE]
    everything = [entry for entry in sorted(target.iterdir()) if entry.name != 'CACHEDIR.TAG']
    return [('incremental', incremental), ('non-gate-profiles', stale), ('target-reset', everything)]


def remove(entry):
    if entry.is_dir() and not entry.is_symlink():
        shutil.rmtree(entry)
    else:
        entry.unlink(missing_ok=True)


def evict(target, maximum, storage):
    """Shrink target below maximum in stages; returns the removed paths."""
    removed = []
    for stage, entries in eviction_stages(target):
        if storage.usage(target) <= maximum:
            break
        for entry in entries:
            if not (entry == target or target in entry.parents):
                raise ValueError(f'eviction candidate escapes the target: {entry}')
            remove(entry)
            removed.append({'stage': stage, 'path': str(entry)})
    return removed


@contextlib.contextmanager
def held_lock(lock, environ):
    """Yield whether this process holds the target's canonical lock.

    A runner that already holds it passes the locked descriptor's number in
    MTG_ENGINE_LOCK_FD. That descriptor must be the canonical lock file, and a
    non-blocking flock on it must succeed: flock is per open file description,
    so this confirms the inherited description holds (or now takes) the lock.
    Without it, this call takes the lock itself without waiting. Busy or
    unverifiable locks fail closed."""
    if lock.is_symlink():
        yield False
        return
    inherited = environ.get('MTG_ENGINE_LOCK_FD')
    if inherited is not None:
        try:
            descriptor = int(inherited)
            held, canonical = os.fstat(descriptor), os.stat(lock)
            if (held.st_dev, held.st_ino) != (canonical.st_dev, canonical.st_ino):
                yield False
                return
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except (ValueError, OSError):
            yield False
            return
        yield True
        return
    lock.parent.mkdir(parents=True, exist_ok=True)
    with lock.open('a') as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            yield False
            return
        yield True


def check(root=ROOT, environ=None):
    """Admit a native command, evicting an over-budget target under its lock."""
    env = os.environ if environ is None else environ
    report = status(root, env)
    if report['allocated_bytes'] <= report['maximum_bytes']:
        return report
    target = owned_target(root, report['target_dir'])
    if target is None:
        report['errors'].append("target is not this checkout's own Cargo cache (target/engine with CACHEDIR.TAG, no symlinks); nothing evicted")
        return report
    with held_lock(Path(report['lock']), env) as owned:
        if not owned:
            report['errors'].append('target lock is busy or the inherited MTG_ENGINE_LOCK_FD is not its held lock; nothing evicted')
            return report
        removed = evict(target, report['maximum_bytes'], storage_module())
    after = status(root, env)
    if any(row['stage'] == 'target-reset' for row in removed):
        # An emptied target holds only its tag: the budget cannot go lower.
        after['errors'] = [e for e in after['errors'] if 'ENGINE_CACHE_MAX_GIB' not in e]
    after['evicted'] = removed
    after['allocated_before_bytes'] = report['allocated_bytes']
    return after


def listing(root=ROOT, environ=None):
    """Read-only per-worktree visibility: default target size and lock state."""
    env = {key: value for key, value in (os.environ if environ is None else environ).items() if key != 'CARGO_TARGET_DIR'}
    storage = storage_module()
    rows = []
    for worktree in storage.worktrees(root):
        target = worktree / DEFAULT_TARGET
        lock = lock_path(target, env)
        state = lock_state(lock)
        if target.is_dir() and not target.is_symlink() or state != 'absent':
            rows.append({'worktree': str(worktree), 'target_dir': str(target), 'lock_state': state,
                         'allocated_bytes': storage.usage(target) if target.is_dir() and not target.is_symlink() else 0})
    cache_home = Path(env.get('XDG_CACHE_HOME', str(Path.home() / '.cache'))).expanduser()
    legacy = [{'target_dir': str(entry), 'allocated_bytes': storage.usage(entry)}
              for entry in sorted((cache_home / 'mtg-ecosystem/cargo').glob('*/*')) if entry.is_dir() and not entry.is_symlink()]
    legacy_lock = Path.home() / '.local/share/mtg-ecosystem/engine-verification/cargo.lock'
    return {'worktrees': sorted(rows, key=lambda row: -row['allocated_bytes']),
            'total_bytes': sum(row['allocated_bytes'] for row in rows),
            'legacy_shared_caches': legacy, 'legacy_shared_lock_state': lock_state(legacy_lock),
            'note': 'Legacy shared caches and the legacy global lock are no longer selected by default; review ownership before reclaiming.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['path', 'lock', 'status', 'check', 'list'])
    args = parser.parse_args()
    try:
        if args.command == 'path':
            print(cache_path())
            return 0
        if args.command == 'lock':
            print(lock_path(cache_path()))
            return 0
        if args.command == 'list':
            print(json.dumps(listing(), indent=2))
            return 0
        report = check() if args.command == 'check' else status()
        print(json.dumps(report, indent=2))
        return bool(report['errors']) if args.command == 'check' else 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Engine cache: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
