# Oracle Text Parser — Layer 0 Design

This document covers the design of the Oracle Text Parser: the system that reads raw MTG card data (primarily oracle text) and compiles it into a structured, machine-readable intermediate representation (IR).

> **2026-08-01 — Rewritten from scratch.** The original ANTLR/antlr4ts
> implementation was audited and replaced with a hand-written TypeScript
> parser. The audit findings and rationale are recorded in
> [decisions.md](decisions.md) and summarized in
> [Why hand-written, not ANTLR](#-why-hand-written-not-antlr) below.

---

## 🎯 Goal

Take any MTG card's oracle text — e.g.:

> *"When Mulldrifter enters, draw two cards. Evoke {2}{U}"*

...and produce a typed AST that a rules engine (or an AI agent) can consume programmatically:

```typescript
const result = parseOracleText("When ~ enters, draw two cards.\nEvoke {2}{U}", "Mulldrifter");
// result.abilities =
[
  {
    kind: "triggered",
    trigger: { trigger: "enters", what: { ref: "self" } },
    effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 2 } }] }]
  },
  {
    kind: "keywords",
    keywords: [{ keyword: "evoke", cost: [{ cost: "mana", symbols: ["2", "U"] }] }]
  }
]
```

---

## 🏗️ Pipeline

```
┌──────────────────┐    ┌───────────┐    ┌────────┐    ┌───────────┐    ┌──────────────────┐
│  Raw Card Data   │──▶ │ normalize │──▶ │  lex   │──▶ │   parse   │──▶ │  Card IR (JSON)  │
│  (Scryfall JSON) │    │  (lines)  │    │(tokens)│    │(typed AST)│    │  per-set files   │
└──────────────────┘    └───────────┘    └────────┘    └───────────┘    └──────────────────┘
```

All stages live in `packages/oracle-parser/src/` — see that package's
`AGENTS.md` for the module map and development loop.

1. **Normalize** — strip reminder text, replace self-references with `~`
   (exact-case, word-bounded), canonicalize typographic unicode, and split
   into lines. **Line boundaries are ability boundaries** and are preserved;
   the original pipeline flattened them, which forced the grammar to guess
   where abilities split.
2. **Lex** — hand-written tokenizer producing words (possessive-flagged,
   contractions kept whole), numbers, `{…}` symbol tokens, and punctuation.
   Unknown characters are a loud error, never dropped.
3. **Parse** — recursive-descent with backtracking, one line at a time:
   loyalty → ability-word → triggered → activated → keyword line → static →
   spell text. Modal bullet lines are folded into their header's `modal`
   effect. Failures carry a farthest-token diagnostic.
4. **Emit IR** — per-set JSON under `packages/card-data/sets/` (next
   milestone; `npm run build-ir` fails loudly until then).

### The scoreboard

`npm run validate` (repo root) runs the parser across the full Scryfall
oracle-card corpus and reports:

- **card-level coverage** — every line of every face parsed,
- **line-level coverage** — the finer-grained progress metric,
- the **largest unparsed template groups**, which is the work queue.

The number must be honest: a wrong parse is worse than a failed parse, so
nothing in the pipeline is allowed to approximate (unknown keywords fail,
unlexable characters fail, unrepresentable clauses fail).

---

## 🛠️ Why hand-written, not ANTLR?

The first implementation used ANTLR (`antlr4ts`) with the lexer/parser split
across nine `.g4` files. The 2026-08-01 audit found the approach was failing
structurally, not just in degree:

1. **Oracle text is not context-free-friendly.** Nearly every English word is
   simultaneously a keyword, a creature subtype, and part of an ability word.
   The grammar coped via a `nameWord` catch-all that matched `AND`, `OF`,
   `THE`, `WITH`, … inside filters — guaranteeing ambiguous, silently-wrong
   parses. A hand-written parser makes tokenization and disambiguation
   context-sensitive where the language actually is.
2. **The safety property was inverted.** The ANTLR lexer's error listeners
   were removed, so unlexable characters were dropped and the parse counted
   as a *success* (`"Flying ; % ##"` parsed cleanly). The project's core rule
   is that a wrong parse is worse than a failed parse.
3. **It only ever recognized.** The visitor that was supposed to build the
   AST was a 12-line stub returning `{}`; 373 lines of AST types were dead
   code. Recognition-only coverage (16% of cards) measured nothing Layer 1
   could use.
4. **Dead tooling.** `antlr4ts` 0.5.0-alpha has been unmaintained for years,
   required Java to regenerate, and used deprecated APIs.

The replacement is zero-dependency TypeScript: tokenizer + backtracking
recursive descent, building the typed AST directly during parsing (no
separate visitor layer to drift out of sync). At the rewrite baseline it
already parses **33% of cards / 53% of lines** into full typed ASTs — versus
16% recognition-only — with strict full-consumption semantics.

What carried over unchanged: the pipeline concept, Scryfall as the data
source, reminder-text stripping, per-set IR output design, and the
validate-scoreboard development loop.

---

## 📦 Output: Per-Set JSON IR

The compiled card IR is stored as **one JSON file per set** in `packages/card-data/sets/`.

Design principles:
- **Self-describing**: Each IR file includes its schema version and set metadata.
- **Deterministic**: The same oracle text always produces the same IR.
- **Diffable**: Git diffs show exactly which cards in which sets changed.
- **Additive**: New sets are added by dropping a new file — no existing files are modified.

The IR card shape is the `Ability[]` AST from `src/ast.ts` plus card
metadata (name, mana cost, types, P/T). A SQLite index can be added later
for fast cross-set queries.

---

## 🧪 Testing Strategy

- **Structural unit tests** (`node --test`): every test asserts the parsed
  AST's *content* with deep equality — never merely that parsing succeeded.
  (The original suite asserted only `tree.text === input`, which passed while
  the parser produced no AST at all. Do not regress to that.)
- **Scryfall bulk validation**: `npm run validate` is the coverage
  scoreboard; it must go up, honestly, with each grammar extension.
- **Round-trip tests** (future): `AST → reconstructed text` semantic
  equivalence once the AST stabilizes.

---

## ✅ Resolved Decisions

| Question | Decision |
|----------|----------|
| **Parser Tool** | **Hand-written TypeScript** (tokenizer + backtracking recursive descent). ANTLR approach retired 2026-08-01 — see above. |
| **AST construction** | Built directly during parsing; the AST in `src/ast.ts` is the Layer-1 contract. Discriminated unions, no `any`. |
| **IR Storage** | **Per-set JSON files** in `packages/card-data/sets/`. SQLite index left as a future option. |
| **Data Source** | **Scryfall** bulk data is the source of truth for all card data. |
| **Reminder Text** | **Strip before parsing.** The system has its own understanding of keywords. |
| **Keyword Representation** | **Atomic + table-driven.** `KeywordInstance` records the name and parameters; the rules engine knows semantics. Unknown keywords fail the line. |
| **Edge-Case Cards** | **Out of scope for now.** Un-sets, Arena-only mechanics (e.g. *conjure*), and truly unique cards stay unparsed rather than misparsed. |
| **Python Bindings** | **Future addition.** Not needed for Milestone 1. |
