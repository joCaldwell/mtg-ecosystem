# Contributing to Project Multiverse

Start with the root [AGENTS.md](AGENTS.md), identify the package, and read its
manual. The top-level Multiverse app owns account UX, navigation, deck editing
and game entry. The existing deck-builder backend owns account/auth, deck
capabilities, SQLite and MCP behind public interfaces. The oracle-parser remains
a limited supported subset. The engine/server support reviewed finite Commander
matches; the product currently integrates setup and a qualified table preview,
with real automated-game launch still outside the delivered scaffold.

The [language stack](docs/runtime-language-decision.md) is adopted: TypeScript
for ingestion/parser/archive, the deck-builder, browser apps, and client SDK;
Rust for semantic compilation, rules, native simulation/search/agents, and the
game server. Read the [engine manual](packages/game-engine/AGENTS.md) and
[current map](docs/game-engine-current.md) for native evidence and focused checks.

## Application work

- [Multiverse](packages/multiverse/README.md): browser/account UX, routing,
  Home, deck editor, Play setup, product build and process entrypoint.
- [Deck backend](packages/deck-builder/AGENTS.md): existing auth/Actor/SQLite,
  deck capabilities, API, CLI and MCP. Preserve data, credentials and public URLs.
- [Game UI](packages/game-ui/AGENTS.md): shared permitted table display and scoped styles.
- [Game client](packages/game-client/README.md): session/decision adapters,
  qualified public preview and standalone integration harness.

Use each package's isolated lockfile and pinned Node launcher. Run the product
and backend gates for cross-boundary changes; game display/session changes also
need their consumer checks. Product builds must not start Cargo or generate
native/SDK artifacts. See [application architecture](docs/multiverse-app.md) for
public package interfaces and the remaining account-to-game launch requirements.

## Oracle parser setup

Use Node ≥ 23, npm, and Git. Install workspace dependencies at the root:

```sh
npm install
npm run check --workspace=@mtg-ecosystem/oracle-parser
```

The parser runs TypeScript directly. `build` means `tsc --noEmit`; tests use
`node --test`. No Java, generated grammar, or visitor is required. Package
commands and constraints are in
[packages/oracle-parser/AGENTS.md](packages/oracle-parser/AGENTS.md).

## Changing the parser

1. Check actual card text in card data and consult the relevant Comprehensive
   Rules. Cite the rule/source for externally defined behavior in code.
2. Decide the complete AST shape in `src/ast.ts`. Preserve distinctions in
   logic, event grouping, ownership, targets, costs, restrictions, and scope.
   Reject unsupported constructs rather than accepting an approximation.
3. Extend the relevant parser module. Returning `null` must restore the cursor;
   effect parsing must also restore contextual state after failed attempts.
4. Add complete structural assertions and negative cases. Pair similar inputs
   when a small wording change affects meaning. Avoid success-only tests and
   unchecked snapshot updates.
5. Run the package check. With a local corpus, run `npm run validate` at the
   root and inspect accepted/rejected examples and changed ASTs. A correctness
   fix may lower acceptance. Compare reports only under the same corpus and
   scope; `--json` includes the identities needed for that comparison.
6. Update the current contract documentation. `src/ast.ts` is authoritative;
   future IR design must be labeled as a proposal.

Pipeline tests use temporary directories and simulated HTTP responses. They
must not need a bulk download or mutate the working cache.

Do not commit downloaded/generated artifacts or modify another package's
ongoing work. Do not assume root test, build, or dependency conventions match
another package's commands. No repository-wide formatter or linter command is
currently prescribed; follow the local TypeScript style and use `git diff
--check` for whitespace errors.

See [parser design](docs/oracle_parser.md), [data contracts](docs/data_schemas.md),
and [Scryfall ingestion](docs/scryfall-integration.md). `npm run build-ir` fails
intentionally until semantic validation and the emitter are implemented.
