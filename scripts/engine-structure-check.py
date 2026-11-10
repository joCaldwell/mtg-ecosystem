#!/usr/bin/env python3
"""Cheap dependency-direction and Git-index artifact guards; never invokes Cargo."""
import argparse
from pathlib import Path
import subprocess
import sys
import tomllib

CRATES = 'packages/game-engine/crates'
# Deliberate dependency review boundary for deterministic layers. Dev-only
# compiler adapters in core are legitimate and explicitly separate below.
ALLOWED = {
    'mtg-contracts': {'serde', 'serde_json', 'schemars', 'num-bigint', 'num-traits', 'sha2'},
    'mtg-compiler': {'mtg-contracts', 'serde', 'serde_json', 'sha2', 'regex', 'serde_path_to_error'},
    'mtg-core': {'mtg-contracts', 'serde', 'serde_json', 'schemars', 'rand_chacha', 'rand_core'},
}


def dependency_errors(root):
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())['workspace'].get('dependencies', {})
    errors = []
    for crate, allowed in ALLOWED.items():
        path = root / CRATES / crate.removeprefix('mtg-') / 'Cargo.toml'
        manifest = tomllib.loads(path.read_text())
        tables = [('all targets', manifest), *manifest.get('target', {}).items()]
        for target, table in tables:
            for kind in ('dependencies', 'build-dependencies', 'dev-dependencies'):
                permit = allowed | ({'sha2'} if kind == 'build-dependencies' else set())
                if crate == 'mtg-core' and kind == 'dev-dependencies':
                    permit |= {'mtg-compiler'}
                for alias, value in table.get(kind, {}).items():
                    spec = value if isinstance(value, dict) else {}
                    base = workspace.get(alias, {}) if spec.get('workspace') else spec
                    base = base if isinstance(base, dict) else {}
                    name = base.get('package', alias)
                    # A path dependency cannot conceal an upper layer with an alias.
                    if 'path' in base:
                        parent = root if spec.get('workspace') else path.parent
                        dep_manifest = parent / base['path'] / 'Cargo.toml'
                        name = tomllib.loads(dep_manifest.read_text())['package']['name']
                    if name not in permit:
                        errors.append(f'{path.relative_to(root)}: {target} {kind} {alias} ({name}) crosses the reviewed dependency boundary; keep host/parser/compiler I/O outside deterministic layers or explicitly review this allowlist')
    return errors


def artifact_reason(path, size, prefix):
    parts = Path(path).parts
    if any(p in {'target', 'node_modules', 'generated', '.scryfall-cache', 'dist', '__pycache__'} for p in parts):
        return 'generated/cache directory'
    if path.startswith('packages/card-data/sets/') and path.endswith('.json'):
        return 'downloaded/compiled card corpus'
    if path.startswith('packages/deck-builder/') and any(p in {'data', '.dev'} for p in parts):
        return 'local deck-builder database/output'
    if prefix.startswith((b'\x7fELF', b'\x00asm', b'SQLite format 3\x00')):
        return 'native/Wasm executable or SQLite database'
    return None


def artifact_errors(root):
    rows = subprocess.check_output(['git', '-C', str(root), 'ls-files', '--stage', '-z']).split(b'\0')
    errors = []
    # Inspect the index, so force-added ignored output and partially staged files
    # are checked exactly as they would enter a commit.
    with subprocess.Popen(['git', '-C', str(root), 'cat-file', '--batch'], stdin=subprocess.PIPE, stdout=subprocess.PIPE) as cat:
        for row in filter(None, rows):
            header, raw_path = row.split(b'\t', 1)
            mode, oid, stage = header.split()
            path = raw_path.decode('utf-8', 'surrogateescape')
            if stage != b'0':
                errors.append(f'{path}: unresolved index conflict')
                continue
            if mode == b'160000':
                continue
            cat.stdin.write(oid + b'\n'); cat.stdin.flush()
            size = int(cat.stdout.readline().split()[2])
            prefix = cat.stdout.read(min(size, 32))
            remaining = size - len(prefix)
            while remaining:
                remaining -= len(cat.stdout.read(min(remaining, 65536)))
            cat.stdout.read(1)
            reason = artifact_reason(path, size, prefix)
            if reason:
                errors.append(f'{path} ({size:,} bytes): {reason}; unstage it and retain it in ignored/external storage (do not delete source evidence)')
        cat.stdin.close()
        cat.wait()
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    errors = dependency_errors(args.root) + artifact_errors(args.root)
    for error in errors:
        print(error, file=sys.stderr)
    if not errors:
        print('Engine structure: dependency boundaries and staged artifacts passed')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())
