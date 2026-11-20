"""Stage revision 17 with one production catalog build (run under the Cargo lock).

Usage: python3 prepare.py BINARY NEW_OUTPUT [AUTHORIZED_REPARSE.catalog.json ...]
NEW_OUTPUT must be under this checkout's generated directory. Each optional
singleton manifest supplies the registered record and its provenance row.
"""
import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

from audit import HERE, read, sha, verify

FIXTURES = HERE.parent
GENERATED = HERE.parents[4] / "generated"


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
        value = read(record)
        digest = sha(record.read_bytes().rstrip())
        origin, = [o for o in origins if o["manifest_file"] == row["archive"]
                   and digest in o["record_sha256"]]
        if (key in selected or digest != row["record_sha256"]
                or digest == frozen[key]["record_sha256"]
                or value["source"] != read(HERE / frozen[key]["file"])["source"]
                or value["sourceSha256"] != frozen[key]["source_sha256"]
                or row["source_sha256"] != frozen[key]["source_sha256"]):
            raise ValueError(f"duplicate or inexact reparse for {key}")
        selected[key] = record, row, origin
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


def prepare(binary, output, paths):
    if not __debug__:
        raise ValueError("frozen audit requires Python assertions")
    target = verify()
    frozen = {c["oracle_id"]: c for c in target["cards"]}
    if len(frozen) != 85 or output.exists() or GENERATED not in output.parents:
        raise ValueError("expected 85 frozen IDs and a new generated output directory")
    prepared_profile = profile(target)
    swaps = replacements(paths, frozen)
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
            path, row = HERE / card["file"], original[key]
            if key in swaps:
                path, row, origin = swaps[key]
                origins[row["archive"]] = origin
            shutil.copyfile(path, records / card["file"])
            rows.append(dict(row, file=card["file"]))
            shutil.copyfile(FIXTURES / row["archive"], records / row["archive"])
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
        write(stage / "producer.json", {"binary_sha256": producer})
        stage.rename(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("reparse", nargs="*", type=Path)
    args = parser.parse_args()
    prepare(args.binary.resolve(), args.output.resolve(), args.reparse)
