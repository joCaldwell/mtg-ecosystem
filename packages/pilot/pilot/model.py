"""Catalog-aware entity transformer and masked pointer/number policy."""

from contextlib import nullcontext
import math

import torch
from torch import nn
from torch.nn import functional as F

from .schema import ModelInput, Schema, signed_log


def precision(device: torch.device):
    if device.type == "cuda":
        return torch.autocast("cuda", dtype=torch.float16)
    return nullcontext()


def masked_policy(logits: torch.Tensor, mask: torch.Tensor) -> torch.Tensor:
    """Log-probabilities shared by sampling, evaluation and PPO."""
    return logits.float().masked_fill(~mask, -torch.inf).log_softmax(dim=-1)


class Pilot(nn.Module):
    def __init__(self, schema: Schema, width: int = 128, layers: int = 5):
        super().__init__()
        shape = schema.model_shape
        self.shape = shape
        self.width = width
        self.register_buffer("card_tokens", torch.from_numpy(schema.card_tokens), persistent=False)
        self.register_buffer("card_lengths", torch.from_numpy(schema.card_lengths), persistent=False)
        self.register_buffer("card_numeric", torch.from_numpy(schema.card_numeric), persistent=False)
        self.token_embed = nn.Embedding(shape.vocabulary_size, 64, padding_idx=0)
        self.token_mix = nn.Conv1d(64, 64, 3, padding=1)
        self.card_project = nn.Linear(64 + shape.card_features, width)
        self.global_project = nn.Linear(shape.global_features, width)
        self.player_project = nn.Linear(shape.player_features, width)
        self.seat_embed = nn.Parameter(torch.randn(shape.seats, width) * 0.02)
        self.entity_project = nn.Linear(shape.entity_features, width)
        self.transformer = nn.TransformerEncoder(
            nn.TransformerEncoderLayer(
                width, 4, 4 * width, dropout=0, activation="gelu",
                batch_first=True, norm_first=True,
            ),
            layers, norm=nn.LayerNorm(width), enable_nested_tensor=False,
        )
        self.candidate_project = nn.Linear(shape.candidate_features, width)
        self.pointer_query = nn.Linear(width, width)
        self.pointer_key = nn.Linear(width, width)
        self.special_pointer = nn.Linear(width, shape.special_slots)
        self.number = nn.Linear(width, shape.number_slots)
        self.winner = nn.Linear(width, shape.seats)

    def encode_catalog(self) -> torch.Tensor:
        """Encode each card once per frozen collection or optimizer step."""
        tokens = self.token_embed(self.card_tokens).transpose(1, 2)
        tokens = F.gelu(self.token_mix(tokens)).transpose(1, 2)
        valid = torch.arange(tokens.shape[1], device=tokens.device) < self.card_lengths[:, None]
        pooled = (tokens * valid[..., None]).sum(1) / self.card_lengths.clamp_min(1)[:, None]
        return self.card_project(torch.cat((pooled, signed_log(self.card_numeric)), dim=-1))

    @staticmethod
    def _linked(rows: torch.Tensor, index: torch.Tensor) -> torch.Tensor:
        # A sentinel row lets an observation have zero visible entities.
        padded = F.pad(rows, (0, 0, 1, 0))
        return padded.gather(1, (index + 1).unsqueeze(-1).expand(-1, -1, rows.size(-1)))

    def encode_observation(self, inputs: ModelInput, cards: torch.Tensor) -> torch.Tensor:
        globals_ = self.global_project(inputs.globals).unsqueeze(1)
        players = self.player_project(inputs.players)
        players = players + self.seat_embed.to(players.dtype)
        entities = self.entity_project(inputs.entity_features) + cards[inputs.entity_cards]
        visible = F.pad(inputs.entity_valid, (1 + self.shape.seats, 0), value=True)
        return self.transformer(
            torch.cat((globals_, players, entities), dim=1),
            src_key_padding_mask=~visible,
        )

    def value_logits(self, inputs: ModelInput, cards: torch.Tensor) -> torch.Tensor:
        return self.winner(self.encode_observation(inputs, cards)[:, 0]).float()

    def forward(self, inputs: ModelInput, cards: torch.Tensor):
        encoded = self.encode_observation(inputs, cards)
        context = encoded[:, 0]
        players = encoded[:, 1:1 + self.shape.seats]
        entities = encoded[:, 1 + self.shape.seats:]
        candidate = (
            self.candidate_project(inputs.candidate_features)
            + self._linked(entities, inputs.candidate_entities)
            + self._linked(players, inputs.candidate_seats)
        )
        query = self.pointer_query(context)
        scores = torch.bmm(self.pointer_key(candidate), query.unsqueeze(-1)).squeeze(-1)
        scores = scores / math.sqrt(self.width)
        padding = self.shape.pointer_slots - self.shape.special_slots - scores.shape[1]
        pointer = torch.cat((F.pad(scores, (0, padding)), self.special_pointer(context)), dim=1)
        number = F.pad(self.number(context), (0, self.shape.pointer_slots - self.shape.number_slots))
        logits = torch.where(inputs.numeric_head[:, None], number, pointer)
        return masked_policy(logits, inputs.mask), self.winner(context).float()
