"""Undiscounted terminal win PPO with a four-outcome value head."""

from dataclasses import dataclass

import numpy as np
import torch

from .model import Pilot, precision
from .rollout import Trajectory
from .schema import Observation, Schema


def segment_gae(
    values: np.ndarray, reward: float, bootstrap: float = 0.0, gae_lambda: float = 0.95,
) -> tuple[np.ndarray, np.ndarray]:
    """Use the terminal win indicator or the live boundary win probability."""
    returns = np.full(len(values), reward + bootstrap, dtype=np.float32)
    advantages = np.empty_like(returns)
    next_value = bootstrap
    next_advantage = 0.0
    for i in range(len(values) - 1, -1, -1):
        delta = reward + next_value - values[i]
        next_advantage = delta + gae_lambda * next_advantage
        advantages[i] = next_advantage
        next_value = values[i]
        reward = 0.0
    return returns, advantages


@dataclass
class UpdateBatch:
    observations: list[Observation]
    actions: np.ndarray
    old_log_probs: np.ndarray
    advantages: np.ndarray
    winner_targets: np.ndarray

    @classmethod
    def from_trajectories(cls, trajectories: list[Trajectory], seats: int):
        observations, actions, log_probs, advantages, targets = [], [], [], [], []
        for trajectory in trajectories:
            samples = trajectory.samples
            target = np.zeros(seats, dtype=np.float32)
            if trajectory.winner is not None:
                target[trajectory.winner] = 1
            bootstrap = 0.0
            if trajectory.bootstrap is not None:
                target = trajectory.bootstrap
                bootstrap = float(target[0])
            _, gae = segment_gae(
                np.asarray([s.win_probability for s in samples]),
                float(trajectory.winner == 0), bootstrap,
            )
            observations.extend(s.observation for s in samples)
            actions.extend(s.action for s in samples)
            log_probs.extend(s.log_probability for s in samples)
            advantages.extend(gae)
            targets.extend([target] * len(samples))

        advantage = np.asarray(advantages, dtype=np.float32)
        advantage = (advantage - advantage.mean()) / max(float(advantage.std()), 1e-8)
        return cls(
            observations, np.asarray(actions, dtype=np.int64),
            np.asarray(log_probs, dtype=np.float32), advantage,
            np.stack(targets),
        )


def optimize(
    model: Pilot,
    optimizer: torch.optim.Optimizer,
    scaler: torch.amp.GradScaler,
    schema: Schema,
    trajectories: list[Trajectory],
    device: torch.device,
    rng: np.random.Generator,
    *,
    minibatch: int,
    epochs: int,
) -> dict:
    batch = UpdateBatch.from_trajectories(trajectories, schema.model_shape.seats)
    size = len(batch.observations)
    totals = torch.zeros(3, device=device)

    def tensor(data, indices):
        return torch.from_numpy(data[indices]).to(device)

    for _ in range(epochs):
        ordering = rng.permutation(size)
        for start in range(0, size, minibatch):
            indices = ordering[start:start + minibatch]
            inputs = schema.collate([batch.observations[i] for i in indices], device)
            actions = tensor(batch.actions, indices)
            old = tensor(batch.old_log_probs, indices)
            advantages = tensor(batch.advantages, indices)
            winner_targets = tensor(batch.winner_targets, indices)

            optimizer.zero_grad(set_to_none=True)
            with precision(device):
                log_probs, values = model(inputs, model.encode_catalog())
            chosen = log_probs.gather(1, actions[:, None]).squeeze(1)
            ratio = (chosen - old).exp()
            policy_loss = -torch.minimum(
                ratio * advantages, ratio.clamp(0.8, 1.2) * advantages,
            ).mean()
            # Natural wins provide one-hot targets. Live boundaries provide
            # soft targets and natural draws yield a zero-weight row.
            value_loss = -(values.log_softmax(dim=1) * winner_targets).sum()
            value_loss = value_loss / winner_targets.any(dim=1).sum().clamp_min(1)
            entropy = -(log_probs.exp() * log_probs.masked_fill(~inputs.mask, 0)).sum(1).mean()
            # Imitation hook (unimplemented): capture a future expert_action
            # in the schema and add its masked cross-entropy here.
            loss = policy_loss + 0.5 * value_loss - 0.01 * entropy
            scaler.scale(loss).backward()
            scaler.unscale_(optimizer)
            torch.nn.utils.clip_grad_norm_(model.parameters(), 0.5)
            scaler.step(optimizer)
            scaler.update()
            totals += len(indices) * torch.stack((policy_loss, value_loss, entropy)).detach()

    policy, value, entropy = (totals / (epochs * size)).cpu().tolist()
    if not np.isfinite([policy, value, entropy]).all():
        raise FloatingPointError("Non-finite PPO loss")
    return {"samples": size, "policy_loss": policy, "value_loss": value, "entropy": entropy}
