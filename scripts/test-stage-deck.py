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

    def test_quantities_must_be_positive_json_integers(self):
        for bad in [0, -1, 0.5, True, "1", None]:
            cards = deck(("leader", "Leader", 1, True), ("forest", "Forest", 98, False), ("elf", "Elf", bad, False))
            with self.subTest(bad), self.assertRaisesRegex(ValueError, "positive integer"):
                self.check(cards)

    def test_only_the_registered_r4_origin_is_accepted(self):
        with tempfile.TemporaryDirectory() as temp:
            fixtures, archive = Path(temp), Path(temp) / "archive.jsonl"
            header = b'{"recordType":"manifest","n":1}\n'
            archive.write_bytes(header + b'{"oracleIdentity":"elf"}\n')
            sha = module["sha"]

            def register(origins):
                (fixtures / "accepted-archives.json").write_text(json.dumps(origins))
                for origin in origins:
                    (fixtures / origin["manifest_file"]).write_bytes(header)

            exact = {"file_sha256": sha(archive.read_bytes()), "manifest_sha256": sha(header.rstrip())}
            other = {"file_sha256": "0" * 64, "manifest_sha256": "1" * 64}
            # A valid, registered older origin (e.g. schema-3 current) is not R4.
            register([dict(exact, manifest_file="archive-manifest-current.json"),
                      dict(other, manifest_file=module["R4_MANIFEST"])])
            with self.assertRaisesRegex(ValueError, "not the registered Archive R4 file"):
                module["accepted_origin"](archive, fixtures)
            register([dict(other, manifest_file="archive-manifest-current.json"),
                      dict(exact, manifest_file=module["R4_MANIFEST"])])
            origin, got_header, _ = module["accepted_origin"](archive, fixtures)
            self.assertEqual((origin["manifest_file"], got_header), (module["R4_MANIFEST"], header))
            (fixtures / module["R4_MANIFEST"]).write_bytes(b'{"recordType":"manifest","n":2}\n')
            with self.assertRaisesRegex(ValueError, "not the registered Archive R4 file"):
                module["accepted_origin"](archive, fixtures)
            with self.assertRaisesRegex(ValueError, "lacks rows"):
                module["archive_rows"](archive.read_bytes(), {"elf", "leader"})

    def test_output_is_new_and_published_without_replacement(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            dangling = root / "dangling"
            dangling.symlink_to(root / "missing")
            existing = root / "existing"
            existing.mkdir()
            for path in [dangling, existing]:
                with self.subTest(path.name), self.assertRaisesRegex(ValueError, "already exists"):
                    module["new_output"](path)
            linked = root / "linked"
            linked.symlink_to(root)
            self.assertEqual(module["new_output"](linked / "out"), root.resolve() / "out")
            staged = root / "staged"
            staged.mkdir()
            empty = root / "appeared"
            empty.mkdir()
            with self.assertRaises(FileExistsError):
                module["publish"](staged, empty)
            self.assertTrue(staged.is_dir())
            module["publish"](staged, root / "published")
            self.assertTrue((root / "published").is_dir() and not staged.exists())


if __name__ == "__main__":
    unittest.main()
