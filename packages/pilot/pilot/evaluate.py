"""Compare saved pilots and uniform masked play against three Develop bots.

    PYTHONPATH=packages/pilot python -m pilot.evaluate \
        --library target/engine/release/libmtg_env.so --device cuda:0 \
        --checkpoint generated/pilot.pt \
        --rotate-profile packages/game-engine/tests/fixtures/profiles/commander-foundations-v1.json

Repeat --checkpoint to test multiple pilots on the same lanes. Each seed
supplies episode zero of every lane. Only natural finishes enter the Wilson
95% interval, including draws as non-wins. The step cap reports unfinished
lanes separately from native censors. --develop-report accepts the independent
all-Develop reference from mtg-sim bench-pods PROFILE --rotate-seats.
"""

import argparse
from collections import Counter
import json
import math
from pathlib import Path
import sys
from time import perf_counter

import numpy as np
import torch

from .checkpoint import load_checkpoint
from .model import Pilot, precision
from .pods import RotatedPods
from .rollout import Outcomes
from .schema import Schema, action_masks, library_digest, load_native, read_results, uniform_actions
from .train import PROFILE, configure_torch, make_env, positive_int


EVALUATION_SEEDS = (907911, 907913, 907917, 907919)


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


def evaluation_counts(outcomes: Outcomes, requested: int) -> dict:
    natural = outcomes.natural
    finished = natural + outcomes.censored
    wins = outcomes.wins[0]
    return {
        **outcomes.summary(), "games_requested": requested,
        "unfinished_games": requested - finished, "seat_0_wins": wins,
        "win_rate": wins / natural if natural else None,
        "win_rate_wilson_95": wilson_95(wins, natural),
        "censor_rate": outcomes.censored / finished if finished else None,
    }


def develop_reference(path: Path, rotation: RotatedPods, profile: str) -> dict:
    report = json.loads(path.read_text())
    if report["profile"] != profile or not report["rotate_seats"] or report["policy"].lower() != "develop":
        raise ValueError("Expected an all-Develop seat-rotation report for the evaluation profile")
    deck_counts = Counter()
    natural = {}
    incomplete = {}
    lineups = [json.dumps(table, sort_keys=True) for table in rotation.tables]
    seen = set()
    for row in report["rows"]:
        case = row["case"]
        offset = case["rotation"]
        key = (case["seed_index"], lineups[offset])
        if key in seen:
            continue
        seen.add(key)
        deck = rotation.learner_decks[offset]
        deck_counts[deck] += 1
        if row["outcome"] == "win":
            natural.setdefault(deck, Outcomes()).wins[row["winner"]] += 1
        elif row["outcome"] == "draw":
            natural.setdefault(deck, Outcomes()).draws += 1
        else:
            incomplete.setdefault(deck, Counter())[row["outcome"]] += 1
    results = {}
    for deck, count in deck_counts.items():
        counts = evaluation_counts(natural.get(deck, Outcomes()), count)
        del counts["censor_rate"], counts["censored_games"], counts["censors_by_class"]
        results[deck.sha256] = {
            "commander": deck.commander, **counts,
            "nonterminal_by_class": dict(incomplete.get(deck, {})),
        }
    return {
        "seed_start": report["seed_start"], "simulator_games": len(report["rows"]),
        "duplicate_rotation_cases": len(report["rows"]) - len(seen), "by_deck": results,
    }


@torch.no_grad()
def evaluate(native, schema, model, device, args, rotation) -> dict:
    outcomes = Outcomes()
    lane_decks = rotation.lane_decks(args.envs_per_seed) if rotation else ()
    deck_counts = Counter(lane_decks)
    by_deck = {deck: Outcomes() for deck in deck_counts}
    started = perf_counter()
    if model is not None:
        with precision(device):
            cards = model.encode_catalog()

    for seed in args.seeds:
        configure_torch(device, args.torch_threads, seed)
        rng = np.random.default_rng(seed)
        env = make_env(native, args.profile, args.envs_per_seed, args.threads, seed, "baseline", rotation)
        active = np.ones(args.envs_per_seed, dtype=bool)
        for _ in range(args.max_steps):
            with env.buffers() as batch:
                mask, numeric = action_masks(batch)
                actions = uniform_actions(mask, rng)
                rows = np.flatnonzero(active & (mask.sum(axis=1) > 1))
                observations = (
                    schema.capture(batch, rows, mask, numeric) if model is not None and rows.size else None
                )

            if observations is not None:
                with precision(device):
                    log_probs, _ = model(observations.tensors(device), cards)
                actions[rows] = torch.multinomial(log_probs.exp(), 1).squeeze(1).cpu().numpy()

            env.step(actions)
            with env.buffers() as batch:
                result = read_results(batch)
            for row in np.flatnonzero(active & result.done):
                reward, censor = result.rewards[row], int(result.censors[row])
                outcomes.record(reward, censor)
                if rotation:
                    by_deck[lane_decks[row]].record(reward, censor)
                active[row] = False
            if not active.any():
                break
        print(f"seed {seed} done, {active.sum()} unfinished", file=sys.stderr, flush=True)
        del env

    result = evaluation_counts(outcomes, len(args.seeds) * args.envs_per_seed)
    result["elapsed_seconds"] = perf_counter() - started
    if rotation:
        result["by_deck"] = {
            deck.sha256: {"commander": deck.commander, **evaluation_counts(by_deck[deck], count * len(args.seeds))}
            for deck, count in deck_counts.items()
        }
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path)
    parser.add_argument("--profile", default=PROFILE)
    parser.add_argument("--rotate-profile", type=Path)
    parser.add_argument("--develop-report", type=Path)
    parser.add_argument("--device", default="cpu")
    parser.add_argument("--checkpoint", type=Path, action="append", required=True)
    parser.add_argument("--seeds", nargs="+", type=int, default=EVALUATION_SEEDS)
    parser.add_argument("--envs-per-seed", type=positive_int, default=128)
    parser.add_argument("--threads", type=positive_int, default=16)
    parser.add_argument("--torch-threads", type=positive_int, default=4)
    parser.add_argument("--max-steps", type=positive_int, default=10000)
    args = parser.parse_args()
    if len(set(args.seeds)) != len(args.seeds):
        parser.error("evaluation seeds must be distinct")
    if len(set(args.checkpoint)) != len(args.checkpoint):
        parser.error("evaluation checkpoints must be distinct")
    rotation = RotatedPods(args.rotate_profile, args.profile) if args.rotate_profile else None
    if rotation and args.envs_per_seed % len(rotation.tables):
        parser.error("--envs-per-seed must be a multiple of the pod rotation count")
    if args.develop_report and rotation is None:
        parser.error("--develop-report requires --rotate-profile")

    native = load_native(args.library)
    device = torch.device(args.device)
    configure_torch(device, args.torch_threads, args.seeds[0])
    env = make_env(native, args.profile, 1, 1, args.seeds[0], "baseline", rotation)
    schema = Schema(env, library_digest(native))
    del env
    model = Pilot(schema).to(device).eval()

    trained = {}
    for checkpoint in args.checkpoint:
        print(f"Evaluating {checkpoint}", file=sys.stderr, flush=True)
        load_checkpoint(checkpoint, model, schema.identity)
        trained[str(checkpoint)] = evaluate(native, schema, model, device, args, rotation)
    print("Evaluating the uniform masked policy", file=sys.stderr, flush=True)
    result = {
        "profile": args.profile, "seeds": args.seeds, "envs_per_seed": args.envs_per_seed,
        "trained": next(iter(trained.values())) if len(trained) == 1 else trained,
        "uniform_random": evaluate(native, schema, None, device, args, rotation),
    }
    if args.develop_report:
        result["develop"] = develop_reference(args.develop_report, rotation, args.profile)
    print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
