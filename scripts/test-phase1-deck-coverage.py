"""Exercise the deck report CLI with tiny compiler-format receipts."""
from collections import Counter
from contextlib import redirect_stderr, redirect_stdout
import hashlib
from io import StringIO
import json
from pathlib import Path
import runpy
import tempfile
import unittest


module = runpy.run_path(str(Path(__file__).with_name("phase1-deck-coverage.py")))
coverage_main = module["main"]
TRIGGER = "abilities[*].trigger.equipped"
EFFECT = "effect:draw"
CASE_ROWS = [
    ("basic", "A\\B\t| C", "parsed", True, "lowered_validated", ""),
    ("trigger", "Trigger", "parsed", False, "unsupported", TRIGGER),
    ("effect", "Effect", "parsed", True, "invalid", EFFECT),
    ("unparsed", "Unparsed", "unsupported", False, "unparsed", "status:unparsed"),
    ("excluded", "Excluded", "excluded", None, "unparsed", "archive_excluded:scope"),
    ("other", "Not In A Deck", "parsed", True, "unsupported", "effect:other"),
]
TARGET_IDS = {"basic", "trigger", "effect", "unparsed", "excluded", "missing"}


class DeckCoverageTests(unittest.TestCase):
    def setUp(self):
        workspace = tempfile.TemporaryDirectory()
        self.addCleanup(workspace.cleanup)
        self.root = Path(workspace.name)
        self.archive = self.root / "archive.jsonl"
        self.coverage = self.root / "whole-archive-coverage.json"
        self.records = self.root / "whole-archive" / "records.tsv"
        self.records.parent.mkdir()
        self.decks = self.root / "decks.json"
        self.output = self.root / "output"
        self.args = ["--decks", str(self.decks), "--archive", str(self.archive), "--coverage", str(self.coverage)]
        self.write_receipts()
        decks = [
            self.deck(1, "paper", basic=2, trigger=3, unparsed=1),
            self.deck(2, "paper", basic=1, effect=1),
            self.deck(3, "theorycrafted", basic=1, effect=1),
            self.deck(4, "paper", missing=1, trigger=1),
            self.deck(5, "theorycrafted", basic=2),
            self.deck(6, "theorycrafted", excluded=1, trigger=1),
        ]
        self.decks.write_text(json.dumps({
            "capturedAt": "fixture", "deckCount": len(decks), "uniqueCardCount": 6, "decks": decks,
        }))

    def deck(self, deck_id, category, **members):
        return {
            "id": deck_id, "name": f"Deck {deck_id}", "category": category, "revision": 1,
            "total_cards": sum(members.values()), "unique_cards": len(members),
            "commanders": ["Former Basic Name"] if "basic" in members else [],
            "cards": [
                {
                    "oracle_id": oracle_id, "name": "Former Basic Name" if oracle_id == "basic" else oracle_id.title(),
                    "quantity": quantity, "is_commander": oracle_id == "basic",
                }
                for oracle_id, quantity in members.items()
            ],
        }

    def write_receipts(self, selected=None):
        header = json.dumps({"parser": {"revision": "fixture"}}).encode()
        archive_rows = [header]
        records = ["oracle_id\tname\tstatus\treason"]
        statuses = dict.fromkeys(("lowered_validated", "unsupported", "invalid", "unparsed"), 0)
        reasons = Counter()
        rows = CASE_ROWS if selected is None else [row for row in CASE_ROWS if row[0] in selected]
        for oracle_id, name, archive_status, bound, status, reason in CASE_ROWS:
            archive_rows.append(json.dumps({
                "recordType": "card", "oracleIdentity": oracle_id, "status": archive_status,
                "binding": None if bound is None else {"fullyBound": bound},
                "source": {"name": name, "legalities": {"commander": "legal"}},
            }).encode())
            if (oracle_id, name, archive_status, bound, status, reason) not in rows:
                continue
            tsv_name = name.replace("\\", r"\\").replace("\t", r"\t")
            records.append("\t".join((oracle_id, tsv_name, status, reason)))
            statuses[status] += 1
            if reason:
                reasons[status, reason] += 1

        archive = b"\n".join(archive_rows) + b"\n"
        self.archive.write_bytes(archive)
        self.records.write_text("\n".join(records) + "\n")
        self.coverage.write_text(json.dumps({
            "report_version": 1, "ir_version": 1, "parser_revision": "fixture",
            "archive_sha256": hashlib.sha256(archive).hexdigest(),
            "archive_manifest_sha256": hashlib.sha256(header).hexdigest(),
            "total": len(rows), "commander_legal": len(rows), "lowered_validated": 1,
            "statuses": statuses, "reasons": [
                {"status": status, "reason": reason, "count": count}
                for (status, reason), count in reasons.items()
            ],
            **({} if selected is None else {"selection": {
                "oracle_ids_sha256": module["selection_sha256"](selected), "requested": len(selected),
                "matched": len(rows), "missing": sorted(selected - {row[0] for row in rows}),
            }}),
        }))

    def test_report_weights_copies_and_distinguishes_slot_and_deck_unlocks(self):
        coverage_main(self.args, self.output)
        json_path = self.output / "deck-gap-report.json"
        markdown_path = self.output / "deck-gap-report.md"
        first = json_path.read_bytes(), markdown_path.read_bytes()
        report = json.loads(first[0])
        self.assertEqual(report["all"]["slotCounts"],
                         {"slots": 16, "compiled": 6, "parsed": 13, "fullyBound": 8, "blocked": 10})
        self.assertEqual(report["paper"]["slotCounts"],
                         {"slots": 10, "compiled": 3, "parsed": 8, "fullyBound": 4, "blocked": 7})
        self.assertEqual(report["all"]["uniqueCounts"],
                         {"cards": 6, "compiled": 1, "parsed": 3, "fullyBound": 2, "blocked": 5})
        self.assertEqual(report["all"]["compilerCompleteDecks"], 1)
        self.assertEqual(report["paper"]["compilerCompleteDecks"], 0)
        by_slot = report["reasonRankings"]["byDeckSlotsUnlocked"]
        self.assertEqual([(r["reasonCode"], r["deckSlotsUnlocked"], r["decksUnlocked"]) for r in by_slot],
                         [(TRIGGER, 5, 0), (EFFECT, 2, 2)])
        by_deck = report["reasonRankings"]["byDecksUnlocked"][0]
        self.assertEqual((by_deck["reasonCode"], by_deck["paperDecksUnlocked"]), (EFFECT, 1))
        deck_one = report["decks"][0]
        self.assertEqual(deck_one["cards"][0]["name"], "A\\B\t| C")
        self.assertEqual(deck_one["cards"][0]["snapshotName"], "Former Basic Name")
        self.assertEqual({g["mechanicFamily"] for g in deck_one["blockerGroups"]}, {"parser coverage", "triggers"})
        self.assertEqual(report["decks"][3]["blockerGroups"][0]["reasonCode"], "archive:missing_oracle_id")
        self.assertIn(b"archive scope", first[1])
        coverage_main(self.args, self.output)
        self.assertEqual((json_path.read_bytes(), markdown_path.read_bytes()), first)

    def test_selected_receipts_match_whole_archive_report(self):
        coverage_main(self.args, self.output)
        full = json.loads((self.output / "deck-gap-report.json").read_bytes())
        self.write_receipts(TARGET_IDS)
        selected_output = self.root / "selected"
        coverage_main(self.args, selected_output)
        selected = json.loads((selected_output / "deck-gap-report.json").read_bytes())
        self.assertEqual(selected["inputIdentity"]["coverage"]["selection"]["missing"], ["missing"])
        for report in (full, selected):
            del report["inputIdentity"]["coverage"]
        self.assertEqual(selected, full)

    def test_selected_receipts_must_cover_exactly_the_snapshot(self):
        self.write_receipts(TARGET_IDS | {"other"})
        self.assert_cli_failure("does not cover exactly the deck snapshot")

    def test_selected_receipts_reject_extra_rows(self):
        self.write_receipts(TARGET_IDS)
        with self.records.open("a") as records:
            records.write("other\tNot In A Deck\tunsupported\teffect:other\n")
        self.assert_cli_failure("archive or coverage row missing")

    def test_compare_prints_card_and_blocker_deltas(self):
        coverage_main(self.args, self.output)
        before = self.root / "before.json"
        (self.output / "deck-gap-report.json").replace(before)
        rows = list(CASE_ROWS)
        CASE_ROWS[1] = ("trigger", "Trigger", "parsed", False, "lowered_validated", "")
        CASE_ROWS[0] = ("basic", "A\\B\t| C", "parsed", True, "invalid", EFFECT)
        self.addCleanup(lambda: CASE_ROWS.__setitem__(slice(None), rows))
        self.write_receipts()
        stdout = StringIO()
        with redirect_stdout(stdout):
            coverage_main(self.args + ["--compare", str(before), "--top", "1"], self.output)
        delta = stdout.getvalue()
        self.assertIn("## Newly compiled cards (1)\n\n- Trigger (`trigger`): was abilities\\[\\*\\].trigger.equipped", delta)
        self.assertIn("## Regressed cards (1)", delta)
        self.assertIn("| Deck 1 (#1) | 2 → 3 | +1 | 3 |", delta)
        self.assertIn("| effect:draw | 8 | +6 | 3 | +1 | 4 |", delta)
        self.assertIn("Cleared reason codes: `abilities[*].trigger.equipped`.", delta)

    def assert_cli_failure(self, message):
        stderr = StringIO()
        with redirect_stderr(stderr), self.assertRaises(SystemExit) as error:
            coverage_main(self.args, self.output)
        self.assertEqual(error.exception.code, 1)
        self.assertIn(message, stderr.getvalue())
        self.assertFalse(self.output.exists())

    def test_rejects_changed_archive_bytes(self):
        with self.archive.open("rb+") as archive:
            archive.seek(-1, 2)
            archive.write(b" \n")
        self.assert_cli_failure("coverage archive SHA-256 mismatch")

    def test_rejects_receipt_identity_mismatch(self):
        self.records.write_text(self.records.read_text().replace("trigger\tTrigger", "effect\tTrigger"))
        self.assert_cli_failure("coverage identity mismatch")

    def test_rejects_receipt_reason_mismatch(self):
        self.records.write_text(self.records.read_text().replace(TRIGGER, "trigger:different-reason"))
        self.assert_cli_failure("coverage reason counts differ from records.tsv")


if __name__ == "__main__":
    unittest.main()
