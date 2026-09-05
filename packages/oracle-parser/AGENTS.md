# 🔮 oracle-parser — Layer 0: Oracle Text Parser & Card IR

Parse every Magic card's oracle text into a typed AST, then (next milestone)
compile it to per-set JSON IR that Layer 1 (the rules engine) will consume.
See [docs/oracle_parser.md](../../docs/oracle_parser.md) for the design and
[docs/architecture.md](../../docs/architecture.md) for where it sits.

**Stack**: hand-written, zero-dependency TypeScript. No ANTLR, no Java, no
build step — Node ≥ 23 runs the `.ts` sources directly (imports use explicit
`.ts` extensions; no TS constructor parameter properties — Node strip-only
mode rejects them). The previous ANTLR/antlr4ts pipeline was audited and
replaced on 2026-08-01; see the git history if you need the archaeology.

## 🧠 Architecture

```
oracle text ──normalize──▶ lines ──lex──▶ tokens ──parse──▶ typed AST
```

| Module | Job |
|---|---|
| `src/normalize.ts` | Strip reminder text, `~`-ify self-references (exact-case, word-bounded), canonicalize unicode, **preserve line boundaries** (one line = one ability). |
| `src/lexer.ts` | Tokens: words (possessive-flagged, contractions kept whole), numbers, `{…}` symbols, punctuation. Unknown characters throw — never silently dropped. |
| `src/symbols.ts` | Shared mana-symbol validation for costs, production, and payment conditions. |
| `src/vocab.ts` | Closed word classes (card types, zones, colors…) + singularization. |
| `src/ast.ts` | The typed AST. Discriminated unions, no `any`. This is the contract Layer 1 consumes — treat changes as API changes. |
| `src/parser/cursor.ts` | Backtracking token cursor; tracks farthest failure for diagnostics. |
| `src/parser/refs.ts` | Shared grammar: filters, object/player refs, amounts, zones, durations, conditions, counters. |
| `src/parser/keywords.ts` | Table-driven keyword abilities. The `KEYWORDS` table is the single source of truth; unknown keywords fail the line. |
| `src/parser/costs.ts` | Activation and keyword-parameter costs. |
| `src/parser/triggers.ts` | "When/Whenever/At …" clauses. |
| `src/parser/effects.ts` | Imperative resolution text (the largest surface). |
| `src/parser/statics.ts` | Continuous effects: anthems, CDAs, enters-tapped, can't. |
| `src/parser/index.ts` | Line classification, modal bullet grouping, card-level API. |

Parse results are per-line: `parseOracleText(text, name)` returns `ok` plus a
`lines[]` array where each failed line carries a diagnostic pointing at the
farthest failure in the original parse attempts. Sliced classifier cursors retain
header offsets; modal failures identify their option. Success/failure results
are discriminated unions. `ok` is complete supported parsing, not a certificate
of engine execution support. See the design doc for symbolic reference limits.

## 🚀 Commands

From this directory:

*   `npm test` — `node --test`, structural AST assertions.
*   `npm run build` — `tsc --noEmit` typecheck (nothing is emitted; consumers
    import the TS sources directly).
*   `npm run check` — both.

From the repo root:

*   `npm run ingest` — cache Scryfall bulk data to `.scryfall-cache/`.
*   `npm run validate` — acceptance scoreboard for the cached corpus. Reports
    corpus/source hashes, Git revision, a versioned scope and exclusions,
    card/block acceptance, and failure-location groups with complete examples.
    `npm run validate -- --json` includes per-card AST hashes for comparison.
    Empty text is valid; missing face text fails. The historical 33% cards /
    53% lines baseline used a different denominator and is not a target.
*   `npm run build-ir` — deliberately fails. Semantic validation and emission
    are future work; acceptance percentage alone does not justify emission.

## 🔄 Development loop: correctness before coverage

1. Fix known false successes before expanding coverage. Use the largest
   in-scope failure groups as investigation hints once correctness checks pass.
2. Read the relevant Comprehensive Rules and check card facts in data. Cite the
   source in code; choose an AST shape that preserves all meaningful clauses.
3. Extend the relevant module and `src/ast.ts`, or reject constructs that cannot
   yet be represented. Prefer clear breaking changes to compatibility shims.
4. Add full structural tests and negative cases. Pair similar wording when it
   changes logic, event grouping, ownership, restrictions, or effect scope.
5. Run `npm run check`, then `npm run validate`. Inspect changed acceptance and
   ASTs using the same corpus hash and scope. Correctness fixes may lower
   coverage; never retain a false success to protect a percentage.

The check includes ingestion/reporting tests through imports from `test/`.
These use temporary directories and simulated network responses, not the local
bulk cache. Generated reports and bulk data must stay ignored.

## ⚠️ Constraints

*   **A wrong parse is worse than a failed parse.** Parsers return complete
    typed nodes or `null` — never a lossy approximation. Unknown keywords,
    unrepresentable clauses, and unlexable characters all fail loudly.
*   **No `any` in the AST.** If you can't type it, you don't understand the
    mechanic yet — go back to the rules.
*   **Every parse function backtracks cleanly.** Return `null` ⇒ cursor
    position restored (use `Cursor.attempt`); restore effect context too.
*   **Line = ability.** Never flatten newlines out of oracle text; modal
    bullets are grouped with their header by `parser/index.ts`.
*   **Clean design beats backward compatibility.** This AST is early; prefer
    reshaping a bad node over layering variants on top of it.
