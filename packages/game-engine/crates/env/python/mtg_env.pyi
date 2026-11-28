"""Typed interface of the ``mtg_env`` extension built from the ``mtg-env`` crate.

Batched four-seat Commander simulation with NumPy observation tensors. The Rust
source is authoritative: ``../src/ffi.rs`` (Python surface), ``../src/lib.rs``
(batching and action validation), ``../src/game.rs`` (episodes, rewards,
``info``) and ``crates/observe`` (column layouts in ``encode``, action
composition in ``action.rs``, funding codes in ``funding.rs``). Build and
support policy: ``docs/game-engine-development.md#batched-numpy-environment``.

Type checkers find this stub for scripts in this directory and for an
``mtg_env.so`` copied beside it; elsewhere, add this directory to the stub
path (``MYPYPATH``, pyright ``stubPath``). It is not engine build input.

``N`` is ``num_envs``. Every exported array is C-contiguous and read-only, and
numeric features saturate at the signed 32-bit limits.
"""

from collections.abc import Sequence
from types import TracebackType
from typing import Literal, TypeAlias, TypedDict, TypeVar, final, overload

import numpy as np

_S = TypeVar("_S", bound=tuple[int, ...])
_I32: TypeAlias = np.ndarray[_S, np.dtype[np.int32]]
_U8: TypeAlias = np.ndarray[_S, np.dtype[np.uint8]]
_E: TypeAlias = Literal[256]  # entity rows, mtg_observe::MAX_ENTITIES

class _Config(TypedDict, total=False):
    """``VecEnv(config=...)`` keys. Any other key raises ``ValueError``."""

    payment: Literal["auto", "manual"]
    """``"auto"`` (default) answers every Pay decision, and mana-ability and
    mana-colour decisions while the stack is non-empty, with the built-in
    Develop policy; ``"manual"`` exposes them. The spend step always uses the
    exact solver and is never exposed."""

    opponents: Literal["self_play", "baseline"]
    """``"self_play"`` (default) exposes every seat's decisions;
    ``"baseline"`` exposes seat 0 only and answers seats 1-3 with Develop."""

    mask_unfundable: bool
    """Default ``True``. ``cand[..., 5]`` is a funding code computed from the
    viewer's unrestricted pool plus the untapped, usable lands they control
    with exactly one basic land type: 0 the fixed cost cannot be paid, 1 it
    can (or the action has no fixed mana cost), 2 unknown (for example every
    non-action candidate, a double-faced or face-down card, a payable cost
    with hybrid symbols, a visible spell-cost reducer, or an unpaid cost while
    restricted mana or other mana sources leave that supply incomplete). When
    true, code-0 pointers are masked unless that would empty the pointer head,
    which then keeps its original mask."""

    manifest_sha256: str
    """Lowercase hex SHA-256 of a bulk catalog's ``manifest.json``. Required
    for a bulk catalog directory; ignored for a profile."""

class _CardCatalog(TypedDict):
    """Card table of the loaded source. Row 0 is padding and the absent
    identity; row ``c >= 1`` is catalog definition ``c - 1`` in Oracle-ID
    order, the card identity used by ``ent_int[..., 0]``. ``C + 1`` rows."""

    card_tokens: _I32[tuple[int, Literal[192]]]
    """Token IDs of each definition's Card IR JSON walk (object keys sorted;
    ``name``, ``oracle_id``, ``source_labels`` and ``provenance`` omitted),
    zero-padded. Token ``t >= 1`` is ``vocabulary[t - 1]``; a program longer
    than 192 tokens keeps 191 and ends with ``"<truncated>"``."""

    card_num: _I32[tuple[int, Literal[16]]]
    """Printed mana value, colour-identity bits (W U B R G C = bits 0-5),
    card-type bits (``CardKind`` discriminants), power, toughness (0 when
    absent); columns 5-15 are zero."""

    card_len: _I32[tuple[int]]
    """Token count of each row, at most 192."""

    vocabulary: list[str]
    """Sorted token vocabulary, including ``"<truncated>"``."""

    vocabulary_sha256: str
    """Lowercase hex SHA-256 of ``vocabulary`` serialized as compact JSON.
    Bind model inputs to both the catalog and this vocabulary identity."""

@final
class VecEnv:
    """``num_envs`` independent four-seat games advanced in lockstep.

    Determinism: episode ``k`` of game ``i`` is seeded from
    ``"mtg-env-v1/{seed}/{i}/{k}"``, which also picks the starting seat, so a
    game depends only on the source, its pod, ``config``, ``seed``, its index
    and its own actions. ``threads`` never changes any tensor or digest.

    Threading: each instance owns a persistent pool of ``min(threads,
    num_envs)`` workers named ``mtg-env-{w}``; worker ``w`` writes the rows
    whose batch index modulo the worker count is ``w``. ``reset`` and ``step``
    release the GIL, so separate instances can advance concurrently; another
    call on the same instance while one runs raises ``RuntimeError`` (PyO3
    borrow check). Every rules and query call uses the pod driver's
    1024-unit work quantum (``mtg_sim::pods::QUANTUM``).

    Faults: a game error or worker panic during ``reset`` or ``step`` raises
    ``RuntimeError`` and leaves the batch unusable; ``step``, ``buffers`` and
    ``state_digests`` then raise ``RuntimeError("reset required")`` until a
    successful ``reset``.
    """

    def __init__(
        self,
        catalog_dir_or_profile: str,
        pods: Sequence[str] | None = None,
        num_envs: int = 1,
        threads: int = 32,
        seed: int = 0,
        config: _Config | None = None,
    ) -> None:
        """Load one card source and allocate the batch; call ``reset`` next.

        ``catalog_dir_or_profile`` names an embedded match profile (for
        example ``"commander-foundations-v1"``); any other value is a bulk
        catalog directory. ``pods`` are file paths, and game ``i`` plays
        ``pods[i % len(pods)]``.

        - Profile: each pod file is a JSON array of four serialized
          decklists; with no pods every game uses the profile's default decks.
        - Bulk catalog: requires ``config["manifest_sha256"]``, an artifact
          built for this engine build identity, and at least one pod file in
          the simulator's ``{"decks": [...]}`` four-seat format.

        ``num_envs`` and ``threads`` must be positive; ``seed`` is an unsigned
        64-bit integer.

        Raises:
            ValueError: an unknown ``config`` key or value; an unreadable or
                invalid catalog, manifest or pod file; a missing or mismatched
                manifest pin or missing pods for a bulk catalog; a zero
                ``num_envs`` or ``threads``; or thread-pool creation failure.
            RuntimeError: another engine build identity is already installed
                in this process.
            TypeError: an argument or ``config`` value of the wrong Python
                type (``pods`` must be a sequence of ``str``, ``config`` a
                ``dict``).
            OverflowError: a negative ``num_envs``, ``threads`` or ``seed``.
        """

    def catalog(self) -> _CardCatalog:
        """Return a new dict over the card table; its arrays are shared and
        read-only. They are NumPy-owned, so keeping them never blocks ``step``."""

    def reset(self) -> None:
        """Restart every game at episode 0 and advance it to its first exposed
        decision.

        Raises:
            RuntimeError: a ``Batch`` array or a view of one is alive
                (``"batch is leased; ..."``), or a game failed to start or
                faulted.
        """

    def step(self, actions: _I32[tuple[int]]) -> None:
        """Apply one micro-action per game and advance each to its next
        exposed decision.

        ``actions`` is a length-``N`` int32 array (strided views are copied).
        ``actions[i]`` indexes ``ptr_mask[i]`` when ``head[i] == 0`` and
        ``num_mask[i]`` when ``head[i] == 1``, and that mask entry must be 1.
        A game whose episode ends returns the next episode's opening decision
        (see ``Batch``).

        Raises:
            RuntimeError: a ``Batch`` lease is alive; no successful ``reset``;
                a wrong length; a masked action (the message names the first
                such index, and that game's ``info[:, 0]`` count increments);
                or a game fault. Lease, length and mask errors leave every game
                at its previous decision.
            TypeError: ``actions`` is not a one-dimensional int32 array.
        """

    def state_digests(self) -> list[str]:
        """Return each game's rules-state digest (lowercase hex SHA-256) in
        batch order.

        Raises:
            RuntimeError: no successful ``reset`` since construction or the
                last fault.
        """

    def buffers(self) -> Batch:
        """Return zero-copy views of the current tensors.

        Raises:
            RuntimeError: no successful ``reset`` since construction or the
                last fault.
        """

@final
class Batch:
    """A lease on one ``VecEnv``'s tensors; only ``VecEnv.buffers`` creates it.

    Arrays borrow contiguous Rust storage that the next ``step`` or ``reset``
    overwrites in place at the same addresses. Every array and every NumPy view
    derived from one holds the lease, and ``step`` and ``reset`` raise while
    any is alive. Leaving the ``with`` block or ``release()`` drops only the
    batch's own references, so copy tensors to keep them.

    Actions: ``head`` selects the pointer (0) or number (1) head; the inactive
    head's mask is all zero. Pointers 0-127 address ``cand`` rows in candidate
    order, 128 ends a slot or ordering, and 129 and 130 finish mana production
    with undo or keep. Numbers 0-62 are literal and 63 selects the upper
    bound: the lesser of the published maximum and, for AnnounceX with a fully
    modeled supply, that supply; the signed 32-bit ceiling when neither
    exists; never below the published minimum. For a distribution, 63 is the
    bucket's largest feasible amount.

    Seats: ``players`` rows, entity seat columns, candidate seats and the
    active/priority globals rotate so the viewer is 0; ``seat`` and ``reward``
    use absolute seat order. Missing entity and seat references are -1; card 0
    is the absent identity.

    Entities (``ent_mask`` marks used rows): objects in published order, stack
    entries in stack order, remembered library positions, one hand-zone row
    per publicly known revealed hand card, then revealed library cards; the
    last three kinds have no targetable handle. No column carries a stack
    entry's announced modes; ``ent_num[..., 21]`` sums every counter kind but
    -1/-1 (``ent_num[..., 20]``), lore included.

    Episodes: when ``done[i] == 1``, row ``i`` already holds the next
    episode's opening decision with the finished episode's ``reward`` and
    ``info``. ``reward`` is +1 for the winner and -1 for the other seats, and
    zero for a draw, a censored episode and every non-terminal row. ``info``
    columns count, per episode: 0 rejected masked actions, 1 cast reversals,
    2 censor class (0 natural outcome or live episode, 1 work, turn or
    decision limit, 2 typed engine capacity fault, 3 tensor capacity: the
    frame did not fit this layout), 3 completed rules decisions; 4-7 are zero.
    """

    @overload
    def __getitem__(self, key: Literal["globals"]) -> _I32[tuple[int, Literal[48]]]: ...
    @overload
    def __getitem__(self, key: Literal["players"]) -> _I32[tuple[int, Literal[4], Literal[24]]]: ...
    @overload
    def __getitem__(self, key: Literal["ent_int"]) -> _I32[tuple[int, _E, Literal[10]]]: ...
    @overload
    def __getitem__(self, key: Literal["ent_num"]) -> _I32[tuple[int, _E, Literal[24]]]: ...
    @overload
    def __getitem__(self, key: Literal["ent_mask"]) -> _U8[tuple[int, _E]]: ...
    @overload
    def __getitem__(self, key: Literal["cand"]) -> _I32[tuple[int, Literal[128], Literal[6]]]: ...
    @overload
    def __getitem__(self, key: Literal["ptr_mask"]) -> _U8[tuple[int, Literal[131]]]: ...
    @overload
    def __getitem__(self, key: Literal["num_mask"]) -> _U8[tuple[int, Literal[64]]]: ...
    @overload
    def __getitem__(self, key: Literal["head", "seat", "done"]) -> _U8[tuple[int]]: ...
    @overload
    def __getitem__(self, key: Literal["reward"]) -> _I32[tuple[int, Literal[4]]]: ...
    @overload
    def __getitem__(self, key: Literal["info"]) -> _I32[tuple[int, Literal[8]]]: ...
    # Another key raises KeyError; any key after release raises RuntimeError.
    def __enter__(self) -> Batch: ...
    def __exit__(
        self,
        ty: type[BaseException] | None,
        value: BaseException | None,
        trace: TracebackType | None,
    ) -> None:
        """Call ``release()``; exceptions propagate."""

    def release(self) -> None:
        """Drop this batch's array references. Derived views keep the lease."""
