"""Cycle embedded-profile deck slots as in mtg-sim bench-pods --rotate-seats.

Use --rotate-profile with the engine's profile JSON. Its decks are written as
four-element arrays per the VecEnv contract in docs/game-engine-development.md.
The original deck contents and clockwise ordering are retained across rotations.
"""

from collections import Counter
from dataclasses import dataclass
from hashlib import sha256
import json
from pathlib import Path


@dataclass(frozen=True)
class LearnerDeck:
    sha256: str
    commander: str


class RotatedPods:
    def __init__(self, path: Path, expected_profile: str):
        profile = json.loads(path.read_text())
        decks = profile["decks"]
        if profile["name"] != expected_profile or len(decks) != 4:
            raise ValueError(f"Expected four decklists for profile {expected_profile} in {path}")
        manifest = path.parent / profile["catalog_manifest"]
        names = {}
        for record in json.loads(manifest.read_text())["records"]:
            source = json.loads((manifest.parent / record).read_text())["source"]
            names[source["oracle_id"]] = source["name"]
        # Keep duplicate rotations. The simulator uses all four slot offsets
        # even when the source profile repeats an identical decklist.
        self.tables = tuple(
            tuple(decks[(seat - offset) % len(decks)] for seat in range(len(decks)))
            for offset in range(len(decks))
        )
        self.learner_decks = tuple(
            LearnerDeck(
                sha256(json.dumps(table[0], sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
                names[table[0]["commander"]],
            )
            for table in self.tables
        )

    def write(self, directory: Path) -> list[str]:
        paths = []
        for index, table in enumerate(self.tables):
            path = directory / f"pod-{index}.json"
            path.write_text(json.dumps(table))
            paths.append(str(path))
        return paths

    def lane_decks(self, count: int) -> list[LearnerDeck]:
        return [self.learner_decks[lane % len(self.learner_decks)] for lane in range(count)]

    def allocation(self, count: int) -> list[dict]:
        return [
            {"commander": deck.commander, "deck_sha256": deck.sha256, "lanes": lanes}
            for deck, lanes in Counter(self.lane_decks(count)).items()
        ]
