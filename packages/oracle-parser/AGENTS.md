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
farthest token any parse path reached.

## 🚀 Commands

From this directory:

*   `npm test` — `node --test`, structural AST assertions.
*   `npm run build` — `tsc --noEmit` typecheck (nothing is emitted; consumers
    import the TS sources directly).
*   `npm run check` — both.

From the repo root:

*   `npm run ingest` — cache Scryfall bulk data to `.scryfall-cache/`.
*   `npm run validate` — **the scoreboard**: run the parser over all ~34k
    unique cards; reports card-level and line-level coverage plus the largest
    unparsed template groups. Baseline at rewrite time: **33% cards / 53%
    lines** (the old ANTLR recognizer claimed 16% but produced no AST and
    silently dropped unlexable characters).
*   `npm run build-ir` — per-set IR emission. Intentionally unimplemented
    until coverage justifies it; fails loudly.

## 🔄 Development loop: raising coverage

1.  `npm run validate` — work the **largest** unparsed-template group, not
    the most interesting one.
2.  Read the Comprehensive Rules for the mechanic first. The AST shape must
    reflect how the rule actually works, because Layer 1 has to execute it.
3.  Extend the right module (usually one new `StepParser` in `effects.ts`, a
    keyword-table row, or a trigger branch) and, if needed, the AST in
    `ast.ts`.
4.  Add a **structural** test asserting the AST content — a test that only
    checks "it parsed" is worthless (that mistake is why the old suite passed
    while the parser produced nothing).
5.  `npm run check`, then `npm run validate` — confirm the number went up and
    the tests still pass.

## ⚠️ Constraints

*   **A wrong parse is worse than a failed parse.** Parsers return complete
    typed nodes or `null` — never a lossy approximation. Unknown keywords,
    unrepresentable clauses, and unlexable characters all fail loudly.
*   **No `any` in the AST.** If you can't type it, you don't understand the
    mechanic yet — go back to the rules.
*   **Every parse function backtracks cleanly.** Return `null` ⇒ cursor
    position restored (use `Cursor.attempt`).
*   **Line = ability.** Never flatten newlines out of oracle text; modal
    bullets are grouped with their header by `parser/index.ts`.
*   **Clean design beats backward compatibility.** This AST is early; prefer
    reshaping a bad node over layering variants on top of it.
