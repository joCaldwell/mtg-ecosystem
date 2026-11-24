"""Run against an installed mtg_env or the local cdylib supplied with --library."""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import time

import numpy as np


def load(path):
    if path:
        spec = importlib.util.spec_from_file_location("mtg_env", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    import mtg_env
    return mtg_env


def rss():
    text = Path("/proc/self/status").read_text()
    return int(next(line.split()[1] for line in text.splitlines() if line.startswith("VmRSS:"))) * 1024


def actions(batch, rng):
    # Uniform categorical choice on the active head. Padded rows have zero mask.
    head = batch["head"].astype(bool)
    mask = batch["ptr_mask"].copy()
    mask[head] = 0
    mask[head, :64] = batch["num_mask"][head]
    if not mask.any(axis=1).all():
        raise AssertionError("empty mask on active head")
    return (rng.random(mask.shape) * mask).argmax(axis=1).astype(np.int32)


def run(args):
    mod = load(args.library)
    before = rss()
    env = mod.VecEnv("commander-foundations-v1", None, args.batch, args.threads, args.seed)
    env.reset()
    rng = np.random.default_rng(args.seed)
    completed = censored = decisions = steps = 0
    previous = np.zeros(args.batch, dtype=np.int64)
    started = time.monotonic()
    while (completed < args.games if args.games else steps < args.steps):
        with env.buffers() as batch:
            a = actions(batch, rng)
        env.step(a)
        steps += 1
        with env.buffers() as batch:
            info = batch["info"].copy()
            done = batch["done"].astype(bool)
        if info[:, 0].any():
            raise AssertionError("masked choice rejected")
        decisions += int((info[:, 3] - previous).sum())
        previous = np.where(done, 0, info[:, 3])
        completed += int((done & (info[:, 2] == 0)).sum())
        censored += int((done & (info[:, 2] != 0)).sum())
        if args.games and (done.any() or steps % 4096 == 0):
            print(f"steps={steps} completed={completed} censored={censored} seconds={time.monotonic()-started:.1f}", file=sys.stderr, flush=True)
    elapsed = time.monotonic() - started
    resident = rss()
    return dict(python=sys.version.split()[0], numpy=np.__version__, profile="commander-foundations-v1",
                threads=args.threads, batch=args.batch, seed=args.seed, decisions=decisions,
                micro_steps=steps * args.batch, completed_games=completed, censored_games=censored,
                elapsed_seconds=elapsed, decisions_per_second=decisions / elapsed,
                micro_steps_per_second=steps * args.batch / elapsed, rss_bytes=resident,
                rss_before_env_bytes=before, incremental_mib_per_game=(resident-before)/args.batch/2**20)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--library", type=Path)
    parser.add_argument("--batch", type=int, default=32)
    parser.add_argument("--threads", type=int, default=32)
    parser.add_argument("--steps", type=int, default=256)
    parser.add_argument("--games", type=int, default=0)
    parser.add_argument("--seed", type=int, default=57)
    print(json.dumps(run(parser.parse_args())), flush=True)
