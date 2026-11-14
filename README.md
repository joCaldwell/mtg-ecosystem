# Project Multiverse

An agent-first Commander platform: a deck library and product application backed
by a finite, deterministic Rust rules engine. Start with [AGENTS.md](AGENTS.md)
for package ownership and development constraints.

## Current product

[Multiverse](packages/multiverse/README.md) owns account/session UX, navigation,
Home, the deck editor, Play setup and the browser build/process. The existing
[deck-builder backend](packages/deck-builder/AGENTS.md) owns authentication,
Actor/SQLite capabilities, deck data, CLI and MCP. There is one account/data
system; no authentication or database migration occurred.

Home and owned-deck setup work. The table preview is a qualified captured sample,
not the selected library deck. The initial game direction is one human and three
automated opponents. Real automated launch, account-to-seat provisioning,
arbitrary-deck admission and complete browser play remain unfinished.
Product sign-in grants no Rust seat. See [application contracts](docs/multiverse-app.md).

## Documentation map

| Task | Canonical starting point |
| --- | --- |
| Package/language ownership and dependency boundaries | [Architecture](docs/architecture.md) |
| Product setup and browser conventions | [Product README](packages/multiverse/README.md), [frontend architecture](packages/multiverse/web/ARCHITECTURE.md), [design](packages/multiverse/web/DESIGN.md) |
| Deck behavior, permissions and public compatibility | [Deck specification](docs/deck-builder-spec.md), [MCP workflows](docs/deck-builder-mcp-workflows.md), [tags](docs/deck-builder-tags.md) |
| Backend operations and checks | [Operations](packages/deck-builder/OPERATIONS.md), [testing](packages/deck-builder/TESTING.md) |
| Client/session contracts, delivery and acceptance | [Game client](docs/game-client.md), [client setup](packages/game-client/README.md) |
| Engine support and source-qualified acceptance | [Current engine map](docs/game-engine-current.md), [mechanics](docs/game-engine-mechanics.md), [evidence](docs/game-engine-evidence.md) |
| Engine environment, verification and storage | [Development](docs/game-engine-development.md), [agent workflow](docs/game-engine-agent-workflow.md), [storage](docs/game-engine-storage.md) |
| Parser, archives and ingestion | [Parser](docs/oracle_parser.md), [data contracts](docs/data_schemas.md), [Scryfall](docs/scryfall-integration.md) |

Read the target package's `AGENTS.md` before editing. Use its locked local tools
and focused checks; there is no root application/native acceptance command.
Engine bootstrap is conditional on missing/broken dependencies, not onboarding
work for every task. See [contributing](CONTRIBUTING.md).

Current manuals describe supported behavior; Rust/types/data and exact evidence
identities resolve implementation details. Old plans and transcripts live in Git
history. Historical test counts never establish current combined acceptance.
