#!/usr/bin/env python3
"""Select and inspect per-worktree Cargo caches and their native locks.

Never moves or deletes cache/evidence bytes."""
import argparse
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
    maximum = gib_setting(env, 'ENGINE_CACHE_MAX_GIB', '24')
    minimum = gib_setting(env, 'ENGINE_CACHE_MIN_FREE_GIB', '16')
    errors = []
    if path.exists() and not path.is_dir():
        errors.append('CARGO_TARGET_DIR exists but is not a directory')
    if used > maximum:
        errors.append('cache exceeds ENGINE_CACHE_MAX_GIB; inspect it and run engine-storage.py prune-cargo, or explicitly raise the budget')
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
            'errors': errors, 'policy': 'preflight only; no automatic eviction; retained evidence is never a cache'}


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
        report = status()
        print(json.dumps(report, indent=2))
        return bool(report['errors']) if args.command == 'check' else 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Engine cache: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
