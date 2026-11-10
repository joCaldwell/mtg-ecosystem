#!/usr/bin/env python3
"""Review linked worktrees; retire only explicit, inactive, published clean trees."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('storage', Path(__file__).with_name('engine-storage.py'))
storage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(storage)


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args])


def registered(root):
    # NUL format preserves spaces, newlines and unquoted paths.
    # https://git-scm.com/docs/git-worktree#_porcelain_format
    return [dict(field.decode().split(' ', 1) if b' ' in field else (field.decode(), '')
                 for field in block.split(b'\0') if field)
            for block in git(root, 'worktree', 'list', '--porcelain', '-z').split(b'\0\0') if block]


def ancestor(root, head, base):
    result = subprocess.run(['git', '-C', str(root), 'merge-base', '--is-ancestor', head, base])
    if result.returncode not in (0, 1):
        raise RuntimeError('Cannot establish commit ancestry')
    return result.returncode == 0


def plan(root, worktree, base, remote='origin', remote_branch='feat/engine-plan'):
    root, worktree = Path(root).resolve(), Path(worktree).absolute()
    storage.no_symlinks(worktree)
    entries = registered(root)
    entry = next((row for row in entries if row['worktree'] == str(worktree)), None)
    if entry is None:
        raise ValueError('Select an exact registered worktree root')
    blockers = []
    if worktree == Path(entries[0]['worktree']) or worktree == root:
        blockers.append('primary or invoking worktree')
    if 'locked' in entry or 'prunable' in entry or 'detached' in entry:
        blockers.append('locked, prunable or detached worktree')
    branch = entry.get('branch', '').removeprefix('refs/heads/')
    if branch in ('main', 'master', base.removeprefix('refs/heads/'), remote_branch):
        blockers.append('protected or integration branch')
    base_sha = git(root, 'rev-parse', '--verify', base + '^{commit}').decode().strip()
    if not ancestor(root, entry['HEAD'], base_sha):
        blockers.append('unique commits outside integration base')
    # Ask the server rather than trusting a stale remote-tracking ref.
    remote_rows = git(root, 'ls-remote', '--heads', remote, 'refs/heads/' + remote_branch).decode().splitlines()
    published = next((line.split()[0] for line in remote_rows
                      if line.split()[1] == 'refs/heads/' + remote_branch), None)
    if published != base_sha:
        blockers.append('integration base is not the exact live published remote head')
    if git(worktree, 'status', '--porcelain', '-z', '--untracked-files=all'):
        blockers.append('tracked modifications or untracked files')
    ignored = [p.decode() for p in git(worktree, 'ls-files', '--others', '--ignored',
                                      '--exclude-standard', '--directory', '-z').split(b'\0') if p]
    if ignored:
        blockers.append('ignored files need separate preservation review')
    refs = subprocess.run(['git', '-C', str(root), 'grep', '-l', '-F', '-e', str(worktree),
                           '-e', branch, '--', 'docs', 'packages/*/AGENTS.md'], capture_output=True)
    if refs.returncode not in (0, 1):
        raise RuntimeError('Cannot inspect documented references')
    references = refs.stdout.decode().splitlines()
    if references:
        blockers.append('documented path or branch references')
    activity = storage.active_processes(worktree)
    if activity['active']:
        blockers.append('active process references')
    return {'worktree': str(worktree), 'branch': branch, 'head': entry['HEAD'],
            'base': base, 'base_head': base_sha, 'published_head': published,
            'blockers': blockers, 'ignored': ignored, 'references': references, 'activity': activity}


def retire(root, worktree, base, remote='origin', remote_branch='feat/engine-plan',
           apply=False, expected_head=None, inactive=False, allow_uninspectable=False,
           lock=storage.DEFAULT_LOCK):
    with storage.cargo_lock(lock):
        result = plan(root, worktree, base, remote, remote_branch)
        result['mode'] = 'apply' if apply else 'dry-run'
        if apply:
            if not inactive or expected_head != result['head']:
                raise ValueError('Apply requires --inactive and exact --expect-head from reviewed dry-run')
            if result['blockers']:
                raise ValueError('Retirement blocked: ' + '; '.join(result['blockers']))
            if result['activity']['uninspectable'] and not allow_uninspectable:
                raise ValueError('Uninspectable PIDs require independent ownership review and --allow-uninspectable')
            # Git performs its own final dirty check. Never --force; keep both
            # local and remote branches as additional recovery references.
            subprocess.run(['git', '-C', str(root), 'worktree', 'remove', str(worktree)], check=True)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument('--worktree', type=Path, required=True)
    parser.add_argument('--base', default='feat/engine-plan')
    parser.add_argument('--remote', default='origin')
    parser.add_argument('--remote-branch', default='feat/engine-plan')
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--expect-head')
    parser.add_argument('--inactive', action='store_true', help='Owner has finished and will not resume in this checkout')
    parser.add_argument('--allow-uninspectable', action='store_true')
    args = parser.parse_args()
    print(json.dumps(retire(args.root, args.worktree, args.base, args.remote, args.remote_branch,
                            args.apply, args.expect_head, args.inactive, args.allow_uninspectable), indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
