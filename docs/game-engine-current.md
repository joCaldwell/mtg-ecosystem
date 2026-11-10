# Current engine map

Start here for engine/server work. The engine executes complete **finite**
Commander matches, supports sampled-world search on its reviewed subset, and has
a local Rust host with a generated TypeScript SDK. It is not a general Magic
rules implementation or arbitrary-deck service. The standalone deck-builder
uses its own Scryfall database and does not consume this engine.

## Architecture and ownership

| Location | Responsibility |
| --- | --- |
| `packages/oracle-parser/` | TypeScript source parsing and versioned AST archive; complete parsing alone does not prove executable semantics |
| `packages/game-engine/crates/contracts/` | Rust executable Card IR types, validation and schema authority |
| `packages/game-engine/crates/compiler/` | Complete-source lowering or explicit rejection; archived-source provenance and catalog loading |
| `packages/game-engine/crates/core/` | Deterministic state, rules, events, decisions, observations, queries, snapshots and replay |
| `packages/game-engine/crates/sim/` | Native CLI, public-observation policies, finite matches, evidence, sampled search |
| `packages/game-server/` | Local process/HTTP/SQLite hosting, authentication and recovery; separate host build identity |
| `packages/game-server/sdk/` | Generated public TypeScript client contracts; separate package manual and install |
| `packages/game-engine/tests/fixtures/` | Exact local card source records, catalog manifests, profile data and small reviewed references |

Core imports contracts, never compiler/parser. Filesystem/network/clock/OS
randomness and inference stay outside core. Catalog/profile identity is separate
from engine code identity. The host adds its own SERVER_BUILD_ID. See
[runtime invariants](game-engine-invariants.md), [profile admission](game-engine-profiles.md)
and [catalog/source workflow](game-engine-catalogs.md) before changing those boundaries.

All admitted catalogs use the common rules implementation. Historical profile
names/IR versions do not enable mechanics or choose capacity ceilings. Current
limits live in `crates/core/src/engine/capacities.rs` beneath the engine package;
the [capacity policy](game-engine-capacity-policy.md) explains their rationale.
Profiles request hosting/search through data, subject to explicit reviewed
capability sets: foundations and attachments are hosted; foundations alone is
search-enabled. New compiler support does not implicitly expand those sets.

## Evidence boundaries at the platform baseline

This platform work starts from `c62777b` on local `feat/engine-plan`, which includes
engine work absent from `main`. These are separate evidence claims:

| Source boundary | Status and scope |
| --- | --- |
| D2c-c at `a7f5571` | Accepted ordinary cast triggers at IR23; see [its report](game-engine-cards-as-data-d2c-c.md) |
| D2c-d + reviewed demo/Bundle lifetimes at `394301d` | Accepted IR24 declared-attack triggers and parent evidence audit; see [D2c-d](game-engine-cards-as-data-d2c-d.md) and [lifetimes](game-engine-verification-lifetimes.md) |
| Later IR25/D2c-e | Present on the development line, with candidate-focused validation; final parent source review/combined acceptance remain pending in [the D2c-e report](game-engine-cards-as-data-d2c-e.md) |
| Later digest reuse and November tooling changes | Separate focused-work evidence, not part of the frozen D2c-d gate; see [trace digests](game-engine-verification-trace-digests.md) and [fast checks](game-engine-fast-checks-20261109.md) |

At this baseline trusted snapshot/bundle formats are 19/20; public
projection/context/query-sidecar formats are 16/16/15. Read Rust types and
validation at the edited source for version authority; old reports retain their
original version numbers. Do not update historical catalogs or expected hashes
just to match the newest IR version. No new quiet P7 performance, offline
rebuild, full-profile or combined-acceptance claim follows from a focused check.

## Start and navigate

- [Development commands](game-engine-development.md): bootstrap, doctor, environment and focused checks.
- [Verification workflow](game-engine-verification-workflow.md): exact coverage omissions, lock ownership and checkpoint cadence.
- [Runtime invariants](game-engine-invariants.md): read the sections affected by a rules, state, privacy or recovery edit.
- [Catalog workflow](game-engine-catalogs.md): source facts, provenance, compilation and generic match commands.
- [Historical index](history/engine/index.md): previous onboarding, release contracts, unchanged evidence paths and recovery.
- [Storage policy](game-engine-storage.md): budgets and bounded retention; ignored output is not automatically disposable.

The long [progress ledger](game-engine-progress.md) is chronological evidence,
not the onboarding sequence. The [plan](game-engine-plan.md) and
[specification](game-engine-spec.md) contain broader goals; use this map and the
package manual to distinguish implemented finite behavior from planned scope.
