# MTG Ecosystem (Project Multiverse)

An agent-first Magic: The Gathering platform with explicit contracts between
card parsing, executable rules, local hosting and applications. TypeScript owns
source ingestion/parsing and clients; Rust owns semantic compilation, deterministic
rules, native simulation/search and hosting.

## What works today

| Layer | Implementation and boundary |
| --- | --- |
| Source data | TypeScript parser and archived AST subset; Rust compiler lowers supported complete sources to executable catalogs. Bulk Card IR emission remains unimplemented. |
| Rules | Deterministic finite Commander matches, events, private decisions, snapshots/forks and replay; unsupported semantics reject or fault explicitly. |
| Search and hosting | Sampled-world search on the reviewed foundations profile; local Rust hosting for foundations and attachments with generated TypeScript SDK. |
| Applications | Active standalone deck-builder with its own Scryfall/SQLite database; it does not depend on parser/engine completion. |

These are finite supported capabilities, not arbitrary-deck Commander or a
complete Magic rules implementation. Browser applications and browser-local
WebAssembly rules remain architectural directions, not a claim of a delivered
full game client. See [architecture](docs/architecture.md).

## Engine development

Read [the engine manual](packages/game-engine/AGENTS.md) and
[current map](docs/game-engine-current.md). From a fresh worktree:

```sh
sh scripts/engine-bootstrap.sh
sh scripts/engine-env.sh npm run check:engine:source
sh scripts/engine-env.sh npm run check:engine -- --plan
```

Bootstrap uses existing pinned tools and installs minimal locked engine/parser
Node dependencies. The environment launcher configures commands and their child
processes. [Development instructions](docs/game-engine-development.md) explain
missing-tool setup, doctor output and choosing focused checks. The default native
check is intentionally narrower than runtime or full acceptance; report omissions.

Accepted D2c-d at `394301d` is separate from later IR25 candidate work and tooling
changes. [Current evidence boundaries](docs/game-engine-current.md#evidence-boundaries-at-the-platform-baseline)
link the exact source-qualified reports. Old release counts and performance
measurements are in the [historical index](docs/history/engine/index.md).

## Other packages

Read the relevant manual before changing a package:

- [Deck-builder](packages/deck-builder/AGENTS.md): standalone application setup.
- [Oracle parser](packages/oracle-parser/AGENTS.md): TypeScript AST/archive tools and structural checks.
- [Game server](packages/game-server/AGENTS.md): local hosting and generated client protocol.
- [Server SDK](packages/game-server/sdk/AGENTS.md): separate SDK dependencies and verification.

The parser's root commands include `ingest`, `archive-parser`, and `validate`;
read its manual before downloading a corpus or interpreting acceptance results.
`npm run build-ir` deliberately rejects the unimplemented bulk pipeline. The
minimal engine bootstrap does not install every application's dependencies.

## Documentation

[Architecture](docs/architecture.md), [language decision](docs/runtime-language-decision.md),
[project structure](docs/project-structure.md), [parser design](docs/oracle_parser.md),
[data schemas](docs/data_schemas.md), [agent design](docs/agent_design.md),
[decisions](docs/decisions.md), [glossary](docs/glossary.md),
[Scryfall integration](docs/scryfall-integration.md), and [contributing](CONTRIBUTING.md).
The [engine plan](docs/game-engine-plan.md) and [specification](docs/game-engine-spec.md)
include goals beyond the implemented finite scope; current onboarding is the
engine map above.
