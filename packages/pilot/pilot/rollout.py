"""Collect on-policy, per-seat trajectories from a leased VecEnv."""

from collections import Counter
from dataclasses import dataclass, field
from time import perf_counter

import numpy as np
import torch

from .model import Pilot, precision
from .schema import Observation, Schema, action_masks, read_results


@dataclass
class Sample:
    observation: Observation
    action: int
    log_probability: float
    win_probability: float


@dataclass
class Trajectory:
    samples: list[Sample]
    winner: int | None  # Viewer-relative seat. A natural draw has no winner.


@dataclass
class Outcomes:
    wins: Counter = field(default_factory=Counter)
    draws: int = 0
    censors: Counter = field(default_factory=Counter)

    def record(self, reward: np.ndarray, censor: int) -> int | None:
        if censor:
            self.censors[censor] += 1
            return None
        winners = np.flatnonzero(reward > 0)
        if len(winners) == 1:
            winner = int(winners[0])
            self.wins[winner] += 1
            return winner
        if not reward.any():
            self.draws += 1
            return None
        raise ValueError(f"Unexpected win reward: {reward}")

    @property
    def natural(self) -> int:
        return self.wins.total() + self.draws

    @property
    def censored(self) -> int:
        return self.censors.total()

    def summary(self) -> dict:
        return {
            "games_finished": self.natural + self.censored,
            "natural_outcomes": self.natural,
            "wins_by_seat": dict(self.wins),
            "draws": self.draws,
            "censored_games": self.censored,
            "censors_by_class": dict(self.censors),
        }


@dataclass
class CollectionStats:
    learner_decisions: int = 0
    forced_decisions: int = 0
    censored_decisions: int = 0
    unfinished_decisions: int = 0
    env_seconds: float = 0
    inference_seconds: float = 0
    outcomes: Outcomes = field(default_factory=Outcomes)


class Collector:
    def __init__(self, env, schema: Schema, device: torch.device):
        self.env = env
        self.schema = schema
        self.device = device
        self.stats = CollectionStats()

    @torch.no_grad()
    def collect(
        self, model: Pilot, target: int, games: int, max_steps: int, deadline: float,
    ) -> list[Trajectory]:
        """Keep only natural terminal trajectories gathered with this policy.

        At the collection boundary, drop unresolved histories. On the next
        collection those lanes can contribute fresh on-policy suffixes. GAE
        advances once per recorded choice of the same seat, with gamma = 1.
        """
        stats = self.stats
        started = perf_counter()
        with precision(self.device):
            cards = model.encode_catalog()
        if self.device.type == "cuda":
            torch.cuda.synchronize(self.device)
        stats.inference_seconds += perf_counter() - started

        trajectories = []
        accepted = 0
        natural_at_start = stats.outcomes.natural
        with self.env.buffers() as batch:
            seats = read_results(batch).seats
        seats_count = self.schema.model_shape.seats
        pending = [[[] for _ in range(seats_count)] for _ in seats]

        for _ in range(max_steps):
            if perf_counter() >= deadline:
                break
            with self.env.buffers() as batch:
                mask, numeric = action_masks(batch)
                actions = mask.argmax(axis=1).astype(np.int32)
                rows = np.flatnonzero(mask.sum(axis=1) > 1)
                observations = self.schema.capture(batch, rows, mask, numeric)

            if observations:
                started = perf_counter()
                inputs = self.schema.collate(observations, self.device)
                with precision(self.device):
                    log_probs, logits = model(inputs, cards)
                choices = torch.multinomial(log_probs.exp(), 1)
                values = logits.softmax(dim=1)[:, 0]
                payload = torch.cat((choices, log_probs.gather(1, choices), values[:, None]), dim=1)
                for row, observation, (action, log_prob, value) in zip(
                    rows, observations, payload.cpu().numpy(), strict=True,
                ):
                    actions[row] = int(action)
                    pending[row][seats[row]].append(Sample(observation, int(action), log_prob, value))
                stats.inference_seconds += perf_counter() - started

            stats.learner_decisions += len(rows)
            stats.forced_decisions += len(actions) - len(rows)
            started = perf_counter()
            self.env.step(actions)
            stats.env_seconds += perf_counter() - started

            with self.env.buffers() as batch:
                result = read_results(batch)
            for row in np.flatnonzero(result.done):
                censor = int(result.censors[row])
                winner = stats.outcomes.record(result.rewards[row], censor)
                for seat, samples in enumerate(pending[row]):
                    if samples:
                        if censor:
                            stats.censored_decisions += len(samples)
                        else:
                            relative = None if winner is None else (winner - seat) % seats_count
                            trajectories.append(Trajectory(samples, relative))
                            accepted += len(samples)
                        pending[row][seat] = []
            seats = result.seats
            if accepted >= target and stats.outcomes.natural - natural_at_start >= games:
                break

        stats.unfinished_decisions += sum(len(samples) for lane in pending for samples in lane)
        return trajectories
