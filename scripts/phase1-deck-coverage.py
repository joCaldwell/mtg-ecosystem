#!/usr/bin/env python3
"""Join the Phase 1 deck snapshot to an archive and its compiler coverage receipts.

Pass the compiler example's stdout JSON or summary.json to --coverage. For the
stdout filename whole-archive-coverage.json, the example's records.tsv lives in
the sibling directory whole-archive. A coverage output directory also works.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
from itertools import zip_longest
import json
from pathlib import Path
import re
import tempfile


STATUSES = ("lowered_validated", "unsupported", "invalid", "unparsed")
REJECTIONS = {"unsupported", "invalid"}
FAMILIES = {
    "activation": "activated abilities",
    "amount": "amounts",
    "binding": "semantic binding",
    "bindings": "semantic binding",
    "card_faces": "layouts",
    "color_identity": "mana and color",
    "colors": "mana and color",
    "cls": "classes",
    "cost": "costs",
    "effect": "effects",
    "effects": "effects",
    "event": "events",
    "faces": "layouts",
    "filter": "object selection",
    "ir": "IR validation",
    "keyword": "keywords",
    "kind": "ability kinds",
    "layout": "layouts",
    "mana": "mana and color",
    "mana_cost": "mana and color",
    "manaCost": "mana and color",
    "modifier": "modifiers",
    "multi_face_layout": "layouts",
    "player": "player selection",
    "prevent": "prevention",
    "ref": "object references",
    "reference": "object references",
    "replacement": "replacements",
    "set": "object selection",
    "shield": "prevention",
    "static": "static abilities",
    "subtype": "subtypes",
    "target": "object selection",
    "targets": "object selection",
    "to": "destinations",
    "trigger": "triggers",
    "type_line": "card types",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_snapshot(path):
    data = path.read_bytes()
    snapshot = json.loads(data)
    decks = snapshot["decks"]
    require(snapshot["deckCount"] == len(decks), "snapshot deckCount mismatch")
    deck_ids = set()
    target_ids = set()
    for deck in decks:
        deck_id = deck["id"]
        require(type(deck_id) is int and deck_id not in deck_ids, f"invalid deck ID {deck_id!r}")
        deck_ids.add(deck_id)
        quantities = {}
        for card in deck["cards"]:
            oracle_id = card["oracle_id"]
            quantity = card["quantity"]
            require(isinstance(oracle_id, str) and oracle_id and oracle_id not in quantities,
                    f"invalid or repeated Oracle ID {oracle_id!r} in deck {deck_id}")
            require(type(quantity) is int and quantity > 0,
                    f"invalid quantity for {oracle_id} in deck {deck_id}")
            require(type(card["is_commander"]) is bool,
                    f"invalid commander flag for {oracle_id} in deck {deck_id}")
            quantities[oracle_id] = quantity
        require(len(quantities) == deck["unique_cards"], f"unique_cards mismatch in deck {deck_id}")
        require(sum(quantities.values()) == deck["total_cards"], f"total_cards mismatch in deck {deck_id}")
        target_ids.update(quantities)
    require(len(target_ids) == snapshot["uniqueCardCount"], "snapshot uniqueCardCount mismatch")
    return snapshot, target_ids, hashlib.sha256(data).hexdigest()


def receipt_paths(coverage):
    if coverage.is_dir():
        return coverage / "summary.json", coverage / "records.tsv"
    if coverage.name.endswith("-coverage.json"):
        return coverage, coverage.with_name(coverage.name.removesuffix("-coverage.json")) / "records.tsv"
    return coverage, coverage.with_name("records.tsv")


def escape_tsv(text):
    # The compiler writes literal backslash escapes, without quoting cells.
    return text.replace("\\", "\\\\").replace("\t", "\\t").replace("\r", "\\r").replace("\n", "\\n")


def read_outcomes(archive, coverage, target_ids):
    summary_path, records_path = receipt_paths(coverage)
    summary_bytes = summary_path.read_bytes()
    summary = json.loads(summary_bytes)
    require(summary["report_version"] == 1 and summary["ir_version"] == 1,
            "expected coverage report version 1 and Card IR version 1")
    archive_hash = hashlib.sha256()
    records_hash = hashlib.sha256()
    statuses = Counter(dict.fromkeys(STATUSES, 0))
    reasons = Counter()
    outcomes = {}
    seen = set()
    with archive.open("rb") as source, records_path.open("rb") as records:
        header = source.readline()
        archive_hash.update(header)
        require(hashlib.sha256(header.rstrip(b"\r\n")).hexdigest() == summary["archive_manifest_sha256"],
                "coverage archive manifest SHA-256 mismatch")
        manifest = json.loads(header)
        require(manifest["parser"]["revision"] == summary["parser_revision"], "coverage parser revision mismatch")
        tsv_header = records.readline()
        records_hash.update(tsv_header)
        require(tsv_header == b"oracle_id\tname\tstatus\treason\n", "invalid coverage records.tsv header")
        for line_number, (raw, tsv) in enumerate(zip_longest(source, records), start=2):
            require(raw is not None and tsv is not None, f"archive or coverage row missing at line {line_number}")
            archive_hash.update(raw)
            records_hash.update(tsv)
            row = json.loads(raw)
            oracle_id = row["oracleIdentity"]
            name = row["source"]["name"]
            require(row["recordType"] == "card" and oracle_id not in seen,
                    f"invalid or duplicate oracleIdentity at archive line {line_number}")
            seen.add(oracle_id)
            fields = tsv.decode("utf-8").removesuffix("\n").split("\t")
            require(len(fields) == 4 and fields[:2] == [escape_tsv(oracle_id), escape_tsv(name)],
                    f"coverage identity mismatch at line {line_number}")
            status, escaped_reason = fields[2:]
            require(status in STATUSES and bool(escaped_reason) == (status != "lowered_validated"),
                    f"invalid coverage outcome at line {line_number}")
            if row["source"]["legalities"]["commander"] == "legal":
                statuses[status] += 1
                if escaped_reason:
                    reasons[status, escaped_reason] += 1
            if oracle_id in target_ids:
                reason = re.sub(r"\\([\\trn])", lambda m: {"t": "\t", "r": "\r", "n": "\n", "\\": "\\"}[m[1]],
                                escaped_reason)
                require(escape_tsv(reason) == escaped_reason, f"invalid coverage escape at line {line_number}")
                outcomes[oracle_id] = {
                    "name": name,
                    "archiveStatus": row["status"],
                    "fullyBound": (row.get("binding") or {}).get("fullyBound") is True,
                    "coverageStatus": status,
                    "reasonCode": reason or None,
                }
    require(archive_hash.hexdigest() == summary["archive_sha256"], "coverage archive SHA-256 mismatch")
    require(len(seen) == summary["total"] and statuses == summary["statuses"]
            and sum(statuses.values()) == summary["commander_legal"]
            and statuses["lowered_validated"] == summary["lowered_validated"],
            "coverage summary counts differ from the archive and per-card receipt")
    reason_rows = summary["reasons"]
    expected = {(r["status"], escape_tsv(r["reason"])): r["count"] for r in reason_rows}
    require(len(expected) == len(reason_rows) and reasons == expected, "coverage reason counts differ from records.tsv")
    identity = {key: summary[key] for key in (
        "archive_sha256", "archive_manifest_sha256", "parser_revision", "report_version", "ir_version",
    )}
    identity["summary_sha256"] = hashlib.sha256(summary_bytes).hexdigest()
    identity["records_sha256"] = records_hash.hexdigest()
    return outcomes, identity


def mechanic_family(status, code):
    if status == "missing_archive":
        return "archive membership"
    if status == "unparsed":
        return "archive scope" if code.startswith("archive_excluded:") else "parser coverage"
    # Prefer an explicit mechanic on paths like abilities[*].trigger.equipped.
    path = code.split(":", 1)[0]
    segments = re.sub(r"\[[^\]]*\]", "", path).split(".")
    for segment in segments:
        if segment in FAMILIES:
            return FAMILIES[segment]
    return "ability structure" if segments[0] == "abilities" else "other compiler diagnostics"


def tally(cards, weight):
    counts = dict.fromkeys(("slots", "compiled", "parsed", "fullyBound", "blocked"), 0)
    for card in cards:
        quantity = weight(card)
        compiled = card["coverageStatus"] == "lowered_validated"
        counts["slots"] += quantity
        counts["compiled"] += quantity * compiled
        counts["parsed"] += quantity * (card["archiveStatus"] == "parsed")
        counts["fullyBound"] += quantity * card["fullyBound"]
        counts["blocked"] += quantity * (not compiled)
    return counts


def totals(cards):
    unique = {card["oracleId"]: card for card in cards}.values()
    unique_counts = tally(unique, lambda card: 1)
    return {
        "slotCounts": tally(cards, lambda card: card["quantity"]),
        "uniqueCounts": {"cards": unique_counts.pop("slots"), **unique_counts},
    }


def cohort(decks):
    return {
        "decks": len(decks),
        "compilerCompleteDecks": sum(deck["slotCounts"]["blocked"] == 0 for deck in decks),
        **totals([card for deck in decks for card in deck["cards"]]),
    }


def build_report(decks_path, archive, coverage):
    snapshot, target_ids, decks_hash = read_snapshot(decks_path)
    outcomes, identity = read_outcomes(archive, coverage, target_ids)
    report_decks = []
    rankings = {}
    for deck in sorted(snapshot["decks"], key=lambda deck: deck["id"]):
        cards = []
        groups = defaultdict(list)
        rankable_slots = Counter()
        for card in sorted(deck["cards"], key=lambda card: (card["name"].casefold(), card["oracle_id"])):
            oracle_id = card["oracle_id"]
            outcome = outcomes.get(oracle_id, {
                "name": card["name"], "archiveStatus": None, "fullyBound": False,
                "coverageStatus": "missing_archive", "reasonCode": "archive:missing_oracle_id",
            })
            member = {
                "oracleId": oracle_id,
                "quantity": card["quantity"],
                "isCommander": card["is_commander"],
                **outcome,
            }
            if card["name"] != member["name"]:
                member["snapshotName"] = card["name"]
            cards.append(member)
            status, code = member["coverageStatus"], member["reasonCode"]
            if code:
                groups[mechanic_family(status, code), code, status].append(member)
            if status in REJECTIONS:
                rankable_slots[code] += card["quantity"]

        paper = deck["category"] == "paper"
        stats = totals(cards)
        for code, quantity in rankable_slots.items():
            entry = rankings.setdefault(code, {
                "reasonCode": code, "deckSlotsUnlocked": 0, "decksUnlocked": 0, "decksAffected": 0,
                "paperDeckSlotsUnlocked": 0, "paperDecksUnlocked": 0,
            })
            completes = quantity == stats["slotCounts"]["blocked"]
            entry["deckSlotsUnlocked"] += quantity
            entry["decksUnlocked"] += completes
            entry["decksAffected"] += 1
            if paper:
                entry["paperDeckSlotsUnlocked"] += quantity
                entry["paperDecksUnlocked"] += completes

        report_decks.append({
            "id": deck["id"], "name": deck["name"], "category": deck["category"],
            "revision": deck["revision"], "commanders": deck["commanders"],
            **stats,
            "cards": cards,
            "blockerGroups": [
                {
                    "mechanicFamily": family, "reasonCode": code, "coverageStatus": status,
                    "slots": sum(card["quantity"] for card in members),
                    "oracleIds": [card["oracleId"] for card in members],
                }
                for (family, code, status), members in sorted(groups.items())
            ],
        })

    return {
        "reportVersion": 1,
        "inputIdentity": {
            "decksSha256": decks_hash, "snapshotCapturedAt": snapshot["capturedAt"], "coverage": identity,
        },
        "counting": {
            "compiled": "Compiler lowered and validated the card as a singleton catalog.",
            "parsed": "Archive card status is parsed.",
            "fullyBound": "Archive binding.fullyBound is true.",
            "blocked": "Card lacks a lowered_validated result, including missing archive cards.",
            "deckSlotsUnlocked": "All copies carrying a compiler rejection code, if repairing it fully cleared their recorded blockers.",
            "decksUnlocked": "Decks whose every blocked slot carries that compiler rejection code.",
            "mechanicFamily": "Diagnostic namespace or archive/parser stage. A generic diagnostic may hide the precise mechanic.",
        },
        "all": cohort(report_decks),
        "paper": cohort([deck for deck in report_decks if deck["category"] == "paper"]),
        "reasonRankings": {
            "byDeckSlotsUnlocked": sorted(rankings.values(), key=lambda r: (
                -r["deckSlotsUnlocked"], -r["decksUnlocked"], r["reasonCode"],
            )),
            "byDecksUnlocked": sorted(rankings.values(), key=lambda r: (
                -r["decksUnlocked"], -r["deckSlotsUnlocked"], r["reasonCode"],
            )),
        },
        "decks": report_decks,
    }


def markdown_text(value):
    return re.sub(r"([\\`*_{}\[\]<>|])", r"\\\1", str(value).replace("\r", " ").replace("\n", " "))


def count_row(label, counts):
    return "| " + " | ".join([label] + [str(counts[key]) for key in (
        "slots", "compiled", "parsed", "fullyBound", "blocked",
    )]) + " |"


def render_markdown(report):
    lines = [
        "# Phase 1A deck compiler coverage", "",
        "Counts are card slots, including commander copies. Parsed, fully bound, and compiled describe",
        "independent stages. Compiled means singleton IR validation. Deck admission and gameplay",
        "require separate checks.", "",
        "| Deck | Slots | Compiled | Parsed | Fully bound | Blocked |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for deck in report["decks"]:
        label = markdown_text(f'{deck["name"]} (#{deck["id"]}, {deck["category"]})')
        lines.append(count_row(label, deck["slotCounts"]))
    for key, label in (("all", "All"), ("paper", "Paper only")):
        summary = report[key]
        lines.append(count_row(f'**{label} ({summary["decks"]} decks)**', summary["slotCounts"]))
    lines += ["", f'**Compiler-complete decks** {report["all"]["compilerCompleteDecks"]} overall and '
              f'{report["paper"]["compilerCompleteDecks"]} paper.', "",
              "The rankings use the first observed compiler rejection per card. Slot unlocks count every",
              "copy with the code. Deck unlocks count decks whose blocked slots all carry that code.",
              "These are hypothetical gains. A fix can expose a later rejection. Parser and archive",
              "blockers still count against deck completion.", ""]
    for key, label in (("byDeckSlotsUnlocked", "deck slots unlocked"), ("byDecksUnlocked", "decks unlocked")):
        entries = report["reasonRankings"][key]
        lines += [f"## Top compiler rejections by {label}", ""]
        if key == "byDecksUnlocked" and not any(entry["decksUnlocked"] for entry in entries):
            lines += ["Every reason ties at zero decks unlocked.", ""]
            continue
        lines += [
                  "| Reason code | Slots unlocked | Decks unlocked | Decks affected | Paper slots | Paper decks unlocked |",
                  "| --- | ---: | ---: | ---: | ---: | ---: |"]
        for entry in entries[:15]:
            lines.append("| " + " | ".join([markdown_text(entry["reasonCode"])] + [str(entry[field]) for field in (
                "deckSlotsUnlocked", "decksUnlocked", "decksAffected", "paperDeckSlotsUnlocked", "paperDecksUnlocked",
            )]) + " |")
        lines.append("")

    lines += ["## Blocking cards by deck", ""]
    for deck in report["decks"]:
        lines += ["### " + markdown_text(f'{deck["name"]} (#{deck["id"]})'), ""]
        cards = {card["oracleId"]: card for card in deck["cards"]}
        if not deck["blockerGroups"]:
            lines += ["Every slot compiled.", ""]
            continue
        lines += ["| Mechanic family | Reason code | Slots | Cards |", "| --- | --- | ---: | --- |"]
        for group in deck["blockerGroups"]:
            members = []
            for oracle_id in group["oracleIds"]:
                card = cards[oracle_id]
                qualifier = " (commander)" if card["isCommander"] else ""
                members.append(f'{markdown_text(card["name"])} ×{card["quantity"]}{qualifier}')
            lines.append(f'| {markdown_text(group["mechanicFamily"])} | {markdown_text(group["reasonCode"])} '
                         f'| {group["slots"]} | {", ".join(members)} |')
        lines.append("")
    return "\n".join(lines)


def write_reports(report, directory):
    directory.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=directory) as workspace:
        staged = Path(workspace)
        json_path = staged / "deck-gap-report.json"
        md_path = staged / "deck-gap-report.md"
        json_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        md_path.write_text(render_markdown(report), encoding="utf-8")
        for path in (json_path, md_path):
            path.replace(directory / path.name)


def main(argv=None, output_dir=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--decks", required=True, type=Path, help="production deck snapshot JSON")
    parser.add_argument("--archive", required=True, type=Path, help="exact Oracle AST archive JSONL")
    parser.add_argument("--coverage", required=True, type=Path, help="coverage summary JSON or receipt directory")
    args = parser.parse_args(argv)
    if output_dir is None:
        output_dir = Path(__file__).resolve().parents[1] / "generated" / "phase1-coverage"
    try:
        write_reports(build_report(args.decks, args.archive, args.coverage), output_dir)
    except (OSError, ValueError, TypeError, KeyError) as error:
        parser.exit(1, f"phase1-deck-coverage: {error}\n")


if __name__ == "__main__":
    main()
