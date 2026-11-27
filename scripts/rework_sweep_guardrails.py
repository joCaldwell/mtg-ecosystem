"""Read-only Rust source metrics for the rework sweep.

The counts measure declarations and legacy routing sites, not semantic
equivalence. Keep names in the receipt so a change can be reviewed in source.
"""
from collections import defaultdict
import hashlib
from pathlib import Path
import re


# Ignore comments and literal contents when looking for Rust declarations and braces.
LEXICAL_NOISE = re.compile(
    r'(?P<comment>//[^\n]*|/\*)|(?P<raw>(?:br|cr|r)(?P<hashes>\#*)".*?"(?P=hashes))'
    r'|(?P<string>"(?:\\.|[^"\\])*")|(?P<char>\'(?:\\.|[^\'\\\n])\')', re.S,
)


def rust_source(text):
    """Mask comments and literals without moving source offsets."""
    code = []
    strings = []
    cursor = 0
    while match := LEXICAL_NOISE.search(text, cursor):
        code.append(text[cursor:match.start()])
        end = match.end()
        if match['comment'] == '/*':
            depth = 1
            while depth:
                marker = re.search(r'/\*|\*/', text[end:])
                if not marker:
                    raise ValueError('unterminated Rust block comment')
                depth += 1 if marker[0] == '/*' else -1
                end += marker.end()
        if match['raw'] or match['string']:
            strings.append((match.start(), match[0]))
        code.append(re.sub(r'[^\n]', ' ', text[match.start():end]))
        cursor = end
    code.append(text[cursor:])
    return ''.join(code), strings


def body(source, start):
    """Return the contents of a brace-delimited body after a declaration."""
    opening = source.index('{', start)
    depth = 0
    for delimiter in re.finditer(r'[{}]', source[opening:]):
        depth += 1 if delimiter[0] == '{' else -1
        if depth == 0:
            return source[opening + 1:opening + delimiter.start()]
    raise ValueError('unterminated Rust body')


def variants(contents):
    """Read the initial identifier of each top-level enum member."""
    start = depth = 0
    for token in re.finditer(r'[][(){},]', contents + ','):
        if token[0] == ',' and depth == 0:
            member = re.sub(r'#\[[^\]]*\]', '', contents[start:token.start()]).strip()
            if match := re.match(r'\w+', member):
                yield match[0]
            start = token.end()
        elif token[0] in '[({':
            depth += 1
        elif token[0] in '])}':
            depth -= 1


def one_enum(enums, name, optional=False):
    matches = enums[name]
    if optional and not matches:
        return []
    if len(matches) != 1:
        raise ValueError(f'expected one {name} enum, found {len(matches)}')
    return matches[0][1]


def guardrails(crates):
    crates = Path(crates)
    sources = {}
    enums = defaultdict(list)
    mirror_sites = []
    fingerprint = hashlib.sha256()
    enum_names = {'Cell', 'KeywordAbility', 'BuiltinId', 'Verb', 'DelayedEvent',
                  'ProposedEvent', 'EventOutcome', 'Reason'}
    for path in sorted(crates.glob('*/src/**/*.rs')):
        name = path.relative_to(crates).as_posix()
        raw = path.read_bytes()
        fingerprint.update(name.encode() + b'\0' + hashlib.sha256(raw).digest())
        code, strings = rust_source(raw.decode())
        sources[name] = code
        for match in re.finditer(r'\benum\s+(\w+)', code):
            enum = match[1]
            if enum in enum_names or (name.startswith('core/src/engine/kir/') and enum.endswith('Stage')):
                enums[enum].append((name, list(variants(body(code, match.end())))))
        if name.startswith('compiler/src/'):
            mirror_sites.extend(f'{name}:{code.count(chr(10), 0, offset) + 1}'
                                for offset, literal in strings if 'shape outside the lowering mirror' in literal)

    declared_cells = one_enum(enums, 'Cell')
    check_sources = '\n'.join(code for name, code in sources.items()
                              if name.startswith('contracts/src/kir/check/'))
    cells = sorted(set(declared_cells) & set(re.findall(r'\bCell::(\w+)', check_sources)))
    if not cells:
        raise ValueError('could not find audited cell references in kir::check')

    kernel = sources.get('core/src/engine/kir/mod.rs', '')
    dispatch = re.search(r'\bfn execute_kernel\s*\(', kernel)
    routes = []
    if dispatch:
        # A pair is one distinct downstream executor reached directly by the
        # ladder that re-recognizes the forwarded Instruction, either through
        # a contract helper or by pattern matching the shape again.
        callees = set(re.findall(r'\bself\s*\.\s*(execute_\w+)\(', body(kernel, dispatch.end())))
        contracts = sources['contracts/src/lib.rs']
        instruction = re.search(r'\bimpl Instruction\s*\{', contracts)
        recognizers = set(re.findall(r'\bfn (\w+)\(&self\)\s*->\s*(?:Option<|bool\b)',
                                     body(contracts, instruction.start())))
        for name, code in sources.items():
            if not name.startswith('core/src/engine/'):
                continue
            specs = {match[1]: body(code, match.end())
                     for match in re.finditer(r'\bfn (\w+_spec)\s*\(', code)}
            for match in re.finditer(r'\bfn (execute_\w+)\s*\(', code):
                if match[1] not in callees:
                    continue
                executor = body(code, match.end())
                signature = code[match.end():code.index('{', match.end())]
                # Some executors re-read a frame through a local shape
                # spec before invoking the same contract recognizer.
                spec_calls = set(re.findall(r'\bself\s*\.\s*(\w+_spec)\(', executor))
                inspected = executor + '\n' + '\n'.join(specs[spec] for spec in spec_calls & specs.keys())
                helpers = sorted(set(re.findall(r'\.(\w+)\(\)', inspected)) & recognizers)
                pattern = bool(re.search(r'\b(?:kernel!|Instruction::Kernel)', executor))
                if (helpers or pattern) and ('Instruction' in signature or 'set_damage' in helpers):
                    routes.append({'executor': match[1], 'source': name, 'helpers': helpers,
                                   'matches_instruction_shape': pattern})

    # Named keyword families come from the Rust vocabularies. Include named
    # actions, counters and procedures outside those enums. The full variant
    # inventory also shows additions before their mechanic name is recognized.
    mechanics = set(one_enum(enums, 'KeywordAbility') + one_enum(enums, 'BuiltinId', optional=True))
    mechanics.update(('Dash', 'Dashed', 'Encore', 'Unearth', 'Myriad', 'Saga', 'Lore', 'Stun', 'Populate',
                      'Transform', 'Transformed', 'Scry', 'Scried', 'Surveil', 'Surveilled',
                      'Hybrid', 'Mulligan', 'Monarch', 'Initiative', 'Regenerate', 'Regenerated'))
    groups = {}
    for enum in ('Verb', 'DelayedEvent', 'ProposedEvent', 'EventOutcome', 'Reason'):
        groups[enum] = one_enum(enums, enum, optional=enum in {'DelayedEvent', 'ProposedEvent', 'EventOutcome'})
    groups['interpreter stages'] = [f'{name}::{enum}::{variant}'
                                    for enum, declarations in enums.items() if enum.endswith('Stage')
                                    for name, members in declarations for variant in members]

    def words(identifier):
        return ' '.join(re.findall(r'[A-Z][a-z]*|[a-z]+', identifier))

    mechanic_pattern = re.compile(r'\b(?:' + '|'.join(re.escape(words(m)) for m in sorted(mechanics)) + r')\b')

    names = {group: sorted(variant for variant in members if mechanic_pattern.search(words(variant.rsplit('::', 1)[-1])))
             for group, members in groups.items()}
    return {
        'source_sha256': fingerprint.hexdigest(),
        'kernel_cells': {'count': len(cells), 'names': cells, 'declared_total': len(declared_cells),
                         'declared_without_check_reference': sorted(set(declared_cells) - set(cells))},
        'mirrored_shape_guards': {'count': len(mirror_sites), 'sites': mirror_sites},
        'routing_ladder_pairs': {'count': len(routes), 'pairs': sorted(routes, key=lambda row: row['executor'])},
        'mechanic_named_variants': {group: {'count': len(members), 'names': members,
                                           'all_variants': sorted(groups[group])}
                                    for group, members in names.items()},
    }
