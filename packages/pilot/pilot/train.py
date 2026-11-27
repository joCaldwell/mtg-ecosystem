"""Run PPO with Python and a locally built mtg-env extension.

From the repo root, using a Python with PyTorch and NumPy installed:

    PYTHONPATH=packages/pilot python -m pilot.train \
        --library target/engine/release/libmtg_env.so --device cuda:0 \
        --opponents baseline --seconds 1800 --checkpoint generated/pilot.pt

Pass --rotate-profile the embedded profile's source JSON to rotate its decks.
Use --device cpu for CPU execution and --opponents self_play for all four seats.
The extension build command is in the repo's batched NumPy environment guide.
Output is JSON Lines with per-update progress and a final throughput summary.
"""

import argparse
from dataclasses import asdict
import json
from pathlib import Path
import resource
from tempfile import TemporaryDirectory
from time import perf_counter

import numpy as np
import torch

from .checkpoint import load_checkpoint, save_checkpoint
from .model import Pilot
from .pods import RotatedPods
from .ppo import optimize
from .rollout import Collector
from .schema import Schema, library_digest, load_native


PROFILE = "commander-foundations-v1"


def positive_int(value: str) -> int:
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("expected a positive integer")
    return number


def configure_torch(device: torch.device, cpu_threads: int, seed: int) -> None:
    torch.set_num_threads(cpu_threads)
    torch.random.default_generator.manual_seed(seed)
    if device.type == "cuda":
        if device.index is not None:
            torch.cuda.set_device(device)
        torch.cuda.manual_seed(seed)


def make_env(native, profile, envs, threads, seed, opponents, rotation=None):
    config = {"opponents": opponents}
    if rotation is None:
        env = native.VecEnv(profile, num_envs=envs, threads=threads, seed=seed, config=config)
    else:
        with TemporaryDirectory(prefix="pilot-pods-") as directory:
            pods = rotation.write(Path(directory))
            env = native.VecEnv(
                profile, pods=pods, num_envs=envs, threads=threads, seed=seed, config=config,
            )
    env.reset()
    return env

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path)
    parser.add_argument("--profile", default=PROFILE)
    parser.add_argument("--rotate-profile", type=Path)
    parser.add_argument("--device", default="cpu")
    parser.add_argument("--opponents", default="self_play", choices=("self_play", "baseline"))
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--resume", type=Path)
    parser.add_argument("--seconds", type=positive_int, default=1800)
    parser.add_argument("--envs", type=positive_int, default=32)
    parser.add_argument("--threads", type=positive_int, default=16)
    parser.add_argument("--torch-threads", type=positive_int, default=4)
    parser.add_argument("--seed", type=int, default=57)
    parser.add_argument("--rollout-samples", type=positive_int, default=4096)
    parser.add_argument("--rollout-games", type=int, default=0)
    parser.add_argument("--rollout-steps", type=positive_int, default=4096)
    parser.add_argument("--minibatch", type=positive_int, default=256)
    parser.add_argument("--epochs", type=positive_int, default=2)
    args = parser.parse_args()
    if args.rollout_games < 0:
        parser.error("--rollout-games must be nonnegative")
    rotation = RotatedPods(args.rotate_profile, args.profile) if args.rotate_profile else None
    if rotation and args.envs % len(rotation.tables):
        parser.error("--envs must be a multiple of the pod rotation count")

    native = load_native(args.library)
    device = torch.device(args.device)
    configure_torch(device, args.torch_threads, args.seed)
    rng = np.random.default_rng(args.seed)
    env = make_env(native, args.profile, args.envs, args.threads, args.seed, args.opponents, rotation)
    schema = Schema(env, library_digest(native))
    model = Pilot(schema).to(device)
    optimizer = torch.optim.AdamW(model.parameters(), lr=3e-4)
    scaler = torch.amp.GradScaler(device.type, enabled=device.type == "cuda")
    if args.resume:
        load_checkpoint(args.resume, model, schema.identity, optimizer)
    args.checkpoint.parent.mkdir(parents=True, exist_ok=True)
    collector = Collector(env, schema, device)

    if device.type == "cuda":
        torch.cuda.reset_peak_memory_stats(device)
    print(json.dumps({
        "event": "start", "profile": args.profile, "opponents": args.opponents,
        "device": str(device), "torch": torch.__version__, "numpy": np.__version__,
        "seed": args.seed, "envs": args.envs, "threads": args.threads,
        "torch_threads": args.torch_threads, "run_seconds": args.seconds,
        "rollout_samples": args.rollout_samples, "rollout_games": args.rollout_games,
        "rollout_steps": args.rollout_steps, "minibatch": args.minibatch, "epochs": args.epochs,
        "parameters": sum(p.numel() for p in model.parameters()),
        "learner_deck_allocation": rotation.allocation(args.envs) if rotation else None,
        "identity": asdict(schema.identity),
    }), flush=True)

    started = perf_counter()
    deadline = started + args.seconds
    update_seconds = 0.0
    updates = trained_samples = 0
    while perf_counter() < deadline:
        model.eval()
        trajectories = collector.collect(
            model, args.rollout_samples, args.rollout_games, args.rollout_steps, deadline,
        )
        update = {}
        if trajectories:
            model.train()
            update_start = perf_counter()
            update = optimize(
                model, optimizer, scaler, schema, trajectories, device, rng,
                minibatch=args.minibatch, epochs=args.epochs,
            )
            update_seconds += perf_counter() - update_start
            updates += 1
            trained_samples += update["samples"]
            del trajectories
            save_checkpoint(args.checkpoint, model, optimizer, schema.identity)
        print(json.dumps({
            "event": "rollout", "elapsed_seconds": perf_counter() - started, "updates": updates,
            "learner_decisions": collector.stats.learner_decisions,
            "bootstrapped_decisions": collector.stats.bootstrapped_decisions,
            "env_seconds": collector.stats.env_seconds,
            "inference_seconds": collector.stats.inference_seconds,
            "update_seconds": update_seconds,
            "peak_rss_bytes": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * 1024,
            **update,
            **collector.stats.outcomes.summary(),
        }), flush=True)

    elapsed = perf_counter() - started
    stats = collector.stats
    print(json.dumps({
        "event": "summary", "elapsed_seconds": elapsed, "updates": updates,
        "learner_decisions": stats.learner_decisions, "forced_decisions": stats.forced_decisions,
        "trained_samples": trained_samples, "censored_decisions": stats.censored_decisions,
        "bootstrapped_decisions": stats.bootstrapped_decisions,
        "learner_decisions_per_second": stats.learner_decisions / elapsed,
        "env_seconds": stats.env_seconds, "inference_seconds": stats.inference_seconds,
        "update_seconds": update_seconds,
        "other_seconds": elapsed - stats.env_seconds - stats.inference_seconds - update_seconds,
        "peak_rss_bytes": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * 1024,
        "peak_vram_allocated_bytes": (
            torch.cuda.max_memory_allocated(device) if device.type == "cuda" else 0
        ),
        "peak_vram_reserved_bytes": (
            torch.cuda.max_memory_reserved(device) if device.type == "cuda" else 0
        ),
        **stats.outcomes.summary(),
    }), flush=True)
    if updates == 0:
        raise SystemExit("No trajectory contributed a PPO update")


if __name__ == "__main__":
    main()
