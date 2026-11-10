# System architecture

Project Multiverse separates source interpretation, executable rules, hosting and
applications. The adopted stack is TypeScript plus Rust; see the
[language decision](runtime-language-decision.md). Commander is the engine's
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

The standalone deck-builder uses its own Scryfall/SQLite store. It does not
consume the parser, Card IR or rules core. Future integration must not block its
current development. General game UI, browser-local WebAssembly rules and broader
clients remain architectural directions.

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

The active deck-builder is independent. The generated SDK supplies the local
hosting client contract; native policy/search agents run in Rust, and external
clients connect through the public host protocol. A complete browser game client,
draft simulator and general rules adviser are future applications, not implied
by current engine or parser acceptance.

For development, start at [the current map](game-engine-current.md) and
[focused workflow](game-engine-development.md). Broader milestones remain in the
[plan](game-engine-plan.md) and [specification](game-engine-spec.md); prior release
proof and onboarding remain in the [historical index](history/engine/index.md).
