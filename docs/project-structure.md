# Project structure

The adopted stack combines TypeScript/npm workspaces with Rust/Cargo. Packages
have explicit public boundaries; read the relevant package manual before applying commands or
conventions. See the [language decision](runtime-language-decision.md).

The Cargo workspace contains executable contracts, semantic compilation,
deterministic rules and native simulation/search under `packages/game-engine/crates/`,
plus the finite local Rust host in `packages/game-server/`. Supported behavior
and exact acceptance belong in [the current engine map](game-engine-current.md).
Use focused package checks; a browser or documentation task does not imply a
full native matrix.

| Location | Responsibility |
| --- | --- |
| `packages/oracle-parser/` | Hand-written TypeScript Oracle-text parser; limited supported subset |
| `packages/multiverse/` | Top-level product application, account UX, navigation, deck editing and game entry |
| `packages/deck-builder/` | Existing account/API backend, deck capability services, SQLite and MCP tools |
| `packages/game-ui/` | Shared permitted table display, inspection and scoped styling |
| `packages/game-client/` | Game session/decision adapters, qualified preview and standalone harness |
| `packages/card-data/` | Reserved output location for future compiled Card IR |
| `packages/game-engine/` | Rust contracts, compiler, deterministic finite Commander matches, simulation/search and CLI |
| `packages/game-server/` | Finite local Rust process/HTTP host and generated TypeScript SDK; direct core calls |
| `scripts/` | Package environment/bootstrap, focused engine checks, parser ingestion/reporting and explicit bulk IR-emitter stub |
| `docs/` | Shared design documentation |

## Product application

`packages/multiverse/web/src` contains `app/` for account/session/navigation,
`features/` for Home, deck editing, Play and the compact game frame, and the
layered design/UI/domain/API modules described in its
[frontend manual](../packages/multiverse/web/ARCHITECTURE.md).
`packages/multiverse/src/server.ts` owns process composition through the backend's
public host factory. The backend keeps its existing database/auth implementation
and storage defaults; moving application ownership is not a data migration.

`packages/game-ui/src/index.ts` exports the shared permitted display components.
The game-client's public preview entrypoint composes an exact qualified capture
without session effects. Its standalone harness separately owns live joining,
opening choices and command recovery. See [application boundaries](multiverse-app.md)
for the delivered scaffold and remaining game provisioning/admission work.

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
| `npm run archive-parser` | `scripts/archive-parser.ts`: verified source, AST, and binding archive |
| `npm run build-ir` | `scripts/build-ir.ts`: fails; emitter not implemented |
| `npm run check --workspace=@mtg-ecosystem/oracle-parser` | Typecheck and tests, including imported pipeline modules |

`scripts/lib/scryfall.ts` is the shared runtime input validator for the parser
scripts. Root `build` and `test` commands run workspace scripts and should not be
confused with the parser's narrower check.

`.scryfall-cache/`, `dist/`, `generated/`, `target/`, and deck-builder runtime data are
ignored. Curated source tests are checked in; bulk downloads and generated IR
are not. See [architecture.md](architecture.md) for layer relationships and
[oracle_parser.md](oracle_parser.md) for the supported AST contract.
