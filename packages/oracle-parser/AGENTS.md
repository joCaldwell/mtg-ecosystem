# 🔮 oracle-parser — Layer 0: Oracle Text Parser & Card IR

> **Status: PAUSED.** Work here is on hold pending a stronger model — the
> remaining grammar surface is the hard tail (layers, replacement effects,
> complex targeting) where a wrong AST is worse than no AST. Do not resume
> work in this package unless Josh asks for it by name. The active project is
> [deck-builder](../deck-builder/AGENTS.md).

Parse every Magic card's oracle text into a typed AST, then compile it to
per-set JSON IR that Layer 1 (the rules engine) will consume. See
[docs/oracle_parser.md](../../docs/oracle_parser.md) for the design and
[docs/architecture.md](../../docs/architecture.md) for where it sits.

**Stack**: ANTLR4 grammar (`.g4`) → generated TypeScript lexer/parser → AST
visitor → IR emitter. Tests run under **Vitest** (note: the deck-builder uses
`node --test` instead — do not mix them up).

---

## 🚀 Commands

These live on the **root** package.json, so run them from the repo root:

*   `npm install` — install workspace dependencies and link packages.
*   `npm run ingest` — download and cache Scryfall's bulk card export to
    `.scryfall-cache/oracle-cards.json`.
*   `npm run generate-parser` — compile `.g4` grammars to TypeScript under
    `generated/`. **Requires Java.**
*   `npm run build-ir` — emit compiled Card IR into `packages/card-data/sets/`.
*   `npm run validate` — run the parser across all 33,000+ cards and report
    the success rate plus grouped error patterns. This is the scoreboard.

For this package's tests, run `npm test` **from this directory** (`vitest run`).
The root `npm test` fans out to every workspace, so it also runs the
deck-builder's `node --test` suite — fine, but slower and noisier than you
probably want while working on grammar.

---

## 🔄 Development loop: adding grammar support

1.  **Analyze** — `npm run validate` to find failing cards and error patterns.
    Work the largest error group, not the most interesting one.
2.  **Consult the rules** — read the Comprehensive Rules (or a reliable wiki)
    for the mechanic before writing grammar. The AST shape should reflect how
    the rule actually works, because Layer 1 has to execute it. Guessing here
    produces a parse that succeeds and a rules engine that can't use it.
3.  **Lexer** — add new literal words as UPPERCASE tokens in
    [grammar/MTGLexer.g4](grammar/MTGLexer.g4).
4.  **Parser** — add syntactic rules to the right sub-parser file, e.g.
    [grammar/MTGEffectsParser.g4](grammar/MTGEffectsParser.g4).
5.  **Compile** — `npm run generate-parser`.
6.  **AST types** — define nodes in [src/ast/types.ts](src/ast/types.ts).
7.  **Visitor** — extend [src/visitor/ASTBuilder.ts](src/visitor/ASTBuilder.ts)
    to build them.
8.  **Test** — add cases to [tests/parser.test.ts](tests/parser.test.ts).
9.  **Re-validate** — `npm run validate` and confirm the baseline went up.

---

## ⚠️ Constraints

*   **Never commit generated parser files.** Everything in `generated/` is
    gitignored output of `npm run generate-parser`.
*   **No string literals in parser grammars.** The lexer and parser are split,
    so literals like `'exile'` or `'+'` cannot appear in a parser file.
    Declare the token in `MTGLexer.g4` and reference it by name.
*   **No reminder-text parsing.** Parentheticals are stripped in
    [src/ingest/normalize.ts](src/ingest/normalize.ts). The grammar only ever
    sees real rules text.
*   **Import extensions**: no `.ts`/`.js` suffix on imports inside package
    source, unless required. Scripts use `.ts` for native ESM.
*   **Clean design beats backward compatibility.** This grammar is early.
    Prefer rewriting a bad rule over layering a shim on top of it.
*   **A wrong parse is worse than a failed parse.** If the grammar cannot
    represent a mechanic honestly, leave it failing and record why — the
    validate report is meant to show real coverage, not a flattering number.
