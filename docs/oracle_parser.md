# Oracle text parser

The parser is a hand-written, zero-runtime-dependency TypeScript package at
`packages/oracle-parser`. It normalizes Oracle text, tokenizes it, and builds a
structured AST. The ANTLR implementation was retired on 2026-08-01.

The parser remains a limited supported subset. The deck-builder is active and
standalone; it does not consume this AST. A rules engine and IR emitter are not
implemented. See [architecture.md](architecture.md) and the package
[AGENTS.md](../packages/oracle-parser/AGENTS.md).

## Pipeline and public API

```text
Oracle text → normalize → ability blocks → lex → parse → typed AST
                                                       ↓ future
                                              validate/lower → Card IR
```

```typescript
import { parseOracleText } from "@mtg-ecosystem/oracle-parser";

const result = parseOracleText("Flying\n{T}: Add {G}.");
if (result.ok) {
  // result.abilities is required here: KeywordLine, ActivatedAbility.
  console.log(result.abilities);
} else {
  for (const line of result.lines) {
    if (!line.ok) console.error(line.text, line.error);
  }
}
```

`src/ast.ts` is the authoritative current contract. `ParsedLine` and
`ParseCardResult` are discriminated success/failure unions. A failed card has
no aggregate `abilities`; successfully parsed individual blocks remain
available in `lines` for diagnostics and coverage work.

`ok: true` means every normalized ability block was consumed by supported
productions and represented in the AST. It is not proof of semantic correctness
or a promise that a downstream engine implements the represented mechanics.
Unknown syntax must fail rather than be preserved as raw executable text.
Acceptance statistics cannot detect an incorrect AST; structural tests and
rules review are the correctness gate.

## Responsibilities and invariants

- **Normalization** canonicalizes punctuation, removes parenthetical reminder
  text, replaces exact-case full/short self-references, and preserves newlines.
  Input is assumed to be Oracle text. `lines[].text` is normalized text, not an
  original-source span. Nested parentheticals and unusual name references
  remain limitations; unsupported remaining text fails lexing/parsing.
- **Ability blocks** ordinarily follow lines. Modal headers own all immediately
  following bullet lines. A successful header must actually consume those
  options. One failed option fails the entire block; orphaned bullets fail.
- **Lexing** never drops unknown characters. Brace tokens remain generic game
  symbols; costs, mana production, and payment conditions validate their own
  supported symbols.
- **Parsing** constructs nodes directly. No generated grammar or visitor exists.
  A cursor parser returning `null` restores its starting position. Effect
  parsers also restore elided-subject context when an attempt fails.
- **Diagnostics** come from the original classifier attempts. Sliced cost,
  loyalty, and ability-word cursors report offsets in the complete header;
  failed modal options report their option number and local token position.
  Backtracking expectations can still be noisy; token positions are not
  original-text character offsets.

## Semantic conventions

The foundational distinctions below are represented explicitly and covered by
regression tests. These are intentional breaking changes to the experimental
AST; no compatibility adapters are provided.

| Construct | Representation |
| --- | --- |
| `artifact creature` | `types: ["artifact", "creature"]`: both required |
| `artifact or creature` | `allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }]` |
| `red and green` / `red or green` | Conjunctive color array / explicit alternatives |
| Single-occurrence / grouped trigger | Default per occurrence / `grouping: "one-or-more"` |
| Graveyard event | `from` zone or `"anywhere"`, plus destination `ZoneRef` |
| Search | Acting player and complete searched `ZoneRef`, including owner |
| Return to battlefield | Optional explicit `controller: "you" | "owner"` |
| Return to hand | Explicit `to: "your" | "owner"` |
| Activation restriction | Typed `sorcery` or `once-each-turn`, including loyalty abilities |
| Coordinated pump and ability gain | Shared trailing duration on both effects |

Filter fields and ordinary array entries are conjunctive; only `anyOf` is
disjunctive. `allOf` combines nested clauses. The existing `orPlayer` field
represents a simple object-or-player union. Mixed color conjunctions without
an implemented unambiguous grouping are rejected.

Duration sharing is limited to supported coordinated modifiers with an elided
subject. It does not cross a `then` or sentence boundary. Other unsupported
wording must fail rather than invent scope.

The AST retains symbolic references such as `it`, `that-player`, `x`, and
keyword names. Binding those references, expanding predefined game concepts,
and deciding execution support belong to a future semantic validation/lowering
stage. That stage must reject unresolved or unsupported nodes before producing
executable Card IR. This package does not yet provide it.

Rules references: [Comprehensive Rules](https://magic.wizards.com/en/rules),
especially 107.4 (symbols), 602.5 (restrictions), 603.2c and 700.1 (events),
and 611.2a (durations). Parser comments cite the rules edition used for the
foundational fixes.

## Development and verification

Use Node ≥ 23 and run `npm install` at the repository root. There is no Java
requirement or emitted build. From `packages/oracle-parser`:

```sh
npm run check
```

This runs TypeScript checking and Node's structural test suite, including
parser rollback, meaning-distinction tests, complete-block failures, corpus
reporting, and simulated interrupted downloads. Pipeline modules imported by
tests are typechecked too. Tests do not require networking or the bulk cache.

From the root:

```sh
npm run ingest
npm run validate
npm run validate -- --json > .scryfall-cache/validation.json
```

Validation uses the existing cache without refreshing it. Reports identify the
exact corpus by SHA-256, the Git revision, and a hash of parser and reporting
sources including working changes. Per-card outcomes include successful AST
hashes for comparing runs. Use the same corpus hash and scope when comparing;
changed AST hashes need review and are not automatically regressions.

The versioned scope excludes digital-only, silver-border, acorn, and explicitly
listed non-card layouts. Other unsupported cards remain failures. Exclusions
are counted separately. Identity uses `oracle_id` (face IDs for reversible
cards), not names or printing IDs. Empty rules text is valid; missing or null face text
is reported as a failure. “Lines” count ability blocks, including grouped modes.

The old rewrite baseline was 33% of cards / 53% of lines under a different
population definition. It is historical, not a minimum acceptance target.
Correcting false successes can lower coverage. Failure groups use the token at
the failure location and retain complete example text; they are investigation
hints, not proof that every example needs the same grammar extension.

## Next milestones

1. Keep all known meaning-loss regressions fixed or explicitly rejected. Review
   newly accepted ASTs against card data and the rules.
2. Expand supported constructs with full structural assertions and negative
   tests; pair near-identical inputs when a word changes the meaning.
3. Design reference binding, semantic validation, predefined token handling,
   and the capability boundary with a first engine consumer.
4. Implement an emitter only after a small, audited subset can pass that
   validation end to end. Corpus coverage alone is not an emission gate.

`npm run build-ir` deliberately fails. The proposed envelope and remaining
storage decisions are described in [data_schemas.md](data_schemas.md).
