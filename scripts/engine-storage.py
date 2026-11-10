#!/usr/bin/env python3
"""Read-only storage inventory and deliberately narrow Cargo cache cleanup."""
import argparse
import contextlib
import fcntl
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

GIB = 1024 ** 3
DEFAULT_LOCK = Path.home() / '.local/share/mtg-ecosystem/engine-verification/cargo.lock'


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def worktrees(root):
    return [Path(line[9:]) for line in git(root, 'worktree', 'list', '--porcelain').splitlines()
            if line.startswith('worktree ')]


def usage(root):
    """Allocated bytes, without following links; count hard links once per row."""
    seen = set()
    total = 0
    pending = [Path(root)]
    while pending:
        entry = pending.pop()
        info = entry.lstat()
        key = (info.st_dev, info.st_ino)
        if key in seen:
            continue
        seen.add(key)
        total += info.st_blocks * 512
        if entry.is_dir() and not entry.is_symlink():
            pending.extend(entry.iterdir())
    return total


def no_symlinks(root, recursive=False):
    root = Path(root).absolute()
    for entry in [root, *root.parents]:
        if entry.is_symlink():
            raise ValueError(f'Symlink refused: {entry}')
    if recursive:
        for current, dirs, files in os.walk(root, followlinks=False):
            for name in dirs + files:
                entry = Path(current) / name
                if entry.is_symlink():
                    raise ValueError(f'Symlink refused: {entry}')


def within(entry, root):
    return entry == root or root in entry.parents


def active_processes(root):
    """Inspect only path references, never arguments, environment or file contents."""
    if not Path('/proc/self/fd').is_dir():
        raise RuntimeError('Cleanup requires Linux /proc activity inspection')
    active, uninspectable = [], []
    for process in Path('/proc').iterdir():
        if not process.name.isdecimal() or int(process.name) == os.getpid():
            continue
        try:
            if process.stat().st_uid != os.getuid():
                continue
            refs = [process / 'cwd', process / 'exe', *list((process / 'fd').iterdir())]
            for ref in refs:
                try:
                    target = Path(os.readlink(ref).removesuffix(' (deleted)'))
                    if target.is_absolute() and within(target, root):
                        active.append(int(process.name))
                        break
                except FileNotFoundError:
                    pass
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError:
            uninspectable.append(int(process.name))
    return {'active': active, 'uninspectable': uninspectable}


@contextlib.contextmanager
def cargo_lock(filename=DEFAULT_LOCK):
    no_symlinks(filename)
    filename.parent.mkdir(parents=True, exist_ok=True)
    with filename.open('a') as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError(f'Cargo lock busy: {filename}; retry when its owner finishes') from error
        yield


def incremental_plan(worktree, max_bytes=2 * GIB):
    worktree = Path(worktree).absolute()
    no_symlinks(worktree)
    if worktree not in worktrees(worktree) or Path(git(worktree, 'rev-parse', '--show-toplevel')) != worktree:
        raise ValueError('Select an exact registered worktree root')
    target = worktree / 'target'
    if not target.exists():
        return []
    no_symlinks(target)
    # Cargo's host and explicit-target layouts only. No arbitrary user paths.
    profiles = [target / name for name in ('debug', 'release')]
    profiles += [entry / name for entry in target.iterdir() if entry.is_dir()
                 and entry.name not in ('debug', 'release') for name in ('debug', 'release')]
    rows = []
    for profile in profiles:
        candidate = profile / 'incremental'
        if not candidate.exists() and not candidate.is_symlink():
            continue
        no_symlinks(candidate, recursive=True)
        if not within(candidate, target) or not candidate.is_dir():
            raise ValueError(f'Invalid incremental cache: {candidate}')
        if git(worktree, 'ls-files', '--', str(candidate.relative_to(worktree))):
            raise ValueError(f'Tracked files in cache: {candidate}')
        size = usage(candidate)
        rows.append({'path': str(candidate), 'allocated_bytes': size})
    # The budget applies to the sum of incremental caches in this worktree.
    return rows if sum(row['allocated_bytes'] for row in rows) > max_bytes else []


def prune_incremental(worktree, max_bytes=2 * GIB, apply=False, lock=DEFAULT_LOCK, allow_uninspectable=False):
    if max_bytes < 0:
        raise ValueError('max-bytes must be nonnegative')
    with cargo_lock(lock):
        rows = incremental_plan(worktree, max_bytes)
        target = Path(worktree).absolute() / 'target'
        activity = active_processes(target) if rows else {'active': [], 'uninspectable': []}
        if activity['active']:
            raise RuntimeError(f"Target is active in PIDs {activity['active']}; nothing removed")
        if apply and activity['uninspectable'] and not allow_uninspectable:
            raise RuntimeError(f"Cannot inspect PIDs {activity['uninspectable']}; establish ownership independently before --allow-uninspectable")
        before = usage(target) if target.exists() else 0
        if apply:
            # Validate all candidates before mutating any; the lock protects
            # cooperating writers, /proc catches current uncoordinated users.
            for row in rows:
                shutil.rmtree(row['path'])
        after = usage(target) if target.exists() else 0
        return {'mode': 'apply' if apply else 'dry-run', 'worktree': str(Path(worktree).absolute()),
                'category': 'reproducible-cargo-incremental', 'candidates': rows, 'activity': activity,
                'candidate_bytes': sum(row['allocated_bytes'] for row in rows),
                'target_before_bytes': before, 'target_after_bytes': after,
                'reclaimed_bytes': before - after}


def inventory(root):
    rows = []
    for worktree in worktrees(root):
        for name, category in [('target', 'reproducible-build'), ('generated', 'mixed-generated-review-before-cleanup'),
                               ('.scryfall-cache', 'downloaded-pinned-inputs'),
                               ('packages/game-server/sdk/generated', 'reproducible-sdk')]:
            entry = worktree / name
            if entry.is_symlink():
                rows.append({'path': str(entry), 'category': category, 'symlink': True, 'allocated_bytes': 0})
            elif entry.exists():
                rows.append({'path': str(entry), 'category': category, 'allocated_bytes': usage(entry)})
    store = Path.home() / '.local/share/mtg-ecosystem'
    for name in ('engine-evidence', 'engine-toolchains', 'engine-verification'):
        entry = store / name
        if entry.exists() and not entry.is_symlink():
            rows.append({'path': str(entry), 'category': name, 'allocated_bytes': usage(entry)})
    return {'rows': sorted(rows, key=lambda row: -row['allocated_bytes']),
            'note': 'Rows count allocated blocks without following symlinks; cross-row hard links may double count. No source, worktree, corpus or evidence deletion is supported.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    audit = commands.add_parser('inventory')
    audit.add_argument('--root', type=Path, default=Path(__file__).resolve().parent.parent)
    prune = commands.add_parser('prune-cargo', help='Drop only incremental caches exceeding budget; dry-run by default')
    prune.add_argument('--worktree', type=Path, required=True)
    prune.add_argument('--max-bytes', type=int, default=2 * GIB)
    prune.add_argument('--apply', action='store_true')
    prune.add_argument('--allow-uninspectable', action='store_true', help='Acknowledge independently checked ownership when /proc denies some same-user processes')
    args = parser.parse_args()
    result = inventory(args.root) if args.command == 'inventory' else prune_incremental(args.worktree, args.max_bytes, args.apply, allow_uninspectable=args.allow_uninspectable)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
