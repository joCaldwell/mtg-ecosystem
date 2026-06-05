# Decision Log: Parser Tool / Language & IR Storage

This document records the options considered and final decisions for two key architectural choices:
1. **What tool/language should we use to build the Oracle Text Parser?** → ✅ **ANTLR → TypeScript**
2. **How should we store the compiled Card IR?** → ✅ **Per-set JSON (with optional SQLite index later)**

---

## 🔧 Decision 1: Parser Tool / Language

### Background
MTG oracle text is a *semi-natural language* — it looks like English, but it follows rigid templating patterns. The parser needs to:
- Tokenize structured patterns like `"Destroy target creature."` and `"{T}: Add {G}."`
- Handle ~180+ keyword abilities atomically
- Be extensible when new mechanics are printed
- Produce a strongly-typed AST
- Parse 27,000+ unique cards reliably

There's an important distinction between **parser generators** (you write a grammar file, the tool generates a parser) and **parser combinators** (you write the parser as code using composable functions).

---

### Option A: ANTLR (Parser Generator) ⭐

> *This is likely the tool you were thinking of.* You write a `.g4` grammar file, and ANTLR generates a full lexer + parser in your chosen target language.

**How it works**: Define a grammar like:
```antlr
ability     : triggered_ability | activated_ability | static_ability | keyword_ability ;
triggered_ability : trigger_condition COMMA effect PERIOD ;
trigger_condition : 'When' subject event_type ;
```
ANTLR generates a lexer, parser, and parse tree walker in your target language. You then write a Visitor/Listener to convert the parse tree → your custom AST.

| ✅ Pros | ❌ Cons |
|---------|---------|
| The grammar IS the documentation — a `.g4` file is a readable, formal spec of the language | Requires a Java runtime at build time to generate code |
| Battle-tested at scale (used by Twitter, Hibernate, Spark SQL) | Generated code can feel "foreign" — lots of boilerplate visitors |
| Generates parsers in **10 languages**: Java, TypeScript, Python, Go, C++, C#, Swift, Dart, PHP | The Rust target (`antlr4rust`) is community-maintained and less mature |
| Existing MTG grammar projects to build on ([mtg-grammar](https://github.com/Soothsilver/mtg-grammar), [Demystify](https://github.com/Zannick/demystify)) | Two-step workflow: edit `.g4` → regenerate → compile |
| Excellent error recovery — can continue parsing after errors | Separate grammar file means AST type definitions live elsewhere |
| Mature IDE tooling (syntax highlighting, railroad diagrams for grammars) | Can struggle with context-sensitive rules without semantic predicates |
| Visitor/Listener pattern separates grammar from application logic cleanly | |

**Best target language if chosen**: **TypeScript** or **Go** (both have mature, officially-maintained ANTLR runtimes; TypeScript gives you easy WASM/browser deployment and the broadest developer accessibility).

---

### Option B: Pest (PEG Parser Generator for Rust)

> A Rust-native PEG parser generator. You write a `.pest` grammar file, and it generates a Rust parser via procedural macros at compile time.

**How it works**: Define a grammar:
```pest
ability = { triggered_ability | activated_ability | keyword_ability }
triggered_ability = { "When" ~ subject ~ event ~ "," ~ effect ~ "." }
```
Pest generates parsing code at compile time (no external tool needed). You then match on the parse tree to build your AST.

| ✅ Pros | ❌ Cons |
|---------|---------|
| Grammar file (`.pest`) is clean and readable | Rust-only — no multi-language code generation |
| No external tools — integrates into `cargo build` seamlessly | PEG grammars use ordered alternatives (`/`), which can cause subtle bugs if ordering is wrong |
| Very fast for small-medium grammars | Error reporting is harder to customize than ANTLR |
| Idiomatic Rust output | Less community support / smaller ecosystem than ANTLR |
| Separation of grammar from code (like ANTLR) | No existing MTG grammars to build on |
| | Can have performance issues with complex backtracking |

---

### Option C: Chumsky (Parser Combinator Library for Rust)

> A modern Rust parser combinator library focused on ergonomic error reporting. You write the parser as composable Rust functions — no separate grammar file.

**How it works**: Define parsers as code:
```rust
let triggered_ability = just("When")
    .then(subject())
    .then(event())
    .then_ignore(just(","))
    .then(effect())
    .then_ignore(just("."))
    .map(|((subj, evt), eff)| Ability::Triggered { trigger: evt, effect: eff });
```

| ✅ Pros | ❌ Cons |
|---------|---------|
| Parsers are just Rust code — full IDE support, type checking, refactoring | No separate grammar file — the grammar is "hidden" in code, harder to read as a spec |
| Best-in-class error reporting (with Ariadne for pretty terminal diagnostics) | Steeper learning curve for non-Rustaceans |
| Full power of Rust at every parsing step (context-sensitive logic is trivial) | Rust-only |
| Zero-copy parsing, high performance | Harder for non-developers to contribute to or review the grammar |
| AST types live right next to the parser code — tight coupling by design | Composability can become unwieldy for very large grammars |
| Active development, modern library | |

---

### Option D: ANTLR Grammar → TypeScript Runtime (Hybrid Approach)

> Write the grammar in ANTLR's `.g4` format (getting the readable spec + existing MTG grammar projects), but generate a **TypeScript** parser for the runtime. This gives you the grammar-as-documentation benefit of ANTLR with the accessibility and portability of the JS/TS ecosystem.

| ✅ Pros | ❌ Cons |
|---------|---------|
| Grammar file IS the spec (`.g4` is readable by anyone) | TypeScript is slower than Rust for bulk parsing (but likely fast enough for 27K cards) |
| Officially maintained TypeScript target | Requires Java at build time for code generation |
| Easy to run in Node.js, browsers, Deno, or Bun | Less type-safe AST representation than Rust enums |
| Largest potential contributor pool | Two-step build workflow |
| Can leverage existing MTG ANTLR grammars | |
| JSON output is native and trivial | |
| Future Python/WASM bindings are straightforward | |

---

### ✅ Final Decision: Option D — ANTLR → TypeScript

**Rationale**: Combines ANTLR's grammar-as-documentation philosophy with the TypeScript ecosystem's accessibility and natural fit for the upper layers (game server, web clients). The `.g4` grammar file serves as the living spec, TypeScript provides native JSON output and broad developer access, and existing MTG ANTLR grammars can be leveraged.

---
---

## 💾 Decision 2: IR Storage Format

After parsing, compiled card data needs to be stored. Here are the options:

---

### Option 1: One JSON File Per Card

```
ir/
├── cards/
│   ├── lightning-bolt_e3285e6b.json
│   ├── mulldrifter_a7c14a04.json
│   └── ... (27,000+ files)
```

| ✅ Pros | ❌ Cons |
|---------|---------|
| Maximum granularity — update one card without touching others | 27,000+ files is unwieldy in file explorers and git |
| Trivially diffable in version control (git shows exactly which cards changed) | Slow filesystem operations (opening/reading thousands of small files) |
| Easy to debug — open one file, see one card | Large git repos with many small files can degrade performance |
| Simple to understand | Importing the full database requires reading thousands of files |

---

### Option 2: One JSON File Per Set

```
ir/
├── sets/
│   ├── LEA.json        (Limited Edition Alpha)
│   ├── MH3.json        (Modern Horizons 3)
│   ├── DSK.json        (Duskmourn)
│   └── ... (~350 files)
```

| ✅ Pros | ❌ Cons |
|---------|---------|
| Natural grouping — MTG releases cards in sets | A single card change re-serializes the entire set file |
| ~350 files is very manageable | Cross-set reprints need a strategy (duplicate data or reference IDs?) |
| Diffs show which sets were affected | Set files vary wildly in size (some have 15 cards, some have 400+) |
| Easy to add a new set: just drop a new file | Slightly harder to look up a single card without an index |
| Mirrors Scryfall's bulk data structure | |

---

### Option 3: Single Monolithic JSON/JSONL File

```
ir/
├── all_cards.jsonl     (one card per line, ~27,000 lines)
```

| ✅ Pros | ❌ Cons |
|---------|---------|
| One file to load — simplest possible ingestion | Any change re-serializes the whole file |
| JSONL (newline-delimited) allows streaming reads | Git diffs are harder to review (massive file) |
| Easy to pipe into other tools (`jq`, `grep`, etc.) | Can't partially load — it's all or nothing |
| Smallest total file size (no per-file overhead) | Merge conflicts in version control are painful |

---

### Option 4: SQLite Database

```
ir/
├── cards.db            (SQLite file)
```

| ✅ Pros | ❌ Cons |
|---------|---------|
| Queryable — `SELECT * FROM cards WHERE types LIKE '%Creature%'` | Binary file — not human-readable, not diffable in git |
| Fast lookups, indexing, and filtering | Requires SQLite tooling to inspect |
| Single file, no filesystem clutter | Merge conflicts are impossible to resolve manually |
| Supports relationships natively (cards → sets, cards → rulings) | Adds a dependency (SQLite library) |
| Great for agent queries ("find all red instants with CMC ≤ 2") | Harder for AI/LLM tools to directly consume vs JSON |

---

### Option 5: Hybrid — Per-Set JSON + SQLite Index (or JSONL + SQLite)

```
ir/
├── sets/
│   ├── LEA.json
│   ├── MH3.json
│   └── ...
├── index.db            (SQLite with card metadata for fast queries)
└── all_cards.jsonl     (optional: flat export for bulk consumers)
```

| ✅ Pros | ❌ Cons |
|---------|---------|
| Per-set JSON files for human readability and clean git diffs | More moving parts — two formats to keep in sync |
| SQLite index for fast programmatic queries | Build step required to regenerate the index |
| JSONL export for bulk consumers and AI pipelines | Slightly more complex tooling |
| Best of all worlds — each consumer picks the format that suits them | |
| New sets: drop a JSON file, rebuild index | |

---

### ✅ Final Decision: Option 2 — Per-Set JSON (with SQLite index as future enhancement)

**Rationale**: Per-set files map naturally to how MTG releases work. Git diffs are clean and meaningful. Adding a new set is just dropping a new file and re-running the parser. A SQLite index (Option 5) is left as an open future enhancement for fast cross-set queries when needed.

---

## ✅ Decision Summary

| Decision | Chosen | Rationale |
|----------|--------|----------|
| **Parser tool** | **ANTLR → TypeScript** | Grammar-as-documentation, existing MTG grammars, TS ecosystem for upper layers |
| **IR storage** | **Per-set JSON** | Natural grouping, clean diffs, easy set additions. SQLite index planned as future enhancement |
