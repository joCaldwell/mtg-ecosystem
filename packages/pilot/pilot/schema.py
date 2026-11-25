"""Adapt the v1 NumPy wire described in docs/game-engine-development.md.

Column meanings here come from crates/observe/src/lib.rs and catalog.rs.
The native build currently exports no observation version or canonical catalog
digest. Fence the local adapter version to the exact extension bytes, and hash
the ordered card table separately from the native vocabulary digest. The card
table hash covers the model's inputs, including any truncation in the native
encoding.
"""

from dataclasses import dataclass
from hashlib import file_digest, sha256
from importlib import import_module
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from typing import NamedTuple

import numpy as np
import torch


OBSERVATION_VERSION = 1
OBSERVATION_FIELDS = ("globals", "players", "ent_int", "ent_num", "ent_mask", "cand")
ACTION_FIELDS = ("ptr_mask", "num_mask", "head")
RESULT_FIELDS = ("seat", "reward", "done", "info")
CARD_COLUMN = 0
CANDIDATE_ENTITY = 1
CANDIDATE_SEAT = 2
CANDIDATE_FEATURES = (0, 3, 4, 5)
CENSOR_COLUMN = 2
POINTER_HEAD = 0
NUMBER_HEAD = 1


@dataclass(frozen=True)
class Identity:
    observation_version: int
    native_library_sha256: str
    card_table_sha256: str
    vocabulary_sha256: str


@dataclass(frozen=True)
class ModelShape:
    global_features: int
    seats: int
    player_features: int
    entity_features: int
    candidate_features: int
    pointer_slots: int
    number_slots: int
    special_slots: int
    vocabulary_size: int
    card_token_slots: int
    card_features: int


class Observation(NamedTuple):
    globals: np.ndarray
    players: np.ndarray
    entity_int: np.ndarray
    entity_num: np.ndarray
    candidates: np.ndarray
    mask: np.ndarray
    numeric_head: bool


class ModelInput(NamedTuple):
    globals: torch.Tensor
    players: torch.Tensor
    entity_cards: torch.Tensor
    entity_features: torch.Tensor
    entity_valid: torch.Tensor
    candidate_features: torch.Tensor
    candidate_entities: torch.Tensor
    candidate_seats: torch.Tensor
    mask: torch.Tensor
    numeric_head: torch.Tensor


@dataclass(frozen=True)
class Results:
    seats: np.ndarray
    rewards: np.ndarray
    done: np.ndarray
    censors: np.ndarray


def load_native(library: Path | None):
    if library is None:
        return import_module("mtg_env")
    spec = spec_from_file_location("mtg_env", library.resolve())
    if spec is None or spec.loader is None:
        raise ImportError(f"Cannot load mtg_env from {library}")
    module = module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def library_digest(native) -> str:
    with Path(native.__file__).open("rb") as extension:
        return file_digest(extension, "sha256").hexdigest()


def read_results(batch) -> Results:
    return Results(
        batch["seat"].copy(), batch["reward"].copy(),
        batch["done"].astype(bool), batch["info"][:, CENSOR_COLUMN].copy(),
    )


def action_masks(batch) -> tuple[np.ndarray, np.ndarray]:
    head = batch["head"]
    if np.any((head != POINTER_HEAD) & (head != NUMBER_HEAD)):
        raise ValueError("Unknown mtg-env action head")
    numeric = head == NUMBER_HEAD
    mask = batch["ptr_mask"].astype(bool)
    num_mask = batch["num_mask"]
    mask[numeric] = False
    mask[numeric, :num_mask.shape[1]] = num_mask[numeric]
    if not mask.any(axis=1).all():
        raise ValueError("Empty published mask for an active head")
    return mask, numeric


def uniform_actions(mask: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    ranks = rng.integers(mask.sum(axis=1))
    return (mask.cumsum(axis=1) > ranks[:, None]).argmax(axis=1).astype(np.int32)


def signed_log(tensor: torch.Tensor) -> torch.Tensor:
    value = tensor.float()
    return value.sign() * value.abs().log1p()


class Schema:
    """Inspect a reset VecEnv and pack owned observations for rollout and replay."""

    def __init__(self, env, native_sha256: str):
        with env.buffers() as batch:
            self.wire = {
                key: (batch[key].shape[1:], batch[key].dtype)
                for key in (*OBSERVATION_FIELDS, *ACTION_FIELDS, *RESULT_FIELDS)
            }
        # Refuse structural wire changes until their meanings are adapted here.
        expected = {
            "globals": (48,), "players": (4, 24), "ent_int": (256, 10),
            "ent_num": (256, 24), "ent_mask": (256,), "cand": (128, 6),
            "ptr_mask": (131,), "num_mask": (64,), "head": (), "seat": (),
            "reward": (4,), "done": (), "info": (8,),
        }
        byte_fields = {"ent_mask", "ptr_mask", "num_mask", "head", "seat", "done"}
        for key, shape in expected.items():
            dtype = np.dtype(np.uint8 if key in byte_fields else np.int32)
            if self.wire[key] != (shape, dtype):
                raise ValueError(f"Unsupported mtg-env v1 buffer {key}: {self.wire[key]}")
        catalog = env.catalog()
        self.card_tokens, self.card_lengths, self.card_numeric = (
            np.array(catalog[name], dtype="<i4", order="C", copy=True)
            for name in ("card_tokens", "card_len", "card_num")
        )
        digest = sha256()
        for array in (self.card_tokens, self.card_lengths, self.card_numeric):
            digest.update(np.asarray(array.shape, dtype="<i8").tobytes())
            digest.update(array.tobytes())
        self.identity = Identity(
            OBSERVATION_VERSION, native_sha256, digest.hexdigest(),
            catalog["vocabulary_sha256"],
        )

        def dimensions(field):
            return self.wire[field][0]

        candidates, _ = dimensions("cand")
        pointer_slots, = dimensions("ptr_mask")
        seats, player_features = dimensions("players")
        self.model_shape = ModelShape(
            global_features=dimensions("globals")[0], seats=seats,
            player_features=player_features,
            entity_features=dimensions("ent_int")[1] - 1 + dimensions("ent_num")[1],
            candidate_features=len(CANDIDATE_FEATURES),
            pointer_slots=pointer_slots, number_slots=dimensions("num_mask")[0],
            special_slots=pointer_slots - candidates,
            vocabulary_size=len(catalog["vocabulary"]) + 1,
            card_token_slots=self.card_tokens.shape[1], card_features=self.card_numeric.shape[1],
        )

    def capture(self, batch, rows, mask, numeric) -> list[Observation]:
        # Entities are written as a contiguous prefix. Candidate rows outside the
        # current policy mask never participate in scoring, so trim the suffix.
        globals_, players, ent_int, ent_num, ent_mask, cand = (
            batch[key] for key in OBSERVATION_FIELDS
        )
        entity_counts = ent_mask[rows].sum(axis=1)
        candidate_count = cand.shape[1]
        observations = []
        for i, entities in zip(rows, entity_counts, strict=True):
            active = np.flatnonzero(mask[i, :candidate_count])
            candidates = 0 if numeric[i] or not active.size else active[-1] + 1
            observations.append(Observation(
                globals_[i].copy(), players[i].copy(), ent_int[i, :entities].copy(),
                ent_num[i, :entities].copy(), cand[i, :candidates].copy(),
                mask[i].copy(), bool(numeric[i]),
            ))
        return observations

    def collate(self, observations: list[Observation], device: torch.device) -> ModelInput:
        size = len(observations)
        entities = max(len(o.entity_int) for o in observations)
        candidates = max(len(o.candidates) for o in observations)
        ent_int = np.zeros((size, entities, self.wire["ent_int"][0][1]), np.int32)
        ent_num = np.zeros((size, entities, self.wire["ent_num"][0][1]), np.int32)
        entity_valid = np.zeros((size, entities), bool)
        cand = np.full((size, candidates, self.wire["cand"][0][1]), -1, np.int32)
        for i, observation in enumerate(observations):
            count = len(observation.entity_int)
            ent_int[i, :count] = observation.entity_int
            ent_num[i, :count] = observation.entity_num
            entity_valid[i, :count] = True
            cand[i, :len(observation.candidates)] = observation.candidates

        def tensor(array):
            return torch.from_numpy(array).to(device)

        entity_int = tensor(ent_int)
        candidate = tensor(cand)
        entity_features = torch.cat((
            entity_int[..., :CARD_COLUMN], entity_int[..., CARD_COLUMN + 1:], tensor(ent_num),
        ), dim=-1)
        return ModelInput(
            signed_log(tensor(np.stack([o.globals for o in observations]))),
            signed_log(tensor(np.stack([o.players for o in observations]))),
            entity_int[..., CARD_COLUMN].clamp_min(0).long(),
            signed_log(entity_features),
            tensor(entity_valid),
            signed_log(candidate[..., CANDIDATE_FEATURES]),
            candidate[..., CANDIDATE_ENTITY].long(),
            candidate[..., CANDIDATE_SEAT].long(),
            tensor(np.stack([o.mask for o in observations])),
            tensor(np.asarray([o.numeric_head for o in observations], dtype=bool)),
        )
