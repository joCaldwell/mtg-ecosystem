"""Stage revision 17 with one production catalog build (run under the Cargo lock).

Usage: python3 prepare.py BINARY NEW_OUTPUT [AUTHORIZED_REPARSE.catalog.json ...]
NEW_OUTPUT must be under this checkout's generated directory. Each optional
singleton manifest supplies the registered record and its provenance row.
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
    parser.add_argument("reparse", nargs="*", type=Path)
    args = parser.parse_args()
    prepare(args.binary.resolve(), args.output.parent.resolve() / args.output.name, args.reparse)
