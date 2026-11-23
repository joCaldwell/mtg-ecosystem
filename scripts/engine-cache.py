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
import stat
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


CARGO_TAG = b'Signature: 8a477f597d28d172789f06886806bc55'
DIRECTORY = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


class Refused(Exception):
    """Eviction cannot proceed safely; nothing has been deleted."""


def open_owned_target(root, target, lock):
    """Pin this checkout's own target/engine as a directory descriptor.

    Ownership is positive: the configured path must be exactly
    `<checkout>/target/engine`; `target` and `engine` are opened with
    O_NOFOLLOW relative to the checkout, CACHEDIR.TAG must be a regular file
    (opened without following links) carrying Cargo's signature, and the lock
    must lie strictly outside the target. Every later read or delete is
    relative to the returned descriptor, so a swapped path cannot redirect it."""
    top = Path(os.path.abspath(toplevel(root)))
    expected = top / DEFAULT_TARGET
    if Path(os.path.abspath(Path(target))) != expected:
        raise Refused("target is not this checkout's own target/engine")
    # Every component from / is opened without following a link.
    descriptor = os.open('/', DIRECTORY)
    try:
        for part in expected.parts[1:]:
            try:
                child = os.open(part, DIRECTORY, dir_fd=descriptor)
            except OSError as error:
                raise Refused(f'{part}/ on the target path is missing, not a directory or a symlink') from error
            os.close(descriptor)
            descriptor = child
        try:
            # O_NONBLOCK: a FIFO tag can never block while the lock is held.
            tag = os.open('CACHEDIR.TAG', os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                          dir_fd=descriptor)
        except OSError as error:
            raise Refused('CACHEDIR.TAG is missing or a symlink') from error
        try:
            if not stat.S_ISREG(os.fstat(tag).st_mode) or not os.read(tag, 64).startswith(CARGO_TAG):
                raise Refused('CACHEDIR.TAG is not a regular Cargo cache tag')
        finally:
            os.close(tag)
        pinned = os.path.realpath(f'/proc/self/fd/{descriptor}')
        for path in (os.path.realpath(lock), os.path.realpath(Path(lock).parent)):
            if path == pinned or path.startswith(pinned + os.sep):
                raise Refused('the target lock lies inside the target; refusing to delete it')
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def entries(descriptor):
    """(name, is_real_directory) of a directory, never following links."""
    with os.scandir(descriptor) as listing:
        return sorted((entry.name, entry.is_dir(follow_symlinks=False)) for entry in listing)


def usage_at(descriptor, seen=None):
    """Allocated bytes below a pinned directory, without following links and
    counting each hard-linked inode once (as engine-storage.py usage does)."""
    seen = set() if seen is None else seen
    info = os.fstat(descriptor)
    if (info.st_dev, info.st_ino) in seen:
        return 0
    seen.add((info.st_dev, info.st_ino))
    total = info.st_blocks * 512
    for name, is_directory in entries(descriptor):
        if is_directory:
            child = os.open(name, DIRECTORY, dir_fd=descriptor)
            try:
                total += usage_at(child, seen)
            finally:
                os.close(child)
        else:
            info = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
            if (info.st_dev, info.st_ino) not in seen:
                seen.add((info.st_dev, info.st_ino))
                total += info.st_blocks * 512
    return total


def remove_at(parent, name):
    """Delete parent/name; links are unlinked, never followed."""
    if stat.S_ISDIR(os.stat(name, dir_fd=parent, follow_symlinks=False).st_mode):
        child = os.open(name, DIRECTORY, dir_fd=parent)
        try:
            for entry, _ in entries(child):
                remove_at(child, entry)
        finally:
            os.close(child)
        os.rmdir(name, dir_fd=parent)
    else:
        os.unlink(name, dir_fd=parent)


def has_directory(descriptor, *parts):
    """Whether descriptor/parts... is a real directory, one O_NOFOLLOW step at a time."""
    current = os.dup(descriptor)
    try:
        for part in parts:
            try:
                child = os.open(part, DIRECTORY, dir_fd=current)
            except OSError:
                return False
            os.close(current)
            current = child
        return True
    finally:
        os.close(current)


def eviction_stages(target):
    """Relative candidate paths in eviction order, from the pinned target."""
    profiles = [name for name, is_directory in entries(target) if is_directory]
    nested = []
    for profile in profiles:
        child = os.open(profile, DIRECTORY, dir_fd=target)
        try:
            nested += [(profile, name) for name, is_directory in entries(child)
                       if is_directory and has_directory(child, name, 'deps')]
        finally:
            os.close(child)
    incremental = [(*path, 'incremental') for path in [(name,) for name in profiles] + nested
                   if has_directory(target, *path, 'incremental')]
    stale = [(name,) for name in profiles if name != GATE_PROFILE]
    everything = [(name,) for name, _ in entries(target) if name != 'CACHEDIR.TAG']
    return [('incremental', incremental), ('non-gate-profiles', stale), ('target-reset', everything)]


def remove_relative(target, path):
    parent = os.dup(target)
    try:
        for part in path[:-1]:
            child = os.open(part, DIRECTORY, dir_fd=parent)
            os.close(parent)
            parent = child
        remove_at(parent, path[-1])
    finally:
        os.close(parent)


def evict(target, maximum):
    """Shrink the pinned target below maximum in stages; returns removed paths."""
    removed = []
    for index in range(3):
        if usage_at(target) <= maximum:
            break
        # Each stage lists what remains after the previous one.
        stage, candidates = eviction_stages(target)[index]
        for path in candidates:
            if any(part in ('', '.', '..') or os.sep in part for part in path):
                raise Refused(f'unsafe eviction candidate {path}')
            remove_relative(target, path)
            removed.append({'stage': stage, 'path': '/'.join(path)})
    return removed


def holds_exclusive_flock(descriptor, inode):
    """Whether this exact open file description holds an exclusive flock on
    inode, read from /proc/self/fdinfo without acquiring anything. Linux lists
    the description's locks as `lock: N: FLOCK ADVISORY WRITE <pid>
    <maj>:<min>:<ino> 0 EOF` (device numbers in hex). The pid may be the
    parent that took it, so it is not compared. Unreadable means no."""
    try:
        with open(f'/proc/self/fdinfo/{descriptor}') as info:
            lines = info.read().splitlines()
    except OSError:
        return False
    wanted = (os.major(inode.st_dev), os.minor(inode.st_dev), inode.st_ino)
    for line in lines:
        fields = line.split()
        if not fields or fields[0] != 'lock:' or '->' in fields:
            continue
        try:
            kind, _, mode, _, device = fields[2:7]
            major, minor, ino = device.split(':')
            found = (int(major, 16), int(minor, 16), int(ino))
        except ValueError:
            continue
        if kind == 'FLOCK' and mode == 'WRITE' and found == wanted:
            return True
    return False


@contextlib.contextmanager
def held_lock(lock, environ):
    """Yield whether this process verifiably holds the target's exclusive lock.

    A runner that holds it passes its descriptor's number in MTG_ENGINE_LOCK_FD.
    That descriptor must be the canonical lock inode, and /proc/self/fdinfo must
    show its own open file description holding an exclusive FLOCK on that
    inode; nothing here locks an inherited descriptor, so an idle, shared-only
    or contended descriptor can never turn into a fresh lock. Without one, this
    call takes the lock itself without waiting. Every failure fails closed."""
    try:
        canonical = os.lstat(lock)
    except FileNotFoundError:
        canonical = None
    if canonical is not None and not stat.S_ISREG(canonical.st_mode):
        yield False
        return
    inherited = environ.get('MTG_ENGINE_LOCK_FD')
    if inherited is not None:
        try:
            descriptor = int(inherited)
            held = os.fstat(descriptor)
        except (ValueError, OSError):
            yield False
            return
        yield (canonical is not None
               and (held.st_dev, held.st_ino) == (canonical.st_dev, canonical.st_ino)
               and holds_exclusive_flock(descriptor, canonical))
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
    """Admit a native command; evict an over-budget target under its lock.

    The eviction decision uses the size measured through the pinned target
    descriptor, under the verified lock. The unlocked path-based status only
    reports, and may admit without eviction when another owner holds the lock."""
    env = os.environ if environ is None else environ
    report = status(root, env)
    maximum = report['maximum_bytes']
    budget_error = lambda error: 'ENGINE_CACHE_MAX_GIB' in error
    lock = Path(report['lock'])
    with held_lock(lock, env) as owned:
        if not owned:
            if env.get('MTG_ENGINE_LOCK_FD') is not None or report['allocated_bytes'] > maximum:
                report['errors'].append('target lock is busy or the inherited MTG_ENGINE_LOCK_FD does not hold it; nothing evicted')
            return report
        try:
            target = open_owned_target(root, report['target_dir'], lock)
        except Refused as error:
            if report['allocated_bytes'] > maximum:
                report['errors'].append(f"not this checkout's own Cargo cache: {error}; nothing evicted")
            return report
        try:
            pinned = usage_at(target)
            report['pinned_allocated_bytes'] = pinned
            if pinned <= maximum:
                report['errors'] = [e for e in report['errors'] if not budget_error(e)]
                return report
            removed = evict(target, maximum)
        finally:
            os.close(target)
    after = status(root, env)
    if any(row['stage'] == 'target-reset' for row in removed):
        # An emptied target holds only its tag: the budget cannot go lower.
        after['errors'] = [e for e in after['errors'] if not budget_error(e)]
    after['evicted'] = removed
    after['allocated_before_bytes'] = pinned
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
