"""Compare declared native source files with regular blobs in a local commit."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def unique_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def check(repo, revision, manifest):
    def git(*args):
        return subprocess.check_output(["git", "--no-replace-objects", "-C", repo, *args])

    commit = git("rev-parse", "--verify", "--end-of-options", revision + "^{commit}").decode().strip()
    if (not isinstance(manifest, dict)
            or manifest.get("status") != "compiled-source-closure-verified"
            or not isinstance(manifest.get("build_id"), str) or not manifest["build_id"]
            or not isinstance(manifest.get("files"), list) or not manifest["files"]):
        raise ValueError("expected native-source-manifest exporter output with files")

    blobs = {}
    for entry in git("ls-tree", "-rz", "--full-tree", commit).split(b"\0"):
        if entry:
            header, name = entry.split(b"\t", 1)
            mode, _, oid = header.split()
            if mode in (b"100644", b"100755"):
                blobs[name] = oid

    seen = set()
    total = 0
    for row in manifest["files"]:
        path = row.get("path") if isinstance(row, dict) else None
        if (not isinstance(path, str) or any(c in path for c in ("\\", "\0"))
                or re.match(r"^[A-Za-z]:", path)
                or any(part in ("", ".", "..") for part in path.split("/"))):
            raise ValueError(f"invalid path {path!r}")
        if path in seen:
            raise ValueError(f"duplicate path {path!r}")
        seen.add(path)
        oid = blobs.get(path.encode())
        if oid is None:
            raise ValueError(f"missing regular blob {path!r} in {commit}")
        data = git("cat-file", "blob", oid)
        if type(row.get("bytes")) is not int or row["bytes"] != len(data):
            raise ValueError(f"byte count mismatch for {path!r} in {commit}")
        if row.get("sha256") != hashlib.sha256(data).hexdigest():
            raise ValueError(f"sha256 mismatch for {path!r} in {commit}")
        total += len(data)
    return f"{commit} matches {len(seen)} declared files ({total} bytes)"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repo", default=".")
    args = parser.parse_args()
    try:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"), object_pairs_hook=unique_keys)
        print(check(args.repo, args.commit, manifest))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")
