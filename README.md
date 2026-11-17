# Project Multiverse

An agent-first Commander platform: a deck library and product application backed
by a finite, deterministic Rust rules engine. Start with [AGENTS.md](AGENTS.md)
for package ownership and development constraints.

## Current product

[Multiverse](packages/multiverse/README.md) owns account/session UX, navigation,
Home, the deck editor and the browser build/process. [Game](packages/game/README.md)
owns the existing session UI, launch/recovery, native supervision and shared DTOs
in `web/`, `server/` and `shared/`, composed in that same process. The existing
[deck-builder backend](packages/deck-builder/AGENTS.md) owns authentication,
Actor/SQLite capabilities, deck data, CLI and MCP. There is one account/data
system; no authentication or database migration occurred.

Home and owned-deck setup work. The table preview is a qualified captured sample,
not the selected library deck. The initial game direction is one human and three
automated opponents. Real automated launch, account-to-seat provisioning,
arbitrary-deck admission and complete browser play remain unfinished.
Product sign-in grants no Rust seat. See [application contracts](packages/multiverse/web/ARCHITECTURE.md#application-boundaries).

## Documentation map

| Task | Canonical starting point |
| --- | --- |
| Package/language ownership and dependency boundaries | [System architecture](#system-architecture) |
| Product setup and browser conventions | [Product README](packages/multiverse/README.md), [frontend architecture](packages/multiverse/web/ARCHITECTURE.md), [design](packages/multiverse/web/DESIGN.md) |
| Deck behavior, permissions and public compatibility | [Deck specification](docs/deck-builder-spec.md), [MCP workflows](docs/deck-builder-mcp-workflows.md) |
| Backend operations and checks | [Operations](packages/deck-builder/OPERATIONS.md), [test gate](packages/deck-builder/AGENTS.md#focused-test-gate) |
| Client/session contracts, delivery and acceptance | [Game client](docs/game-client.md), [client setup](packages/game/README.md) |
| Engine support and source-qualified acceptance | [Current engine map](docs/game-engine-current.md), [mechanics](docs/game-engine-mechanics.md), [evidence](docs/game-engine-evidence.md) |
| Engine environment, verification and storage | [Development and verification](docs/game-engine-development.md), [storage](docs/game-engine-storage.md) |
| Parser, archives and ingestion | [Parser](docs/oracle_parser.md), [data contracts](docs/oracle_parser.md#data-contracts), [Scryfall](docs/oracle_parser.md#ingestion) |

Read the target package's `AGENTS.md` before editing. Use its locked local tools
and focused checks; there is no root application/native acceptance command.
Engine bootstrap is conditional on missing/broken dependencies, not onboarding
work for every task. See [documentation and handoff policy](AGENTS.md#documentation-and-handoff).

Current manuals describe supported behavior; Rust/types/data and exact evidence
identities resolve implementation details. Old plans and transcripts live in Git
history. Historical test counts never establish current combined acceptance.

## System architecture

The adopted stack is TypeScript for ingestion, normalization, parsing, archives,
browser applications and generated SDK consumers; Rust for semantic compilation,
executable contracts, deterministic rules, native simulation/search and hosting.
Cargo/npm keep separate toolchains and locks. Commander is the supported format;
browser-local Wasm rules are deferred. Runtime rules never invoke an LLM.

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
qualification are separate claims. Core imports contracts only and has no host
I/O, clocks or OS randomness; profiles select admission data, never mechanics.
The host owns HTTP/auth/SQLite, resource groups and exact durable recovery, with
its own build identity. [Engine scope](docs/game-engine-current.md) and
[host contracts](docs/game-engine-server.md) define the finite supported release.
General per-set `build-ir` emission and complete printing/set indexes remain
unfinished; generated card data is not hand-edited source.

The deck library uses independent Scryfall/SQLite facts, not parser AST or Card IR.
External agents use MCP and the same Actor-scoped capabilities as HTTP.
[Application boundaries](packages/multiverse/web/ARCHITECTURE.md#application-boundaries)
define product/display/session ownership and the remaining game-launch gates.

## Vocabulary

- **Oracle identity:** canonical card identity, separate from printing and physical object.
- **Catalog:** exact compiled definitions plus source provenance and canonical digest.
- **Profile:** reviewed deck/pod admission data and consumer requests.
- **Capability:** supported semantics; a **seat capability** separately authorizes a host seat.
- **Actor:** verified backend principal and attribution; ownership is a relation.
- **Observation:** viewer-permitted state. Snapshots, receipts and query sidecars are trusted host data.
- **Budget/capacity fault:** resource boundary, never a rules win or draw.
