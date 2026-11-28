# Project Multiverse

An agent-first Commander platform: a deck library and product application backed
by a finite, deterministic Rust rules engine. [AGENTS.md](AGENTS.md) owns package
ownership and development constraints. [Multiverse](packages/multiverse/README.md)
composes the [deck-builder backend](packages/deck-builder/AGENTS.md), the one
account/data system, with the [game](packages/game/README.md) session in one
process; product sign-in grants no Rust seat.

## Documentation map

| Task | Canonical starting point |
| --- | --- |
| Package/language ownership and dependency boundaries | [System architecture](#system-architecture), [application boundaries](packages/multiverse/web/ARCHITECTURE.md#application-boundaries) |
| Product setup, browser conventions and launch gates | [Product README](packages/multiverse/README.md), [frontend architecture](packages/multiverse/web/ARCHITECTURE.md), [design](packages/multiverse/web/DESIGN.md), [game launch](packages/multiverse/web/ARCHITECTURE.md#game-launch) |
| Deck behavior, permissions, MCP workflows and public compatibility | [Deck specification](docs/deck-builder-spec.md), [agent method skill](packages/deck-builder/skill/commander-deck-building/SKILL.md) |
| Backend operations and checks | [Operations](packages/deck-builder/OPERATIONS.md), [test gate](packages/deck-builder/AGENTS.md#focused-test-gate) |
| Client/session contracts, delivery and acceptance | [Game client](docs/game-client.md), [client setup](packages/game/README.md) |
| Engine and parser rework: stages, gates and training target | [Rework plan](docs/game-engine-rework.md) |
| Engine support and source-qualified acceptance | [Current engine map](docs/game-engine-current.md), [core mechanics](docs/game-engine-mechanics.md), [board mechanics](docs/game-engine-board-mechanics.md), [evidence](docs/game-engine-evidence.md) |
| Engine environment, verification and storage | [Development and verification](docs/game-engine-development.md), [storage](docs/game-engine-storage.md) |
| Parser, archives and ingestion | [Parser](docs/oracle_parser.md), [data contracts](docs/oracle_parser.md#data-contracts), [Scryfall](docs/oracle_parser.md#ingestion) |

Read the target package's `AGENTS.md` before editing and use its locked tools and
focused checks; there is no root application/native acceptance command. Manuals
describe current supported behavior; Rust/types/data and exact evidence identities
resolve details ([handoff policy](AGENTS.md#documentation-and-handoff)).

## System architecture

TypeScript owns ingestion, normalization, parsing, archives, browser applications
and generated SDK consumers; Rust owns semantic compilation, executable contracts,
deterministic rules, native simulation/search and hosting, with separate Cargo/npm
toolchains and locks. Commander is the supported format; browser-local Wasm rules
are deferred. Runtime rules never invoke an LLM.

```text
Scryfall → TypeScript parser → nonexecutable source/AST archive
                                         ↓
                          Rust compiler → executable IR catalog
                                         ↓
                      deterministic core ← profile admission data
                             ↓                       ↓
                  native simulation/search     local Rust host
                                                     ↓
                                          generated TypeScript SDK
```

Compile complete supported sources or reject them. Parser acceptance/reference
binding, compiler support, pod admission, hosting/search review and browser
qualification are separate claims. Core imports the IR contracts and permitted view
with no host I/O, clocks or OS randomness; profiles select admission data, never
mechanics. The NumPy gym runs native matches and encodes the choosing seat's
observation. The host owns HTTP/auth/SQLite, resource groups and exact durable
recovery with its own build identity; [engine scope](docs/game-engine-current.md)
and [host contracts](docs/game-engine-server.md) define the finite supported
release. Per-set `build-ir` emission and complete printing/set indexes remain
unfinished; generated card data is not hand-edited. The deck library uses
independent Scryfall/SQLite facts, not parser AST or Card IR; external agents use
MCP over the same Actor-scoped capabilities as HTTP.

## Vocabulary

- **Oracle identity:** canonical card identity, separate from printing and physical object.
- **Catalog:** exact compiled definitions plus source provenance and canonical digest.
- **Profile:** reviewed deck/pod admission data and consumer requests.
- **Capability:** supported semantics; a **seat capability** separately authorizes a host seat.
- **Actor:** verified backend principal and attribution; ownership is a relation.
- **Observation:** viewer-permitted state. Snapshots, receipts and query sidecars are trusted host data.
- **Budget/capacity fault:** resource boundary, never a rules win or draw.
