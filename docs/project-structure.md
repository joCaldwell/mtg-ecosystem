# Project structure

This is a TypeScript monorepo using npm workspaces. Packages are independent;
read the relevant package manual before applying commands or conventions.

| Location | Responsibility |
| --- | --- |
| `packages/oracle-parser/` | Hand-written TypeScript Oracle-text parser; limited supported subset |
| `packages/deck-builder/` | Active standalone deck-building application |
| `packages/card-data/` | Reserved output location for future compiled Card IR |
| `packages/game-engine/` | Future rules engine; README only |
| `packages/game-server/` | Future game API; README only |
| `scripts/` | Parser ingestion, acceptance reporting, and explicit IR-emitter stub |
| `docs/` | Shared design documentation |

## Oracle parser

```text
packages/oracle-parser/
  AGENTS.md                  package constraints and development loop
  package.json               Node test runner; tsc --noEmit
  tsconfig.json              strict Node TypeScript checking
  src/
    index.ts                 public exports
    ast.ts                   authoritative AST and result unions
    normalize.ts             reminder text, self-references, line boundaries
    lexer.ts                 tokenization and lexical errors
    symbols.ts               shared supported mana-symbol validation
    vocab.ts                 closed word classes and singularization
    parser/
      cursor.ts              transactional token cursor and diagnostics
      refs.ts                amounts, filters, objects, players, zones
      keywords.ts            keyword names and parameter parsing
      costs.ts               activation and keyword costs
      triggers.ts            event conditions
      effects.ts             resolution instructions and modal options
      statics.ts             supported continuous abilities
      index.ts               classification, block grouping, card API
  test/
    lexer.test.ts
    normalize.test.ts
    parser.test.ts           structural examples
    foundations.test.ts      semantic distinctions, failure and rollback cases
    pipeline.test.ts         ingestion failures and reproducible reporting
```

There are no `.g4` grammar files, generated parser, AST visitor, Java dependency,
or emitted parser build. The retired ANTLR implementation remains in Git history.

## Root parser commands

| Command | Implementation |
| --- | --- |
| `npm run ingest` | `scripts/ingest-scryfall.ts`: validate and atomically replace bulk cache |
| `npm run validate` | `scripts/validate-parser.ts`: scope, identities, acceptance and failure groups |
| `npm run validate -- --json` | Same report with per-card AST hashes in JSON |
| `npm run build-ir` | `scripts/build-ir.ts`: fails; emitter not implemented |
| `npm run check --workspace=@mtg-ecosystem/oracle-parser` | Typecheck and tests, including imported pipeline modules |

`scripts/lib/scryfall.ts` is the shared runtime input validator for the parser
scripts. Root `build` and `test` commands run workspace scripts and should not be
confused with the parser's narrower check.

`.scryfall-cache/`, `dist/`, `generated/`, and deck-builder runtime data are
ignored. Curated source tests are checked in; bulk downloads and generated IR
are not. See [architecture.md](architecture.md) for layer relationships and
[oracle_parser.md](oracle_parser.md) for the supported AST contract.
