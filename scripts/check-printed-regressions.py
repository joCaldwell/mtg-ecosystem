#!/usr/bin/env python3
"""Compare D0-verified R6 candidates by face, block and AST node path."""

import argparse
import json
from itertools import zip_longest
from pathlib import Path

STRONG_BASES = {"recorded", "alias", "implicit"}


def coverage(node, nodes):
    if node["basis"] == "alias":
        node = nodes[node["of"]]
    return {
        (line, index)
        for field in ("span", "folds", "clauses")
        for line, start, end in node.get(field, ())
        for index in range(start, end)
    }


def read_blocks(row):
    blocks = {}
    for face in row.get("printed", {}).get("faces", ()):
        for index, block in enumerate(face.get("blocks", ())):
            if block["status"] == "read":
                blocks[face["sourceFaceIndex"], index] = block
    return blocks


def compare_row(old, new, receipt):
    source_index = old["sourceIndex"]
    old_faces = old.get("printed", {}).get("faces", ())
    new_faces = new.get("printed", {}).get("faces", ())
    for before, after in zip_longest(old_faces, new_faces, fillvalue={}):
        if (before.get("sourceFaceIndex"), before.get("status"), before.get("lines")) != (
            after.get("sourceFaceIndex"), after.get("status"), after.get("lines")
        ):
            receipt["regressions"].append({
                "sourceIndex": source_index, "face": before.get("sourceFaceIndex"),
                "reason": "printed token coordinates changed",
            })
    old_blocks, new_blocks = read_blocks(old), read_blocks(new)
    if old_blocks.keys() != new_blocks.keys():
        receipt["regressions"].append({"sourceIndex": source_index, "reason": "read blocks changed"})
    for site in sorted(old_blocks.keys() | new_blocks.keys()):
        before = {node["path"]: node for node in old_blocks.get(site, {}).get("nodes", ())}
        after = {node["path"]: node for node in new_blocks.get(site, {}).get("nodes", ())}
        receipt["nodesCompared"] += len(before.keys() | after.keys())
        if before == after:
            continue
        for path in sorted(before.keys() | after.keys()):
            previous, current = before.get(path), after.get(path)
            if previous is None or current is None:
                receipt["regressions"].append({
                    "sourceIndex": source_index, "face": site[0], "block": site[1],
                    "path": path, "reason": "AST node inventory changed",
                })
                continue
            # An unchanged alias can still gain or lose coverage when its target changes.
            old_tokens, new_tokens = coverage(previous, before), coverage(current, after)
            if previous == current and old_tokens == new_tokens:
                continue
            lost = sorted(old_tokens - new_tokens)
            lower_basis = previous["basis"] in STRONG_BASES and current["basis"] not in STRONG_BASES
            change = {
                "sourceIndex": source_index, "name": old["source"]["name"],
                "face": site[0], "block": site[1], "path": path,
                "before": previous, "after": current,
                "addedTokens": sorted(new_tokens - old_tokens), "lostTokens": lost,
                "lowerBasis": lower_basis,
            }
            receipt["nodeChanges"].append(change)
            if lost or lower_basis:
                receipt["regressions"].append(change)


def compare(baseline, candidate, previous):
    receipt = {
        "rows": 0, "nodesCompared": 0, "changedRows": [],
        "changedSincePrevious": [], "nodeChanges": [], "regressions": [],
    }
    headers = [json.loads(stream.readline()) for stream in (baseline, candidate, previous)]
    expected = headers[0]["inputRecords"]
    if any(header["recordType"] != "manifest" or header["schemaVersion"] != 5
           or header["inputRecords"] != expected or header["legacy"] != headers[0]["legacy"]
           for header in headers):
        raise ValueError("Expected schema-5 candidates from the same frozen R5 archive")

    for index, (old_line, new_line, previous_line) in enumerate(zip_longest(baseline, candidate, previous)):
        if None in (old_line, new_line, previous_line):
            raise ValueError(f"Archives have different lengths at source index {index}")
        old = json.loads(old_line)
        new = old if old_line == new_line else json.loads(new_line)
        prior = new if previous_line == new_line else json.loads(previous_line)
        for row in (old, new, prior):
            if (row["sourceIndex"], row["oracleIdentity"], row["sourceSha256"]) != (
                index, old["oracleIdentity"], old["sourceSha256"]
            ):
                raise ValueError(f"Archive source identity differs at source index {index}")
        receipt["rows"] += 1
        identity = {"sourceIndex": index, "name": old["source"]["name"]}
        if old_line != new_line:
            receipt["changedRows"].append(identity)
        if previous_line != new_line:
            receipt["changedSincePrevious"].append(identity)
        compare_row(old, new, receipt)

    if receipt["rows"] != expected:
        raise ValueError(f"Expected {expected} source rows, found {receipt['rows']}")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path, help="preserved candidate v2")
    parser.add_argument("candidate", type=Path, help="candidate under test")
    parser.add_argument("previous", type=Path, help="previous candidate for changed-row counts")
    args = parser.parse_args()
    with args.baseline.open("rb") as baseline, args.candidate.open("rb") as candidate, args.previous.open("rb") as previous:
        receipt = compare(baseline, candidate, previous)
    print(json.dumps(receipt, indent=2))
    return bool(receipt["regressions"])


if __name__ == "__main__":
    raise SystemExit(main())
