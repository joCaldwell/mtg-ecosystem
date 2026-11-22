#!/usr/bin/env python3
"""Stage any Commander deck as a self-contained catalog and match profile from Archive R4.

  stage-deck.py MTG_SIM OUTPUT --deck "Jo's Cascade" [--decks phase1-target-decks.json] [--archive R4.jsonl]
  stage-deck.py MTG_SIM OUTPUT --decklist deck.json [--archive R4.jsonl]

A decklist is the target-deck shape: {"name", "cards": [{"oracle_id", "name",
"quantity", "is_commander"}]}: positive integer quantities, exactly 100 cards and
one commander. Every card comes from its row in the accepted bulk Archive R4
file: the whole file must match the registered R4 origin
(tests/fixtures/accepted-archives.json) and its first line the registered R4
header, so each row is bound to its line. No other archive revision is
accepted; a card the compiler rejects fails the stage.

OUTPUT (new) mirrors the fixture tree, so installing it under tests/fixtures
only adds files: deck-SLUG/ (rows, header, provenance, origin), catalogs/
deck-SLUG.{json,sha256}, expected-deck-SLUG-ir.json and profiles/
commander-deck-SLUG-v1.json (the deck in all four seats, headless only). The
Commander policy file is copied for a byte comparison with the installed one.
`MTG_SIM catalog build OUTPUT/catalogs/deck-SLUG.json` rebuilds the catalog.
"""
import argparse
import ctypes
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "packages" / "game-engine" / "tests" / "fixtures"
TEMPLATE = FIXTURES / "profiles" / "commander-four-colors-v1.json"
R4_MANIFEST = "archive-manifest-phase1-r4.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    return json.loads(Path(path).read_bytes())


def write(path, data):
    path.write_text(json.dumps(data, indent=2) + "\n")


def shared_generated(name):
    """This checkout's generated/ input, else the primary checkout's."""
    local = ROOT / "generated" / name
    if local.exists():
        return local
    common = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], cwd=ROOT,
                            capture_output=True, text=True).stdout.strip()
    return Path(common).parent / "generated" / name if common else local


def slug(name):
    value = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    if not value:
        raise ValueError(f"deck name {name!r} has no slug")
    return value


def select_deck(args):
    if args.decklist:
        return read(args.decklist)
    decks = read(args.decks)["decks"]
    matches = [d for d in decks if args.deck in (d["name"], slug(d["name"]), str(d.get("id")))]
    if len(matches) != 1:
        names = ", ".join(repr(d["name"]) for d in decks)
        raise ValueError(f"expected one deck named {args.deck!r}; available: {names}")
    return matches[0]


def accepted_origin(archive, fixtures=FIXTURES):
    """The registered Archive R4 origin, if ARCHIVE is exactly its whole file and header."""
    data = archive.read_bytes()
    header = data.split(b"\n", 1)[0] + b"\n"
    digest = sha(data)
    origins = [o for o in read(fixtures / "accepted-archives.json") if o["manifest_file"] == R4_MANIFEST]
    if len(origins) != 1:
        raise ValueError(f"expected one registered {R4_MANIFEST} origin")
    origin = origins[0]
    registered = fixtures / R4_MANIFEST
    if (origin["file_sha256"] != digest or origin["manifest_sha256"] != sha(header.rstrip())
            or registered.read_bytes() != header):
        raise ValueError(f"{archive} is not the registered Archive R4 file (file SHA-256 {digest})")
    return origin, header, data


def archive_rows(data, keys):
    """The unique row of each selected oracle id, with its 1-based line number."""
    selected = {}
    for number, raw in enumerate(data.splitlines(keepends=True)[1:], 2):
        start = raw.find(b'"oracleIdentity":"')
        if start < 0:
            continue
        key = raw[start + 18:raw.index(b'"', start + 18)].decode()
        if key not in keys:
            continue
        value = json.loads(raw)
        if value["oracleIdentity"] != key or key in selected:
            raise ValueError(f"duplicate or misplaced archive row for {key}")
        selected[key] = raw.rstrip(b"\n"), value, number
    missing = sorted(set(keys) - set(selected))
    if missing:
        raise ValueError(f"archive lacks rows for {missing}")
    return selected


def validate_deck(deck, rows, policy):
    cards = deck["cards"]
    for card in cards:
        # JSON booleans are Python ints; a profile count is a positive u32.
        quantity = card.get("quantity")
        if type(quantity) is not int or not 0 < quantity < 2**32:
            raise ValueError(f"quantity of {card.get('name', card.get('oracle_id'))!r} must be a positive integer")
    commanders = [c for c in cards if c.get("is_commander") is True]
    if sum(c["quantity"] for c in cards) != 100 or len(commanders) != 1 or commanders[0]["quantity"] != 1:
        raise ValueError("a staged deck has exactly 100 cards and one commander")
    if len({c["oracle_id"] for c in cards}) != len(cards):
        raise ValueError("duplicate decklist entries")
    bans = {r["oracle_id"] for r in policy["named_bans"] + policy["offensive_policy"]["records"]}
    leader = rows[commanders[0]["oracle_id"]][1]["source"]
    types = leader["type_line"].split(" — ")[0].split()
    if "Legendary" not in types or "Creature" not in types:
        raise ValueError(f"ineligible commander {leader['name']}")
    for card in cards:
        source = rows[card["oracle_id"]][1]["source"]
        if card.get("name") not in (None, source["name"]):
            raise ValueError(f"decklist name {card['name']!r} differs from archived {source['name']!r}")
        basic = source["type_line"].split(" — ")[0].split()[:2] == ["Basic", "Land"]
        if (card["oracle_id"] in bans or source["legalities"]["commander"] != "legal"
                or not set(source["color_identity"]) <= set(leader["color_identity"])
                or (card["quantity"] > 1 and not basic)):
            raise ValueError(f"ineligible deck entry {source['name']}")
    return commanders[0]["oracle_id"]


def publish(stage, output):
    """Linux renameat2(2) with RENAME_NOREPLACE: never replaces anything at OUTPUT."""
    rename = ctypes.CDLL(None, use_errno=True).renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    at_fdcwd, rename_noreplace = -100, 1
    if rename(at_fdcwd, os.fsencode(stage), at_fdcwd, os.fsencode(output), rename_noreplace):
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), str(output))


def new_output(path):
    """OUTPUT with only its parent resolved; anything (even a dangling symlink) at OUTPUT rejects."""
    output = path.parent.resolve() / path.name
    if os.path.lexists(output):
        raise ValueError(f"{output} already exists")
    return output


def stage(binary, output, deck, archive):
    output = new_output(output)
    origin, header, data = accepted_origin(archive)
    name = slug(deck["name"])
    keys = {c["oracle_id"] for c in deck["cards"]}
    rows = archive_rows(data, keys)
    template = read(TEMPLATE)
    policy_file = (TEMPLATE.parent / template["commander_policy"]["banned_list"]).resolve()
    commander = validate_deck(deck, rows, read(policy_file))
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        out = Path(temp) / "stage"
        records = out / f"deck-{name}"
        records.mkdir(parents=True)
        provenance = []
        for key in sorted(keys):
            raw, value, number = rows[key]
            (records / f"{key}.json").write_bytes(raw + b"\n")
            provenance.append({"file": f"{key}.json", "oracle_id": key, "record_sha256": sha(raw),
                               "source_sha256": value["sourceSha256"], "ast_sha256": value["astSha256"],
                               "binding_sha256": value["bindingSha256"], "archive": origin["manifest_file"],
                               "archive_line": number})
        (records / origin["manifest_file"]).write_bytes(header)
        rules = read(FIXTURES / "provenance.json")
        write(records / "provenance.json", {"fixture_version": rules["fixture_version"],
                                            "rules_edition": rules["rules_edition"],
                                            "rules_sha256": rules["rules_sha256"], "records": provenance})
        write(records / "accepted-archives.json",
              [dict(origin, record_sha256=sorted(r["record_sha256"] for r in provenance))])
        (out / "catalogs").mkdir()
        manifest = out / "catalogs" / f"deck-{name}.json"
        write(manifest, {"schema_version": 1, "output_order": "canonical",
                         "provenance": f"../deck-{name}/provenance.json",
                         "records": [f"../deck-{name}/{r['file']}" for r in provenance],
                         "expected_ir": f"../expected-deck-{name}-ir.json",
                         "expected_digest": f"deck-{name}.sha256"})
        producer = sha(binary.read_bytes())
        build = subprocess.run([binary, "catalog", "build", manifest, "--write"], capture_output=True, text=True)
        if build.returncode:
            sys.stderr.write(build.stdout + build.stderr)
            raise ValueError(f"{deck['name']} does not compile completely from {archive.name}")
        definitions = read(out / f"expected-deck-{name}-ir.json")["definitions"]
        if (sha(binary.read_bytes()) != producer
                or {d["oracle_id"]: d["provenance"]["source_sha256"] for d in definitions}
                != {r["oracle_id"]: r["source_sha256"] for r in provenance}):
            raise ValueError("compiled catalog or producer identity mismatch")
        seat = {"commander": commander,
                "cards": [{"oracle_id": c["oracle_id"], "count": c["quantity"]}
                          for c in sorted(deck["cards"], key=lambda c: c["oracle_id"])]}
        (out / "profiles").mkdir()
        write(out / "profiles" / f"commander-deck-{name}-v1.json",
              dict(template, name=f"commander-deck-{name}-v1", catalog_manifest=f"../catalogs/deck-{name}.json",
                   decks=[seat] * 4, commanders=[commander] * 4, hosted=False, search=False))
        shutil.copyfile(policy_file, out / policy_file.name)
        write(out / "producer.json", {"binary_sha256": producer, "deck": deck["name"], "slug": name,
                                      "profile": f"commander-deck-{name}-v1", "archive_sha256": origin["file_sha256"],
                                      "archive_manifest_sha256": origin["manifest_sha256"],
                                      "cards": len(keys), "slots": sum(c["quantity"] for c in deck["cards"])})
        try:
            publish(out, output)
        except FileExistsError as error:
            raise ValueError(f"{output} appeared during staging") from error
    return f"commander-deck-{name}-v1"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("binary", type=Path, help="release mtg-sim built from this checkout")
    parser.add_argument("output", type=Path)
    choice = parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--deck", help="deck name, slug or id in --decks")
    choice.add_argument("--decklist", type=Path, help="decklist JSON in the target-deck shape")
    parser.add_argument("--decks", type=Path, default=shared_generated("phase1-target-decks.json"))
    parser.add_argument("--archive", type=Path, default=shared_generated("oracle-ast-phase1-r4.jsonl"))
    args = parser.parse_args(argv)
    try:
        profile = stage(args.binary.resolve(), args.output, select_deck(args), args.archive.resolve())
    except ValueError as error:
        sys.exit(f"stage-deck: {error}")
    print(profile)


if __name__ == "__main__":
    main()
