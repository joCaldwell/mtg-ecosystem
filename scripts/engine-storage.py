#!/usr/bin/env python3
"""Read-only storage inventory and the safety primitives worktree retirement uses.

Cargo targets are bounded by engine-cache.py check, never cleaned here."""
import argparse
import contextlib
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys



def worktree_lock(worktree):
    """The native lock of a worktree's default Cargo target (engine-cache.py owns the key)."""
    spec = importlib.util.spec_from_file_location('engine_cache', Path(__file__).with_name('engine-cache.py'))
    cache = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(cache)
    return cache.worktree_lock(Path(worktree).absolute())


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
        try:
            info = entry.lstat()
        except FileNotFoundError:
            continue  # Compiler temp files can disappear during a read-only scan.
        key = (info.st_dev, info.st_ino)
        if key in seen:
            continue
        seen.add(key)
        total += info.st_blocks * 512
        if entry.is_dir() and not entry.is_symlink():
            try:
                pending.extend(entry.iterdir())
            except FileNotFoundError:
                pass
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
def cargo_lock(filename):
    filename = Path(filename)
    no_symlinks(filename)
    filename.parent.mkdir(parents=True, exist_ok=True)
    with filename.open('a') as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError(f'Cargo lock busy: {filename}; retry when its owner finishes') from error
        yield


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
    args = parser.parse_args()
    print(json.dumps(inventory(args.root), indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
