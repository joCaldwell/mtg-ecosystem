#!/usr/bin/env python3
"""Check repository Markdown count and local links without network or generation."""
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]


def markdown_source(path):
    # Fenced examples are not actual links/headings. Preserve inline code in
    # headings: GitHub includes its text in the fragment identifier.
    lines = []
    fence = None
    for line in path.read_text().splitlines():
        marker = re.match(r"^\s*(`{3,}|~{3,})", line)
        if marker:
            token = marker[1]
            if fence is None:
                fence = token
            elif token[0] == fence[0] and len(token) >= len(fence):
                fence = None
            lines.append("")
        else:
            lines.append(line if fence is None else "")
    return "\n".join(lines)


def anchors(source):
    result = set(re.findall(r'\b(?:id|name)=[\"\']([^\"\']+)[\"\']', source))
    seen = {}
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", source, re.M):
        text = re.sub(r"<[^>]+>", "", heading).lower()
        text = re.sub(r"[^\w\-\s]", "", text).replace(" ", "-")
        duplicate = seen.get(text, 0)
        seen[text] = duplicate + 1
        result.add(f"{text}-{duplicate}" if duplicate else text)
    return result


def main():
    files = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
    ).decode().split("\0")
    docs = sorted({p for p in files if p.lower().endswith((".md", ".markdown")) and (ROOT / p).is_file()})
    errors = []
    if len(docs) >= 100:
        errors.append(f"{len(docs)} Markdown files; repository must stay below 100 (prefer below 50)")
    sources = {ROOT / p: markdown_source(ROOT / p) for p in docs}
    fragments = {p: anchors(s) for p, s in sources.items()}
    checked = 0
    evidence = 0
    for name in docs:
        path = ROOT / name
        source = sources[path]
        links = list(re.finditer(r"\]\(<?([^\s)>]+)>?(?:\s+\"[^\"]*\")?\)", source))
        # Reference-style definitions have the same target semantics.
        links += list(re.finditer(r"^\s*\[[^\]]+\]:\s*<?([^\s>]+)>?", source, re.M))
        for link in links:
            target = urlsplit(link[1])
            if target.scheme or target.netloc:
                continue
            destination = Path(os.path.abspath(path.parent / unquote(target.path))) if target.path else path
            checked += 1
            try:
                relative = destination.relative_to(ROOT)
            except ValueError:
                errors.append(f"{name}: link escapes repository: {link[1]}")
                continue
            # Explicit ignored evidence locators can be absent from a fresh tree.
            if relative.parts and relative.parts[0] in {"generated", ".scryfall-cache"}:
                evidence += 1
                continue
            if not destination.exists():
                errors.append(f"{name}: missing target {link[1]}")
            elif target.fragment and destination in fragments and unquote(target.fragment) not in fragments[destination]:
                errors.append(f"{name}: missing anchor {link[1]}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"{len(docs)} Markdown files; {checked} local links checked ({evidence} ignored evidence locators excluded).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
