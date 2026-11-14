# System architecture

Project Multiverse separates source interpretation, executable rules, hosting and
applications. The adopted stack is TypeScript plus Rust; see the
[language decision](architecture.md). Commander is the engine's
supported format, with four-player headless matches and agent search as the
primary workload. The delivered scope is finite and explicitly reviewed.

## Implemented path

```text
Scryfall source → TypeScript parser → versioned AST/source archive
                                            ↓
                          Rust semantic compiler + executable IR contracts
                                            ↓
                      deterministic Rust core ← catalog/profile admission data
                               ↓                         ↓
                    native simulator/search       local Rust host
                                                        ↓
                                              generated TypeScript SDK
```

The deck library uses its own Scryfall/SQLite store and does not consume the
parser, Card IR or rules core. The [top-level Multiverse application](multiverse-app.md) hosts
deck building and game presentation under the existing account. Shared display
components have no protocol or rules authority. Account-to-match provisioning and
full browser play remain distinct integration work; browser-local WebAssembly
rules remain outside the application.

## Layer 0: source parser and semantic compiler

The TypeScript normalizer/lexer/parser produces a supported AST subset while
preserving source boundaries, unknown text and explicit failures. Source archives
bind exact records, ASTs and provenance. Parsing acceptance alone never establishes
executable rules support. See [parser design](oracle_parser.md) and its
[package manual](../packages/oracle-parser/AGENTS.md).

Rust contracts define executable Card IR; the compiler lowers complete supported
sources or rejects them. Catalog manifests select exact archived definitions,
expected IR and canonical digests. Generic source-backed catalog compilation
exists; bulk per-set `build-ir` emission remains unimplemented. Generated bulk
Card IR belongs under `packages/card-data/`, not in hand-edited source files.
See [catalog workflow](game-engine-catalogs.md).

## Layer 1: deterministic rules and native simulation

Core owns state, typed events, priority, announcements/payment, stack resolution,
turns/combat/SBAs, supported continuous/replacement/trigger families, resumable
choices, private observations, budgeted queries, snapshots, forks and replay.
Unsupported semantics are explicit; this is not the complete Magic rules system.
Core imports executable contracts only and performs no host I/O or OS randomness.

Legal finite Commander matches include deck/pod admission, seeded shuffle,
multiplayer mulligans and complete games. Profiles are strict data, not hardcoded
rule selectors. All admitted catalogs use the common rules implementation and
shared structural capacities. Data requests for hosting/search must pass explicit
capability review. See [profiles](game-engine-profiles.md),
[capacities](game-engine-capacity-policy.md) and [runtime invariants](game-engine-invariants.md).

The native simulator provides generic catalog and match commands, public-observation
policies, deterministic evidence/replay and sampled-world search. Search consumes
permitted information and distinguishes hypothetical sample origins from real
restores. Performance claims belong to exact measured source/executables.
See [search](game-engine-search.md) and [the current evidence map](game-engine-current.md).

## Layer 2: local hosting and generated API

The Rust host directly calls core and owns OS processes, HTTP, authentication,
SQLite persistence, queues, clocks and resource groups. Durable accepted inputs,
idempotent commands, private seat views, crash/reconnect and recovery are implemented
for the reviewed finite hosted profiles. The host has its own build identity
alongside the engine identity. Public authentication services, deployment/lobby
infrastructure and unrestricted hosting are outside this finite local release.

Rust DTOs and public core types generate schemas and the TypeScript SDK. Private
snapshots, raw identities, seeds, query receipts and other seats' history never
become public API. Read [the server manual](../packages/game-server/AGENTS.md)
and [host contract](game-engine-server.md) for exact scope.

## Layer 3: applications and agents

The top-level `multiverse` application owns account UX and the product shell,
consuming the existing backend through public host and type contracts. The
game client supplies decision and session adapters, with shared permitted display
components in `game-ui`. The generated SDK supplies the local hosting contract;
native policy/search agents run in Rust, and external clients connect through the
public host protocol. A shared account does not itself grant a host seat. Complete
browser play and automated match creation need their own admission and integration
evidence; they are not implied by a table preview or parser/engine acceptance.

## Adopted decisions and vocabulary

The language decision is final: TypeScript owns ingestion, normalization, parsing,
archives, browser applications and generated SDK consumers; Rust owns semantic
compilation, executable contracts, deterministic core, simulation/search policies
and native hosting. Cargo and npm retain separate toolchains/locks. Browser-local
Wasm reuse is deferred; do not add a second rules implementation. The product
format is Commander. Runtime rules never invoke an LLM; external deck agents use
MCP and the same Actor-scoped capability services as HTTP.

An **Oracle identity** is a card's canonical rules identity, not a printing or
physical game object. A **catalog** is exact compiled source data with a canonical
digest; a **profile** adds reviewed deck/pod admission and consumer requests.
A **capability** declares supported semantics, never permission for a user/seat.
An **Actor** is a verified backend principal plus attribution, not deck ownership.
A **seat capability** is separate Rust-host authorization. An **Observation** is
viewer-permitted state; a snapshot, receipt or query sidecar is trusted host data.
A **budget/capacity fault** is a resource boundary, never a rules win/draw.

Canonical executable definitions are separate from printings/set indexes. Bulk
per-set JSON remains proposed storage, not an implemented emitter. Parser
acceptance, reference binding, compiler support, pod admission, hosting/search
review and browser consumer qualification are separate claims.

For engine development use [the current map](game-engine-current.md),
[mechanics](game-engine-mechanics.md), [invariants](game-engine-invariants.md) and
[source-qualified evidence](game-engine-evidence.md). For product coordination use
[application contracts](multiverse-app.md); for current client integration use
[the client guide](game-client.md). These replace historical plans as onboarding.
