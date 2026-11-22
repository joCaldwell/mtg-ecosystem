"""Exercise stage-deck.py selection, Commander checks and archive binding without a compiler."""
import json
from pathlib import Path
import runpy
import tempfile
import unittest

module = runpy.run_path(str(Path(__file__).with_name("stage-deck.py")))
POLICY = {"named_bans": [{"oracle_id": "banned"}], "offensive_policy": {"records": []}}


def source(name, type_line, identity=("G",), legal="legal"):
    return {"source": {"name": name, "type_line": type_line, "color_identity": list(identity),
                       "legalities": {"commander": legal}}}


def deck(*cards):
    return {"name": "Test Deck", "cards": [
        {"oracle_id": key, "name": name, "quantity": quantity, "is_commander": commander}
        for key, name, quantity, commander in cards]}


class StageDeckTests(unittest.TestCase):
    def setUp(self):
        self.rows = {
            "leader": (b"", source("Leader", "Legendary Creature — Elf"), 2),
            "forest": (b"", source("Forest", "Basic Land — Forest", ()), 3),
            "elf": (b"", source("Elf", "Creature — Elf"), 4),
            "blue": (b"", source("Blue", "Instant", ("U",)), 5),
            "banned": (b"", source("Banned", "Sorcery"), 6),
        }

    def check(self, cards):
        return module["validate_deck"](cards, self.rows, POLICY)

    def test_slug_and_named_selection(self):
        self.assertEqual(module["slug"]("Jo's Cascade"), "jo-s-cascade")
        with tempfile.TemporaryDirectory() as temp:
            decks = Path(temp) / "decks.json"
            decks.write_text(json.dumps({"decks": [{"id": 11, "name": "Jo's Cascade"}, {"id": 3, "name": "Other"}]}))
            for name in ["Jo's Cascade", "jo-s-cascade", "11"]:
                args = type("Args", (), {"decklist": None, "decks": decks, "deck": name})()
                self.assertEqual(module["select_deck"](args)["id"], 11)
            args = type("Args", (), {"decklist": None, "decks": decks, "deck": "missing"})()
            with self.assertRaisesRegex(ValueError, "expected one deck"):
                module["select_deck"](args)

    def test_commander_deck_rules(self):
        legal = deck(("leader", "Leader", 1, True), ("forest", "Forest", 98, False), ("elf", "Elf", 1, False))
        self.assertEqual(self.check(legal), "leader")
        cases = {
            "exactly 100": deck(("leader", "Leader", 1, True), ("forest", "Forest", 98, False)),
            "ineligible deck entry Elf": deck(("leader", "Leader", 1, True), ("forest", "Forest", 97, False),
                                              ("elf", "Elf", 2, False)),
            "ineligible deck entry Blue": deck(("leader", "Leader", 1, True), ("forest", "Forest", 98, False),
                                               ("blue", "Blue", 1, False)),
            "ineligible deck entry Banned": deck(("leader", "Leader", 1, True), ("forest", "Forest", 98, False),
                                                 ("banned", "Banned", 1, False)),
            "ineligible commander Elf": deck(("elf", "Elf", 1, True), ("forest", "Forest", 99, False)),
            "differs from archived": deck(("leader", "Leader", 1, True), ("forest", "Forest", 98, False),
                                          ("elf", "Wrong", 1, False)),
        }
        for message, cards in cases.items():
            with self.subTest(message), self.assertRaisesRegex(ValueError, message):
                self.check(cards)

    def test_unregistered_archive_and_missing_rows_reject(self):
        with tempfile.TemporaryDirectory() as temp:
            archive = Path(temp) / "archive.jsonl"
            archive.write_bytes(b'{"recordType":"manifest"}\n{"oracleIdentity":"elf"}\n')
            with self.assertRaisesRegex(ValueError, "registered accepted archive"):
                module["accepted_origin"](archive)
            with self.assertRaisesRegex(ValueError, "lacks rows"):
                module["archive_rows"](archive.read_bytes(), {"elf", "leader"})


if __name__ == "__main__":
    unittest.main()
