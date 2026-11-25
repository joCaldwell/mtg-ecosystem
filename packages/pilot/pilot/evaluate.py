"""Compare a saved pilot with uniform masked play against three Develop bots.

    PYTHONPATH=packages/pilot python -m pilot.evaluate \
        --library target/engine/release/libmtg_env.so --device cuda:0 \
        --checkpoint generated/pilot.pt

Each fixed seed supplies episode zero of every lane. Both policies use the
same seed list and lane indices. Only natural finishes enter the Wilson 95%
interval, including draws as non-wins. The step cap reports any unfinished
lanes separately from native censors.
"""

import argparse
import json
import math
from pathlib import Path
import sys
from time import perf_counter

import numpy as np
import torch

from .checkpoint import load_checkpoint
from .model import Pilot, precision
from .rollout import Outcomes
from .schema import Schema, action_masks, library_digest, load_native, read_results, uniform_actions
from .train import PROFILE, configure_torch, make_env, positive_int


EVALUATION_SEEDS = (907901, 907903, 907907, 907909)


def wilson_95(wins: int, games: int) -> list[float] | None:
    if games == 0:
        return None
    z = 1.959963984540054
    fraction = wins / games
    denominator = 1 + z**2 / games
    center = (fraction + z**2 / (2 * games)) / denominator
    half_width = z * math.sqrt(fraction * (1 - fraction) / games + z**2 / (4 * games**2))
    half_width /= denominator
    return [max(0.0, center - half_width), min(1.0, center + half_width)]


@torch.no_grad()
def evaluate(native, schema, model, device, args) -> dict:
    outcomes = Outcomes()
    unfinished = 0
    started = perf_counter()
    if model is not None:
        with precision(device):
            cards = model.encode_catalog()

    for seed in args.seeds:
        configure_torch(device, args.torch_threads, seed)
        rng = np.random.default_rng(seed)
        env = make_env(native, args.profile, args.envs_per_seed, args.threads, seed, "baseline")
        active = np.ones(args.envs_per_seed, dtype=bool)
        for _ in range(args.max_steps):
            with env.buffers() as batch:
                mask, numeric = action_masks(batch)
                actions = uniform_actions(mask, rng)
                if model is not None:
                    rows = np.flatnonzero(active & (mask.sum(axis=1) > 1))
                    observations = schema.capture(batch, rows, mask, numeric)

            if model is not None and observations:
                inputs = schema.collate(observations, device)
                with precision(device):
                    log_probs, _ = model(inputs, cards)
                actions[rows] = torch.multinomial(log_probs.exp(), 1).squeeze(1).cpu().numpy()

            env.step(actions)
            with env.buffers() as batch:
                result = read_results(batch)
            for row in np.flatnonzero(active & result.done):
                outcomes.record(result.rewards[row], int(result.censors[row]))
                active[row] = False
            if not active.any():
                break
        unfinished += int(active.sum())
        print(f"seed {seed} done, {active.sum()} unfinished", file=sys.stderr, flush=True)
        del env

    natural = outcomes.natural
    finished = natural + outcomes.censored
    wins = outcomes.wins[0]
    return {
        **outcomes.summary(), "games_requested": len(args.seeds) * args.envs_per_seed,
        "unfinished_games": unfinished, "seat_0_wins": wins,
        "win_rate": wins / natural if natural else None,
        "win_rate_wilson_95": wilson_95(wins, natural),
        "censor_rate": outcomes.censored / finished if finished else None,
        "elapsed_seconds": perf_counter() - started,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path)
    parser.add_argument("--profile", default=PROFILE)
    parser.add_argument("--device", default="cpu")
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--seeds", nargs="+", type=int, default=EVALUATION_SEEDS)
    parser.add_argument("--envs-per-seed", type=positive_int, default=16)
    parser.add_argument("--threads", type=positive_int, default=16)
    parser.add_argument("--torch-threads", type=positive_int, default=4)
    parser.add_argument("--max-steps", type=positive_int, default=10000)
    args = parser.parse_args()
    if len(set(args.seeds)) != len(args.seeds):
        parser.error("evaluation seeds must be distinct")

    native = load_native(args.library)
    device = torch.device(args.device)
    configure_torch(device, args.torch_threads, args.seeds[0])
    env = make_env(native, args.profile, 1, 1, args.seeds[0], "baseline")
    schema = Schema(env, library_digest(native))
    del env
    model = Pilot(schema).to(device).eval()
    load_checkpoint(args.checkpoint, model, schema.identity)

    print("Evaluating the trained checkpoint", file=sys.stderr, flush=True)
    trained = evaluate(native, schema, model, device, args)
    print("Evaluating the uniform masked policy", file=sys.stderr, flush=True)
    random = evaluate(native, schema, None, device, args)
    print(json.dumps({
        "profile": args.profile, "seeds": args.seeds, "envs_per_seed": args.envs_per_seed,
        "trained": trained, "uniform_random": random,
    }), flush=True)


if __name__ == "__main__":
    main()
