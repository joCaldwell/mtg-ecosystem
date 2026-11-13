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

Core's runtime imports contracts, never compiler/parser; compiler-backed tests
are development-only dependencies. Filesystem/network/clock/OS
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
The separate [damage-trigger development profile](game-engine-damage-match-profile.md)
admits finite legal headless matches for IR26–IR29. Its separate
[v2 profile](game-engine-damage-match-profile-v2.md) extends the pool through IR32;
neither profile requests hosting or search.

## Evidence boundaries at the platform baseline

The platform baseline was `c62777b` on the former `feat/engine-plan` branch.
Engine, platform and architecture work are now integrated on `main`; start new
independent work there. Historical evidence remains separately qualified:

| Source boundary | Status and scope |
| --- | --- |
| D2c-c at `a7f5571` | Accepted ordinary cast triggers at IR23; see [its report](game-engine-cards-as-data-d2c-c.md) |
| D2c-d + reviewed demo/Bundle lifetimes at `394301d` | Accepted IR24 declared-attack triggers and parent evidence audit; see [D2c-d](game-engine-cards-as-data-d2c-d.md) and [lifetimes](game-engine-verification-lifetimes.md) |
| Later IR25/D2c-e | Present on the development line, with candidate-focused validation; final parent source review/combined acceptance remain pending in [the D2c-e report](game-engine-cards-as-data-d2c-e.md) |
| Later finite trigger extensions | Separate capability/version gates cover [opponent damage](game-engine-opponent-damage-triggers.md), [optional draws](game-engine-optional-draws.md), [event-player discard](game-engine-event-player-discard.md), [event-player sacrifice](game-engine-event-player-sacrifice.md), [filtered sources](game-engine-filtered-damage-sources.md), [controller discard](game-engine-damage-controller-discard.md), [damage-quantity draws](game-engine-damage-quantity-draw.md), and [attacking keywords/Snow](game-engine-attacking-creature-keywords.md) across IR26–IR33. The subsequent [event-time appearance correction](game-engine-damage-match-appearance.md) preserves these IR/catalog contracts. Their focused validation does not renew combined acceptance or expand hosting/search. |
| Later digest reuse and November tooling changes | Separate focused-work evidence, not part of the frozen D2c-d gate; see [trace digests](game-engine-verification-trace-digests.md) and [fast checks](game-engine-fast-checks-20261109.md) |

At the platform baseline trusted snapshot/bundle formats were 19/20 and public
projection/context/query-sidecar formats were 16/16/15. The architecture refactor
retains snapshot/bundle/public formats but advances query work sidecars to 16
and adds one charged initialization unit; old sidecars reject. `mtg_contracts::CURRENT_IR_VERSION`
is the current IR authority; see [version and build checks](game-engine-builds.md).
The unreleased schema stays at 33 while there are no external consumers; new
features evolve it in place with capability validation and exact build fencing.
Read Rust types and validation at the edited source; old reports retain their
original version numbers. Do not update historical catalogs or expected hashes
just to match the newest IR version. No new quiet P7 performance, offline
rebuild, full-profile or combined-acceptance claim follows from a focused check.

## Start and navigate

- [Agent workflow](game-engine-agent-workflow.md): task entry, autonomous execution, machine output and verifiable handoffs.
- [Architecture refactor](game-engine-architecture-refactor.md): rules/knowledge/audit ownership, shared input logs, atomic steps, player sessions and query sidecar16. This later change preserves the baseline's snapshot and public projection formats.
- [Development commands](game-engine-development.md): bootstrap, doctor, environment and focused checks.
- [Verification workflow](game-engine-verification-workflow.md): exact coverage omissions, lock ownership and checkpoint cadence.
- [Runtime invariants](game-engine-invariants.md): read the sections affected by a rules, state, privacy or recovery edit.
- [Catalog workflow](game-engine-catalogs.md): source facts, provenance, compilation and generic match commands.
- [Historical index](history/engine/index.md): previous onboarding, release contracts, unchanged evidence paths and recovery.
- [Storage policy](game-engine-storage.md): budgets and bounded retention; ignored output is not automatically disposable.
- [Build/cache policy](game-engine-builds.md): version authority, source guards, cache selection and measured invalidation costs.
- [Tiny runtime smoke](game-engine-smoke.md): fixed seedless probe, small failure artifact and explicit replay; not full acceptance.
- [Branch/worktree lifecycle](game-engine-worktrees.md): safe checkpoints, integration and preservation of unique work/evidence.

The long [progress ledger](game-engine-progress.md) is chronological evidence,
not the onboarding sequence. The [plan](game-engine-plan.md) and
[specification](game-engine-spec.md) contain broader goals; use this map and the
package manual to distinguish implemented finite behavior from planned scope.
