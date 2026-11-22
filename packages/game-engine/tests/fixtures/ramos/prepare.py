"""Stage revision 17 with one production catalog build (run under the Cargo lock).

Usage: python3 prepare.py BINARY NEW_OUTPUT [--r4 ARCHIVE.jsonl] [AUTHORIZED_REPARSE.catalog.json ...]
NEW_OUTPUT must be under this checkout's generated directory. Each optional
singleton manifest supplies the registered record and its provenance row.
With --r4, the R4_CARDS rows come from the accepted bulk Archive R4 file: the
whole file, its header and each registered row are verified, and each row must
carry its frozen card's exact source. A card is taken from exactly one origin;
nothing falls back to a frozen revision-17 row the current compiler rejects.
The stage is self-contained: `BINARY catalog build <stage>/catalogs/ramos.json`
rebuilds it from its own records.
"""
import argparse
import ctypes
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

from audit import HERE, read, sha, verify

FIXTURES = HERE.parent
GENERATED = HERE.parents[4] / "generated"


TARGET_SHA256 = "639e62900042296f1bbcbc200c76aeb3bcfe761ec40d43489b3bf19a2d5612e5"
R4_MANIFEST = "archive-manifest-phase1-r4.json"
# Frozen cards whose revision-17 rows the current compiler rejects: each is
# compiled from its accepted Archive R4 row instead.
R4_CARDS = {
    "0623c51a-e829-413f-a3a0-817c0902821e": "Call Forth the Tempest",
    "23695bc2-86d3-4d78-b374-9e83c4cc9cc4": "Averna, the Chaos Bloom",
    "26445dc7-8363-4205-aee8-1cafeb4ba4c0": "Omnath, Locus of All",
    "2d7e00b6-12f0-4b03-82a6-50e3d1b5395e": "Progenitus",
    "4137a22c-f793-4e27-a9b2-72740a6e2121": "Abzan Charm",
    "7cb9e29f-835f-4155-a2a5-4b778866c773": "Mosswort Bridge",
    "9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f": "Yidris, Maelstrom Wielder",
    "c336b735-3e6b-471e-be7a-84d7402ae2f5": "Creative Technique",
    "cf127842-5fb1-44df-8d91-8cc0f48c559a": "Sakashima's Protege",
    "ece44a82-dcf0-4439-bdd9-a09c99a6f159": "Double Major",
}


def publish(stage, output):
    """Linux renameat2(2), RENAME_NOREPLACE; both paths are absolute."""
    rename = ctypes.CDLL(None, use_errno=True).renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    at_fdcwd, rename_noreplace = -100, 1
    if rename(at_fdcwd, os.fsencode(stage), at_fdcwd, os.fsencode(output), rename_noreplace):
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), output)


def write(path, data):
    path.write_text(json.dumps(data, indent=2) + "\n")


def replacements(paths, frozen):
    origins = read(FIXTURES / "accepted-archives.json")
    selected = {}
    for path in paths:
        manifest = read(path)
        file, = manifest["records"]
        record = (path.parent / file).resolve()
        provenance = (path.parent / manifest["provenance"]).resolve()
        row, = [r for r in read(provenance)["records"]
                if (provenance.parent / r["file"]).resolve() == record]
        key = row["oracle_id"]
        raw = record.read_bytes()
        value = json.loads(raw)
        digest = sha(raw.rstrip())
        origin, = [o for o in origins if o["manifest_file"] == row["archive"]
                   and digest in o["record_sha256"]]
        # Registered reparse archives contain the header and one record. Verify
        # the entire archive digest to bind the raw line to its claimed position.
        header = (FIXTURES / origin["manifest_file"]).read_bytes()
        if (type(row.get("archive_line")) is not int or row["archive_line"] != 2
                or len(raw.splitlines()) != 1 or len(header.splitlines()) != 1
                or sha(header.rstrip()) != origin["manifest_sha256"]
                or sha(header + raw) != origin["file_sha256"]
                or value["oracleIdentity"] != key or row["ast_sha256"] != value["astSha256"]
                or ("binding_sha256" in row and row["binding_sha256"] != value["bindingSha256"])):
            raise ValueError(f"reparse provenance differs from registered archive for {key}")
        if (key in selected or digest != row["record_sha256"]
                or digest == frozen[key]["record_sha256"]
                or value["source"] != read(HERE / frozen[key]["file"])["source"]
                or value["sourceSha256"] != frozen[key]["source_sha256"]
                or row["source_sha256"] != frozen[key]["source_sha256"]):
            raise ValueError(f"duplicate or inexact reparse for {key}")
        selected[key] = raw, row, origin, header
    return selected


def r4_rows(archive, frozen, keys):
    """Registered rows of the accepted bulk Archive R4 file, by oracle id.

    The whole file must match the accepted origin's file digest and its first
    line the registered header, so every row is bound to its line. Each
    selected row must be registered by digest and carry the frozen card's
    exact source and source digest."""
    origin, = [o for o in read(FIXTURES / "accepted-archives.json") if o["manifest_file"] == R4_MANIFEST]
    header = (FIXTURES / R4_MANIFEST).read_bytes()
    data = archive.read_bytes()
    if sha(data) != origin["file_sha256"] or sha(header.rstrip()) != origin["manifest_sha256"]:
        raise ValueError("bulk R4 archive or header differs from the accepted origin")
    lines = data.splitlines(keepends=True)
    if lines[0] != header:
        raise ValueError("bulk R4 archive header differs from the registered manifest")
    selected = {}
    for number, raw in enumerate(lines[1:], 2):
        if not any(f'"oracleIdentity":"{key}"'.encode() in raw for key in keys):
            continue
        value = json.loads(raw)
        key = value["oracleIdentity"]
        if key not in keys:
            continue
        digest = sha(raw.rstrip())
        if (key in selected or digest not in origin["record_sha256"]
                or value["source"] != read(HERE / frozen[key]["file"])["source"]
                or value["sourceSha256"] != frozen[key]["source_sha256"]):
            raise ValueError(f"unregistered, duplicate or inexact R4 row for {key}")
        row = {"file": frozen[key]["file"], "oracle_id": key, "record_sha256": digest,
               "source_sha256": value["sourceSha256"], "ast_sha256": value["astSha256"],
               "binding_sha256": value["bindingSha256"], "archive": R4_MANIFEST,
               "archive_line": number}
        selected[key] = raw, row, origin, header
    if set(selected) != set(keys):
        raise ValueError(f"bulk R4 archive lacks {sorted(set(keys) - set(selected))}")
    return selected


def profile(target):
    cards = target["cards"]
    names = {c["name"]: c["oracle_id"] for c in cards}
    decks = [{"commander": target["deck"]["commander_oracle_id"],
              "cards": [{"oracle_id": c["oracle_id"], "count": c["quantity"]} for c in cards]}]
    for name, lands in [
        ("Averna, the Chaos Bloom", ["Island", "Mountain", "Forest"]),
        ("Maelstrom Wanderer", ["Island", "Mountain", "Forest"]),
        ("Selvala, Heart of the Wilds", ["Forest"]),
    ]:
        commander = names[name]
        entries = [{"oracle_id": commander, "count": 1}]
        entries.extend({"oracle_id": names[n], "count": 99 // len(lands)} for n in lands)
        decks.append({"commander": commander, "cards": entries})
    sources = {c["oracle_id"]: read(HERE / c["file"])["source"] for c in cards}
    policy = read(HERE / target["source_boundary"]["banned_policy_file"])
    bans = {r["oracle_id"] for r in policy["named_bans"] + policy["offensive_policy"]["records"]}
    for deck in decks:
        leader = sources[deck["commander"]]
        types = leader["type_line"].split(" — ")[0].split()
        if "Legendary" not in types or "Creature" not in types:
            raise ValueError(f"ineligible commander {leader['name']}")
        for entry in deck["cards"]:
            key = entry["oracle_id"]
            source = sources[key]
            if (key in bans or source["legalities"]["commander"] != "legal"
                    or not set(source["color_identity"]) <= set(leader["color_identity"])
                    or (entry["count"] > 1 and not source["type_line"].startswith("Basic Land"))):
                raise ValueError(f"ineligible deck entry {source['name']}")
    template = read(FIXTURES / "profiles/commander-four-colors-v1.json")
    return dict(template, name="commander-ramos-v1", catalog_manifest="../catalogs/ramos.json",
                decks=decks, commanders=[d["commander"] for d in decks], hosted=False, search=False)


def prepare(binary, output, paths, r4=None):
    if not __debug__:
        raise ValueError("frozen audit requires Python assertions")
    raw_manifest = (HERE / "revision-17.manifest.json").read_bytes()
    if sha(raw_manifest) != TARGET_SHA256:
        raise ValueError("revision-17 manifest digest mismatch")
    target = verify()
    if target != json.loads(raw_manifest):
        raise ValueError("revision-17 manifest changed during verification")
    frozen = {c["oracle_id"]: c for c in target["cards"]}
    deck = target["deck"]
    commanders = [c for c in target["cards"] if c["zone"] == "command"]
    if (sum(c["quantity"] for c in target["cards"]) != 100 or deck["frozen_source_land_count"] != 37
            or len(commanders) != 1 or commanders[0]["quantity"] != 1):
        raise ValueError("expected 100 cards, 37 lands and one designated commander")
    if len(frozen) != 85 or os.path.lexists(output) or GENERATED not in output.parents:
        raise ValueError("expected 85 frozen IDs and a new generated output directory")
    prepared_profile = profile(target)
    swaps = replacements(paths, frozen)
    if r4 is not None:
        bulk = r4_rows(r4, frozen, set(R4_CARDS))
        if set(bulk) & set(swaps):
            raise ValueError("a card has both a reparse manifest and an R4 row")
        swaps.update(bulk)
    provenance = read(HERE / "provenance.json")
    origins = {o["manifest_file"]: o for o in read(HERE / "accepted-archives.json")}
    original = {r["oracle_id"]: r for r in provenance["records"]}
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        stage = Path(temp) / "stage"
        records = stage / "records"
        records.mkdir(parents=True)
        rows = []
        for key, card in frozen.items():
            if key in swaps:
                raw, row, origin, header = swaps[key]
                origins[row["archive"]] = origin
            else:
                row = original[key]
                raw = (HERE / card["file"]).read_bytes()
                header = (HERE / row["archive"]).read_bytes()
                if sha(raw) != card["file_sha256"]:
                    raise ValueError(f"frozen record changed for {key}")
            (records / card["file"]).write_bytes(raw)
            rows.append(dict(row, file=card["file"]))
            (records / row["archive"]).write_bytes(header)
        write(records / "provenance.json", dict(provenance, records=rows))
        write(records / "accepted-archives.json", list(origins.values()))
        catalogs = stage / "catalogs"
        catalogs.mkdir()
        manifest = catalogs / "ramos.json"
        write(manifest, dict(schema_version=1, output_order="canonical",
                             provenance="../records/provenance.json",
                             records=["../records/" + r["file"] for r in rows],
                             expected_ir="../expected-ramos-ir.json", expected_digest="ramos.sha256"))
        producer = sha(binary.read_bytes())
        subprocess.run([binary, "catalog", "build", manifest, "--write"], check=True)
        definitions = read(stage / "expected-ramos-ir.json")["definitions"]
        if (len(definitions) != 85 or sha(binary.read_bytes()) != producer or
                {d["oracle_id"]: d["provenance"]["source_sha256"] for d in definitions}
                != {k: c["source_sha256"] for k, c in frozen.items()}):
            raise ValueError("compiled catalog or producer identity mismatch")
        (stage / "profiles").mkdir()
        write(stage / "profiles/commander-ramos-v1.json", prepared_profile)
        policy = HERE / target["source_boundary"]["banned_policy_file"]
        shutil.copyfile(policy, stage / policy.name)
        write(stage / "producer.json", {"binary_sha256": producer, "target_manifest_sha256": TARGET_SHA256})
        publish(stage, output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--r4", type=Path, help="accepted bulk Archive R4 file for R4_CARDS")
    parser.add_argument("reparse", nargs="*", type=Path)
    args = parser.parse_args()
    prepare(args.binary.resolve(), args.output.parent.resolve() / args.output.name, args.reparse,
            args.r4.resolve() if args.r4 else None)
