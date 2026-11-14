#!/usr/bin/env python3
"""Revision-17 input check and singleton production-catalog audit; see support doc.

No parser, compiler, admission, or diagnostic semantics are implemented here.
Run native invocations under the documented shared Cargo lock. Outputs are
create-new, ignored evidence; the frozen baseline is never regenerated in place.
"""
import hashlib
import json
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


def verify():
    manifest = read(HERE / "revision-17.manifest.json")
    deck, boundary, cards = (manifest[k] for k in ("deck", "source_boundary", "cards"))
    export = (HERE / deck["export_file"]).read_bytes()
    assert sha(export) == deck["export_sha256"]
    assert export.endswith(b"\n") and b"\r" not in export
    assert export.decode() == "".join(f"{c['quantity']} {c['name']}\n" for c in cards)
    assert len(cards) == deck["definition_count"] == len({c["oracle_id"] for c in cards})
    assert sum(c["quantity"] for c in cards) == deck["quantity_count"]
    assert [c["oracle_id"] for c in cards if c["zone"] == "command"] == [deck["commander_oracle_id"]]
    assert sha((HERE / boundary["archive_manifest_file"]).read_bytes().rstrip()) == boundary["archive_manifest_sha256"]
    assert sha((HERE / boundary["banned_policy_file"]).read_bytes()) == boundary["banned_policy_file_sha256"]
    provenance = {r["oracle_id"]: r for r in read(HERE / "provenance.json")["records"]}
    assert set(provenance) == {c["oracle_id"] for c in cards}
    origin = next(r for r in read(HERE / "accepted-archives.json") if r["file_sha256"] == boundary["archive_sha256"])
    lands = 0
    for c in cards:
        raw = (HERE / c["file"]).read_bytes()
        assert sha(raw) == c["file_sha256"]
        assert sha(raw.rstrip()) == c["record_sha256"]
        assert c["record_sha256"] in origin["record_sha256"]
        r = json.loads(raw)
        assert r["oracleIdentity"] == r["source"]["oracle_id"] == c["oracle_id"]
        assert r["source"]["name"] == c["name"]
        for key, archive_key in (("source_sha256", "sourceSha256"), ("ast_sha256", "astSha256"), ("binding_sha256", "bindingSha256")):
            assert r.get(archive_key) == c[key]
        assert r["status"] == c["parse_status"]
        assert r["binding"]["fullyBound"] == c["binding_fully_bound"]
        assert r["source"]["legalities"]["commander"] == c["commander_legality"]
        for key in ("file", "archive_line", "record_sha256", "source_sha256", "ast_sha256"):
            assert provenance[c["oracle_id"]][key] == c[key]
        lands += c["quantity"] if "Land" in r["source"]["type_line"] else 0
    assert lands == deck["frozen_source_land_count"]
    return manifest


def audit(binary, output, manifest):
    output.mkdir(parents=True, exist_ok=False)
    rows = []
    report = {"format_version": 1, "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
              "target_manifest_sha256": sha((HERE / "revision-17.manifest.json").read_bytes()),
              "binary": str(binary), "binary_sha256": sha(binary.read_bytes()), "records": rows}
    for c in manifest["cards"]:
        stem = output / c["oracle_id"]
        catalog = {"schema_version": 1, "output_order": "canonical", "provenance": str(HERE / "provenance.json"),
                   "records": [str(HERE / c["file"])], "expected_ir": str(stem.with_suffix(".ir.json")),
                   "expected_digest": str(stem.with_suffix(".sha256"))}
        path = stem.with_suffix(".catalog.json")
        path.write_text(json.dumps(catalog, indent=2) + "\n")
        command = [str(binary), "catalog", "build", str(path), "--write"]
        run = subprocess.run(command, capture_output=True, text=True)
        row = {"oracle_id": c["oracle_id"], "name": c["name"], "command": command,
               "exit_code": run.returncode, "stdout": run.stdout, "stderr": run.stderr}
        if run.returncode == 0:
            definition = read(stem.with_suffix(".ir.json"))["definitions"][0]
            row.update(status="lowered_validated", capabilities=definition["capabilities"],
                       singleton_catalog_sha256=stem.with_suffix(".sha256").read_text())
        else:
            row["status"] = "catalog_error"
            # Decode only the CLI's Rust Debug envelope. Exact output stays above.
            envelope = re.fullmatch(r'Error: InvalidData \{ reason: ("(?:\\.|[^"\\])*") \}\n', run.stderr)
            if envelope:
                detail = json.loads(envelope[1])
                diagnostic = re.search(r': (Unsupported|Invalid) \{ path: ("(?:\\.|[^"\\])*"), reason: ("(?:\\.|[^"\\])*") \}$', detail)
                if diagnostic:
                    row.update(status=diagnostic[1].lower(), path=json.loads(diagnostic[2]), reason=json.loads(diagnostic[3]))
        rows.append(row)
        # Preserve each result immediately, including unexpected failures.
        (output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({s: sum(r["status"] == s for r in rows) for s in sorted({r["status"] for r in rows})}))
    assert all(r["status"] != "catalog_error" for r in rows), "inspect preserved unexpected CLI errors"


if __name__ == "__main__":
    target = verify()
    if sys.argv[1:] == ["--verify"]:
        print(json.dumps({"verified_definitions": len(target["cards"]), "cards": target["deck"]["quantity_count"], "lands": target["deck"]["frozen_source_land_count"]}))
    elif len(sys.argv) == 3:
        audit(pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve(), target)
    else:
        raise SystemExit("usage: audit.py --verify | audit.py MTG_SIM_BINARY NEW_OUTPUT_DIRECTORY")
