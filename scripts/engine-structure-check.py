#!/usr/bin/env python3
"""Cheap dependency-direction and Git-index artifact guards; never invokes Cargo."""
import argparse
from pathlib import Path
import re
import subprocess
import sys
import tomllib

CRATES = 'packages/game-engine/crates'
# Deliberate dependency review boundary for deterministic layers, including
# dev-dependencies: compiler-backed witnesses live in the dev-only mtg-witness
# crate, so mtg-core test builds never compile or wait on mtg-compiler.
ALLOWED = {
    'mtg-contracts': {'serde', 'serde_json', 'schemars', 'num-bigint', 'num-traits', 'sha2'},
    'mtg-oracle-ast': {'serde', 'serde_json'},
    'mtg-compiler': {'mtg-contracts', 'mtg-oracle-ast', 'serde', 'serde_json', 'sha2', 'regex', 'serde_path_to_error'},
    'mtg-view': {'mtg-contracts', 'serde', 'schemars'},
    'mtg-observe': {'mtg-view', 'mtg-contracts', 'serde_json', 'sha2'},
    'mtg-core': {'mtg-view', 'mtg-contracts', 'serde', 'serde_json', 'schemars', 'rand_chacha', 'rand_core'},
}
# Test-only additions per layer: the published Catalog schema is checked with
# the host's already-pinned validator, never linked into a runtime layer.
DEV_ALLOWED = {'mtg-contracts': {'jsonschema'}}


def dependency_errors(root):
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())['workspace'].get('dependencies', {})
    errors = []
    for crate, allowed in ALLOWED.items():
        path = root / CRATES / crate.removeprefix('mtg-') / 'Cargo.toml'
        manifest = tomllib.loads(path.read_text())
        tables = [('all targets', manifest), *manifest.get('target', {}).items()]
        runtime = set()
        for target, table in tables:
            for kind in ('dependencies', 'build-dependencies', 'dev-dependencies'):
                permit = allowed | ({'sha2'} if kind == 'build-dependencies' and crate in {'mtg-core', 'mtg-compiler'} else set())
                permit |= DEV_ALLOWED.get(crate, set()) if kind == 'dev-dependencies' else set()
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
                    if target == 'all targets' and kind == 'dependencies':
                        runtime.add(name)
                    if name not in permit:
                        errors.append(f'{path.relative_to(root)}: {target} {kind} {alias} ({name}) crosses the reviewed dependency boundary; keep host/parser/compiler I/O outside deterministic layers or explicitly review this allowlist')
        if crate == 'mtg-observe' and allowed - runtime:
            missing = ', '.join(sorted(allowed - runtime))
            errors.append(f'{path.relative_to(root)}: missing reviewed runtime dependencies: {missing}')
    return errors


# Only these test crates may enable mtg-core's `testkit` harness feature or
# depend on the dev-only witness crate.
TESTKIT_CRATES = {'mtg-witness'}


def testkit_errors(root):
    """`testkit` exposes engine internals to external test binaries; production
    crates and mtg-core defaults never enable it, and nothing depends on
    mtg-witness. Build identity excludes testkit/witness sources on that basis."""
    errors = []
    members = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['members']
    for member in members:
        path = root / member / 'Cargo.toml'
        manifest = tomllib.loads(path.read_text())
        name = manifest['package']['name']
        rel = path.relative_to(root)
        if name == 'mtg-core' and 'testkit' in manifest.get('features', {}).get('default', []):
            errors.append(f'{rel}: testkit must never be a default feature')
        if name in TESTKIT_CRATES:
            continue
        for feature, enables in manifest.get('features', {}).items():
            if any(e.split('/')[-1] == 'testkit' and e.split('/')[0].rstrip('?') in ('mtg-core', 'dep:mtg-core') for e in enables if '/' in e):
                errors.append(f'{rel}: feature {feature} enables mtg-core/testkit; only {sorted(TESTKIT_CRATES)} may')
        tables = [manifest, *manifest.get('target', {}).values()]
        for table in tables:
            for kind in ('dependencies', 'dev-dependencies', 'build-dependencies'):
                for alias, value in table.get(kind, {}).items():
                    spec = value if isinstance(value, dict) else {}
                    package = spec.get('package', alias)
                    if package == 'mtg-witness':
                        errors.append(f'{rel}: {kind} depends on dev-only mtg-witness')
                    if package == 'mtg-core' and 'testkit' in spec.get('features', []):
                        errors.append(f'{rel}: {kind} enables mtg-core/testkit; only {sorted(TESTKIT_CRATES)} may')
    return errors


MOD_DECL = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;', re.M)
PATH_ATTR = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')


def test_only(name):
    """Mirror of `test_only_source` in contracts `build_input.rs`, per path component."""
    stem = name.removesuffix('.rs')
    return stem == 'tests' or stem.endswith('_tests') or stem == 'testkit'


def test_module_errors(root):
    """Engine build identity omits test-named sources (`build_input.rs`); every
    such module must therefore be `#[cfg(test)]` (a `testkit` module
    `#[cfg(feature = "testkit")]`) or declared inside one."""
    errors = []
    for path in sorted((root / CRATES).rglob('*.rs')):
        rel = path.relative_to(root)
        parts = rel.parts
        if any(p in {'target', 'generated', 'tests', 'examples', 'benches'} for p in parts) or any(test_only(p) for p in parts):
            continue
        text = path.read_text()
        for match in MOD_DECL.finditer(text):
            # Attributes directly above the declaration (contiguous lines).
            lines = text[:match.start()].rstrip('\n').split('\n')
            attributes = []
            while lines and lines[-1].strip().startswith('#['):
                attributes.append(lines.pop().strip())
            target = match.group(1) + '.rs'
            for attribute in attributes:
                found = PATH_ATTR.search(attribute)
                if found:
                    target = found.group(1).split('/')[-1]
            if target.removesuffix('.rs') == 'testkit':
                if not any(re.fullmatch(r'#\[cfg\(feature\s*=\s*"testkit"\)\]', a) for a in attributes):
                    line = text[:match.start()].count('\n') + 1
                    errors.append(f'{rel}:{line}: testkit module must be #[cfg(feature = "testkit")]: engine build identity excludes it')
            elif test_only(target) and not any(re.search(r'cfg\((all\()?test\b', a) for a in attributes):
                line = text[:match.start()].count('\n') + 1
                errors.append(f'{rel}:{line}: test-named module {target} must be #[cfg(test)]: engine build identity excludes test-only sources')
    return errors


def rust_code(text):
    # The pinned sources use ordinary literals and hash-delimited raw strings.
    return re.sub(r'//[^\n]*|/\*.*?\*/|r(#+)".*?"\1|"(?:\\.|[^"\\])*"', ' ', text, flags=re.S)


def unsafe_errors(root):
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']
    errors = []
    if workspace.get('lints', {}).get('rust', {}).get('unsafe_code') != 'forbid':
        errors.append('workspace must forbid unsafe_code')
    for member in workspace['members']:
        directory = root / member
        lints = tomllib.loads((directory / 'Cargo.toml').read_text()).get('lints', {})
        if member != f'{CRATES}/env':
            if not lints.get('workspace'):
                errors.append(f'{member} must inherit workspace lints')
            continue
        if lints.get('rust', {}).get('unsafe_code') != 'deny':
            errors.append('mtg-env must deny unsafe_code')
        for path in directory.rglob('*.rs'):
            if path == directory / 'src/ffi.rs':
                continue
            code = rust_code(path.read_text())
            if re.search(r'\bunsafe\b|\b(?:allow|expect|warn)\s*\([^)]*\bunsafe_code\b', code):
                errors.append(f'{path.relative_to(root)}: unsafe code is confined to env/src/ffi.rs')
    return errors


def decision_validator_errors(root):
    """Decision validators can depend on shape, never on descriptive reason."""
    engine = root / CRATES / 'core/src/engine'
    errors = []
    view = root / CRATES / 'view/src'
    sources = [(engine, path) for path in engine.rglob('*.rs')]
    sources.extend((view, path) for path in view.glob('check.rs'))
    for base, source in sorted(sources):
        parts = source.relative_to(base).with_suffix('').parts
        validates = parts == ('api', 'decision', 'selected') or any(
            p in {'check', 'validate', 'validation', 'validator', 'validators'}
            or p.startswith('validate_') or p.endswith(('_validator', '_validation')) for p in parts
        )
        if not validates:
            continue
        code = rust_code(source.read_text())
        if re.search(r'\bReason\b|\.\s*reason\b|\b(?:match|matches!\s*\()\s*&?\s*reason\b', code):
            errors.append(f'{source.relative_to(root)}: decision validation may only depend on Shape')
    return errors


def testkit_private_fields(src):
    """Fields made public only by core's testkit_fields! macro."""
    fields = set()
    for source in src.rglob('*.rs'):
        text = source.read_text()
        if 'testkit_fields!' not in text:
            continue
        for body in re.findall(r'\btestkit_fields!\s*\{[^{}]*?\bpub\s+struct\b[^{}]*\{([^{}]*)\}', rust_code(text), re.S):
            fields.update(re.findall(r'(?:^|[,\n])\s*(?:pub\s*\([^)]*\)\s*)?(\w+)\s*:(?!:)', body))
    return fields


def public_witness_errors(core):
    tests = core / 'tests'
    private = testkit_private_fields(core / 'src')
    # Only these named roots intentionally compile white-box witnesses.
    white_box = {'rules/rules_kernel', 'rules/trigger_kernel', 'recovery/search_work', 'recovery/state_recovery'}
    roots = {'rules/main.rs', 'recovery/main.rs', 'recovery/corpus.rs'}
    errors = []
    for source in sorted(tests.rglob('*.rs')):
        rel = source.relative_to(tests)
        is_adversarial = any('adversarial' in part for part in rel.parts)
        if str(rel) in roots or (not is_adversarial and '/'.join(rel.parts[:2]) in white_box):
            continue
        # Strip comments and string literals so fixture data and examples do
        # not become imports or field reads. Public accessor calls are legal.
        code = rust_code(source.read_text())
        forbidden = re.search(
            r'\btestkit\b|\bsuper\s*::|\bcrate\s*::\s*(?!support\b)\S'
            r'|\b(?:super|crate)\s+as\b|\b(?:rules_kernel|trigger_kernel|search_work|state_recovery)\s*::'
            r'|#\s*\[\s*path\b|\binclude\s*!', code)
        accessed = {m[1] for m in re.finditer(r'(?<!\.)\.\s*(\w+)\b(?!\s*\()', code)} & private
        if forbidden or accessed:
            detail = ', '.join(sorted(accessed)) if accessed else forbidden[0]
            errors.append(f'{rel}: public-API witness uses a private field or unpermitted module: {detail}')
    return errors


def topology_errors(root):
    """Integration targets stay together and adversarial tests use the public API."""
    errors = []
    for directory, expected in [
        (f'{CRATES}/core', {'rules', 'recovery'}),
        (f'{CRATES}/witness', {'cards'}),
        ('packages/game-server', {'host'}),
    ]:
        package = root / directory
        actual = {p.stem for p in (package / 'tests').glob('*.rs')}
        actual.update(p.parent.name for p in (package / 'tests').glob('*/main.rs'))
        if actual != expected:
            errors.append(f'{directory}/tests: expected {sorted(expected)}, found {sorted(actual)}')
    errors.extend(public_witness_errors(root / CRATES / 'core'))
    sim = root / CRATES / 'sim/src'
    library = (sim / 'lib.rs').read_text()
    for name in ('play', 'policy', 'pods', 'analytics', 'search', 'mana_spend'):
        if not re.search(rf'^pub mod {name}\s*[;{{]', library, re.M):
            errors.append(f'{sim.relative_to(root)}/lib.rs: missing simulator module {name}')
    if len((sim / 'main.rs').read_text().splitlines()) > 300:
        errors.append(f'{sim.relative_to(root)}/main.rs: CLI exceeds 300 lines')
    return errors


def artifact_reason(path, size, prefix):
    parts = Path(path).parts
    sdk_output = (
        path.startswith(('packages/game-server/sdk/dist/', 'packages/game-server/sdk/generated/'))
        or path == 'packages/game/generated/sdk-identity.ts'
    )
    blocked_parts = {'target', 'node_modules', '.scryfall-cache', '__pycache__'}
    if not sdk_output:
        blocked_parts.update(('dist', 'generated'))
    if blocked_parts.intersection(parts):
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
    errors = unsafe_errors(args.root) + dependency_errors(args.root) + testkit_errors(args.root) + test_module_errors(args.root) + decision_validator_errors(args.root) + topology_errors(args.root) + artifact_errors(args.root)
    for error in errors:
        print(error, file=sys.stderr)
    if not errors:
        print('Engine structure: dependency boundaries, testkit confinement, test-only modules and staged artifacts passed')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())
