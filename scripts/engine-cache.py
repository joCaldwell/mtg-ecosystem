#!/usr/bin/env python3
"""Select and inspect a shared Cargo cache. Never move or delete cache/evidence bytes."""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
GIB = 1024 ** 3


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()[:16]


def cache_path(root=ROOT, environ=None):
    env = os.environ if environ is None else environ
    if env.get('CARGO_TARGET_DIR'):
        return Path(os.path.abspath(Path(env['CARGO_TARGET_DIR']).expanduser()))
    common = subprocess.check_output(['git', '-C', str(root), 'rev-parse', '--path-format=absolute', '--git-common-dir'], text=True).strip()
    rustc = subprocess.check_output(['rustc', '-vV'], text=True)
    cache_home = Path(env.get('XDG_CACHE_HOME', str(Path.home() / '.cache'))).expanduser()
    return (cache_home / 'mtg-ecosystem/cargo' / digest(str(Path(common).resolve())) / digest(rustc)).absolute()


def gib_setting(env, name, default):
    value = float(env.get(name, default))
    if not math.isfinite(value) or value < 0:
        raise ValueError(f'{name} must be a finite nonnegative GiB value')
    return int(value * GIB)


def status(root=ROOT, environ=None):
    env = os.environ if environ is None else environ
    path = cache_path(root, env)
    # Reuse the read-only allocated-byte walker; it never follows symlinks.
    spec = importlib.util.spec_from_file_location('engine_storage', ROOT / 'scripts/engine-storage.py')
    storage = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(storage)
    ancestor = path
    while not ancestor.exists():
        ancestor = ancestor.parent
    used = storage.usage(path.resolve()) if path.exists() else 0
    free = shutil.disk_usage(ancestor).free
    maximum = gib_setting(env, 'ENGINE_CACHE_MAX_GIB', '16')
    minimum = gib_setting(env, 'ENGINE_CACHE_MIN_FREE_GIB', '4')
    errors = []
    if path.exists() and not path.is_dir():
        errors.append('CARGO_TARGET_DIR exists but is not a directory')
    if used > maximum:
        errors.append('cache exceeds ENGINE_CACHE_MAX_GIB; inspect it and coordinate cleanup, or explicitly raise the budget')
    if free < minimum:
        errors.append('free disk below ENGINE_CACHE_MIN_FREE_GIB; reclaim reviewed disposable output before compiling')
    return {'target_dir': str(path), 'explicit_target': bool(env.get('CARGO_TARGET_DIR')),
            'allocated_bytes': used, 'filesystem_free_bytes': free, 'maximum_bytes': maximum,
            'minimum_free_bytes': minimum, 'incremental': env.get('CARGO_INCREMENTAL', '0'),
            'errors': errors, 'policy': 'preflight only; no automatic eviction; retained evidence is never a cache'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['path', 'status', 'check'])
    args = parser.parse_args()
    try:
        if args.command == 'path':
            print(cache_path())
            return 0
        report = status()
        print(json.dumps(report, indent=2))
        return bool(report['errors']) if args.command == 'check' else 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Engine cache: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
