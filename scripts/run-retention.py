#!/usr/bin/env python3
"""Bound retention storage, serialize Cargo, and discard only this run's scratch."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

spec = importlib.util.spec_from_file_location('engine_storage', Path(__file__).with_name('engine-storage.py'))
storage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(storage)
GIB = storage.GIB


def budget_check(scratch, store, scratch_limit, store_limit, reserve):
    scratch_bytes, store_bytes = storage.usage(scratch), storage.usage(store)
    if scratch_bytes > scratch_limit:
        raise RuntimeError(f'Retention scratch exceeds {scratch_limit} bytes')
    if store_bytes > store_limit:
        raise RuntimeError(f'Evidence store exceeds {store_limit} bytes; review retained runs or explicitly raise --store-gib')
    if any(shutil.disk_usage(p).free < reserve for p in (scratch, store)):
        raise RuntimeError(f'Retention requires {reserve} free bytes; stopping before disk exhaustion')
    return scratch_bytes, store_bytes


def stop_group(child):
    # The worker gets its own session: cancellation also stops cargo/rustc and
    # grandchildren before removing files they might still be writing.
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        child.wait()
        return
    try:
        child.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    child.wait()


def supervise(command, root, store, *, scratch_limit=32 * GIB, store_limit=128 * GIB,
              reserve=16 * GIB, interval=2, lock=storage.DEFAULT_LOCK):
    """Only one fixed scratch slot; SIGKILL leftovers block rather than accumulate."""
    root, store = Path(root).absolute(), Path(store).absolute()
    storage.no_symlinks(root)
    storage.no_symlinks(store)
    generated = root / 'generated'
    scratch = generated / '.retention-work'
    storage.no_symlinks(scratch)
    with storage.cargo_lock(lock):
        generated.mkdir(parents=True, exist_ok=True)
        store.mkdir(parents=True, exist_ok=True)
        # A previous SIGKILL/power loss may leave children or useful diagnostics.
        # Do not delete it implicitly, even when the advisory lock is free.
        scratch.mkdir()
        child = None
        previous = {}
        started = time.monotonic()
        peak_scratch = peak_store = 0
        outcome = {'status': 'failed', 'scratch': str(scratch), 'store': str(store)}
        def cancelled(signum, _frame):
            raise InterruptedError(f'Retention cancelled by signal {signum}')
        try:
            (scratch / 'owner.json').write_text(json.dumps({'version': 1, 'pid': os.getpid()}) + '\n')
            for sig in (signal.SIGINT, signal.SIGTERM):
                previous[sig] = signal.signal(sig, cancelled)
            peak_scratch, peak_store = budget_check(scratch, store, scratch_limit, store_limit, reserve)
            env = {**os.environ, 'MTG_RETENTION_SCRATCH': str(scratch), 'TMPDIR': str(scratch),
                   'CARGO_TARGET_DIR': str(scratch / 'initial-target'),
                   'MTG_ENGINE_ARTIFACT_ROOT': str(scratch / 'artifacts'),
                   'CARGO_INCREMENTAL': os.environ.get('CARGO_INCREMENTAL', '0')}
            child = subprocess.Popen(command, cwd=root, env=env, start_new_session=True)
            while True:
                scratch_bytes, store_bytes = budget_check(scratch, store, scratch_limit, store_limit, reserve)
                peak_scratch, peak_store = max(peak_scratch, scratch_bytes), max(peak_store, store_bytes)
                code = child.poll()
                if code is not None:
                    if code != 0:
                        raise RuntimeError(f'Retention worker exited {code}; no successful verification claimed')
                    break
                time.sleep(interval)
            outcome['status'] = 'worker_completed'
        except BaseException as error:
            outcome['error'] = str(error)
            raise
        finally:
            # Ignore repeated cancellation until descendants stop and cleanup ends.
            for sig in previous:
                signal.signal(sig, signal.SIG_IGN)
            try:
                if child is not None:
                    stop_group(child)
                shutil.rmtree(scratch)
                outcome.update({'scratch_removed': True, 'peak_sampled_scratch_bytes': peak_scratch,
                                'peak_sampled_store_bytes': peak_store,
                                'wall_seconds': time.monotonic() - started})
                (generated / 'retention-storage-latest.json').write_text(json.dumps(outcome, indent=2) + '\n')
            finally:
                for sig, handler in previous.items():
                    signal.signal(sig, handler)
        return outcome


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('kind', choices=('engine', 'server'))
    parser.add_argument('store', type=Path, nargs='?', help='Engine evidence root; server uses its verified predecessor')
    parser.add_argument('--scratch-gib', type=int, default=32)
    parser.add_argument('--store-gib', type=int, default=128)
    parser.add_argument('--reserve-gib', type=int, default=16)
    args = parser.parse_args()
    if min(args.scratch_gib, args.store_gib, args.reserve_gib) <= 0:
        parser.error('Storage bounds must be positive')
    root = Path(__file__).resolve().parent.parent
    if args.kind == 'server':
        if args.store:
            parser.error('Server retention uses its verified predecessor store')
        engine = json.loads((root / 'generated/p2-retention.json').read_text())
        store = Path(engine['bundle']).parent.parent
    else:
        store = args.store or Path.home() / '.local/share/mtg-ecosystem/engine-evidence'
    store = store.absolute()
    node = os.environ.get('npm_node_execpath') or shutil.which('node')
    if not node:
        raise RuntimeError('Node is required')
    command = [node, str(root / 'scripts' / f'retain-{args.kind}-evidence.ts')]
    if args.kind == 'engine':
        command.append(str(store))
    result = supervise(command, root, store, scratch_limit=args.scratch_gib * GIB,
                       store_limit=args.store_gib * GIB, reserve=args.reserve_gib * GIB)
    print(json.dumps(result))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, InterruptedError) as error:
        sys.exit(str(error))
