"""CPU checks. Run with PYTHONPATH=packages/pilot python -m unittest discover -s packages/pilot/tests."""

from contextlib import contextmanager
from dataclasses import replace
from pathlib import Path
from tempfile import TemporaryDirectory
from time import perf_counter
from types import SimpleNamespace
from unittest import TestCase

import numpy as np
import torch

from pilot.checkpoint import load_checkpoint, save_checkpoint
from pilot.model import masked_policy
from pilot.pods import RotatedPods
from pilot.ppo import UpdateBatch, segment_gae
from pilot.rollout import Collector
from pilot.schema import Identity, action_masks, uniform_actions


class MaskTests(TestCase):
    def test_active_heads_and_sample_support(self):
        batch = {
            "ptr_mask": np.array([[1, 0, 0, 1], [1, 0, 0, 1]], dtype=np.uint8),
            "num_mask": np.array([[0, 1], [0, 1]], dtype=np.uint8),
            "head": np.array([0, 1], dtype=np.uint8),
        }
        mask, numeric = action_masks(batch)
        np.testing.assert_array_equal(numeric, [False, True])
        np.testing.assert_array_equal(mask, [[1, 0, 0, 1], [0, 1, 0, 0]])
        rng = np.random.default_rng(14)
        observed = {tuple(uniform_actions(mask, rng)) for _ in range(32)}
        self.assertEqual(observed, {(0, 1), (3, 1)})

        logits = torch.tensor([[0., 80., 9., 0.], [100., 0., 100., 100.]])
        log_prob = masked_policy(logits, torch.from_numpy(mask))
        torch.testing.assert_close(log_prob.exp(), torch.tensor([[0.5, 0., 0., 0.5], [0., 1., 0., 0.]]))
        self.assertEqual(log_prob[1, 1].item(), 0)

    def test_empty_active_head_rejected(self):
        with self.assertRaisesRegex(ValueError, "Empty published mask"):
            action_masks({"ptr_mask": np.ones((1, 2)), "num_mask": np.zeros((1, 1)), "head": np.ones(1)})


class AdvantageTests(TestCase):
    def test_terminal_win_and_zero_bootstrap(self):
        values = np.array([0.2, 0.3, 0.5], dtype=np.float32)
        returns, advantages = segment_gae(values, 1.0)
        np.testing.assert_allclose(returns, [1, 1, 1])
        np.testing.assert_allclose(advantages, [0.74125, 0.675, 0.5])

        returns, advantages = segment_gae(values, 0.0)
        np.testing.assert_allclose(returns, [0, 0, 0])
        np.testing.assert_allclose(advantages, [-0.16125, -0.275, -0.5], atol=1e-7)

    def test_live_segment_targets_use_the_next_win_probability(self):
        values = np.array([0.2, 0.3], dtype=np.float32)
        returns, advantages = segment_gae(values, 0.0, 0.8)
        np.testing.assert_allclose(returns, [0.8, 0.8])
        np.testing.assert_allclose(advantages, [0.575, 0.5])


class RolloutTests(TestCase):
    def fixture(self):
        native_row = {
            "seat": np.array([1, 3]), "reward": np.zeros((2, 4), np.int32),
            "done": np.zeros(2), "info": np.zeros((2, 8), np.int32),
            "head": np.zeros(2), "ptr_mask": np.ones((2, 2)), "num_mask": np.zeros((2, 1)),
        }
        test = self

        class Env:
            leased = False
            steps = 0

            @contextmanager
            def buffers(self):
                self.leased = True
                try:
                    yield native_row
                finally:
                    self.leased = False

            def step(self, actions):
                test.assertFalse(self.leased)
                self.steps += 1
                if self.steps == 1:
                    native_row["seat"][:] = [2, 0]
                    native_row["ptr_mask"][0, 1] = 0
                else:
                    test.assertEqual(actions[0], 0)
                    native_row["seat"][:] = [0, 2]
                    native_row["done"][:] = 1
                    native_row["reward"][0] = [-1, -1, 1, -1]
                    native_row["info"][1, 2] = 3

        class UniformPolicy:
            def encode_catalog(self):
                pass

            def __call__(self, mask, cards):
                return masked_policy(torch.zeros_like(mask, dtype=torch.float32), mask), torch.zeros((len(mask), 4))

            def value_logits(self, mask, cards):
                return torch.tensor([0.1, 0.2, 0.3, 0.4]).log().repeat(len(mask), 1)

        def capture(batch, rows, mask, numeric):
            chosen = mask[rows]
            return SimpleNamespace(
                tensors=lambda device: torch.from_numpy(chosen),
                rows=lambda: iter(chosen),
            )

        schema = SimpleNamespace(
            model_shape=SimpleNamespace(seats=4),
            capture=capture,
        )
        return Collector(Env(), schema, torch.device("cpu")), UniformPolicy()

    def test_previous_seat_is_scored_after_reset_and_censors_are_dropped(self):
        collector, model = self.fixture()
        completed = collector.collect(model, 1, 1, 2, perf_counter() + 30)
        self.assertEqual([(len(t.samples), t.winner) for t in completed], [(1, 1)])
        self.assertEqual(collector.stats.outcomes.wins, {2: 1})
        self.assertEqual(collector.stats.outcomes.censors, {3: 1})
        self.assertEqual(collector.stats.censored_decisions, 2)
        self.assertEqual(collector.stats.forced_decisions, 1)

    def test_live_boundary_rotates_forecast_and_next_update_continues_episode(self):
        collector, model = self.fixture()
        live = collector.collect(model, 1, 0, 2, perf_counter() + 30)
        self.assertEqual(collector.env.steps, 1)
        self.assertEqual(collector.stats.bootstrapped_decisions, 2)
        self.assertEqual([len(t.samples) for t in live], [1, 1])
        np.testing.assert_allclose(
            UpdateBatch.from_trajectories(live, 4).winner_targets,
            [[0.4, 0.1, 0.2, 0.3], [0.4, 0.1, 0.2, 0.3]],
        )
        self.assertEqual(collector.collect(model, 1, 0, 1, perf_counter() + 30), [])
        self.assertEqual(collector.env.steps, 2)
        self.assertEqual(collector.stats.censored_decisions, 1)
        self.assertEqual(collector.stats.outcomes.wins, {2: 1})


class RotationTests(TestCase):
    def test_foundations_rotations_preserve_the_pod_and_balance_seat_zero(self):
        fixtures = Path(__file__).resolve().parents[2] / "game-engine/tests/fixtures"
        rotation = RotatedPods(fixtures / "profiles/commander-foundations-v1.json", "commander-foundations-v1")
        first, second, third, fourth = rotation.tables
        self.assertEqual(first, third)
        self.assertEqual(second, fourth)
        self.assertEqual(second, (first[-1], *first[:-1]))
        self.assertNotEqual(first[0], second[0])
        self.assertEqual([row["lanes"] for row in rotation.allocation(512)], [256, 256])


class CheckpointTests(TestCase):
    def test_identity_mismatch_refused_before_loading_weights(self):
        model = torch.nn.Linear(3, 2)
        optimizer = torch.optim.AdamW(model.parameters(), lr=0.04)
        model(torch.ones(1, 3)).sum().backward()
        optimizer.step()
        identity = Identity(1, "binary", "catalog", "vocab")
        with TemporaryDirectory() as directory:
            path = Path(directory) / "pilot.pt"
            save_checkpoint(path, model, optimizer, identity)
            restored = torch.nn.Linear(3, 2)
            initial_weight = restored.weight.detach().clone()
            restored_optimizer = torch.optim.AdamW(restored.parameters())
            mismatches = {
                "observation_version": 2, "native_library_sha256": "other build",
                "card_table_sha256": "other catalog", "vocabulary_sha256": "other vocab",
            }
            for field, value in mismatches.items():
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, field):
                    load_checkpoint(path, restored, replace(identity, **{field: value}))
                torch.testing.assert_close(restored.weight, initial_weight)
            load_checkpoint(path, restored, identity, restored_optimizer)
            torch.testing.assert_close(restored.weight, model.weight)
            self.assertEqual(restored_optimizer.param_groups[0]["lr"], 0.04)
            self.assertEqual(len(restored_optimizer.state), len(optimizer.state))
