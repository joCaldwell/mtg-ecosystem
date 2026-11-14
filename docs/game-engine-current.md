# Current engine map

Start here for engine/server work. The engine executes finite Commander matches,
sampled-world search on a reviewed subset, and local Rust hosting with a generated
TypeScript SDK. It is not arbitrary-deck Commander or a complete Magic rules engine.
The deck library uses independent Scryfall/SQLite data and does not consume Card IR.

## Authorities and navigation

| Area | Authority / manual |
| --- | --- |
| Source parsing/archive | TypeScript `packages/oracle-parser`; [manual](../packages/oracle-parser/AGENTS.md) |
| Executable IR and complete-source lowering | Rust `crates/contracts` and `crates/compiler`; [catalog workflow](game-engine-catalogs.md) |
| Deterministic rules, decisions, visibility, replay | Rust `crates/core`; [invariants](game-engine-invariants.md), [mechanics](game-engine-mechanics.md) |
| Public-view policies and native sampled search | Rust `crates/sim`; [search contract](game-engine-search.md) |
| Admission pools | `tests/fixtures/catalogs` and `profiles`; [profiles](game-engine-profiles.md) |
| Hosting and generated protocol | `packages/game-server`; [host contract](game-engine-server.md), [SDK manual](../packages/game-server/sdk/AGENTS.md) |
| Environment, caches and execution | [Development](game-engine-development.md), [verification](game-engine-verification-workflow.md), [storage](game-engine-storage.md) |

Paths under `crates/` and `tests/` above are relative to `packages/game-engine`.
Core imports contracts at runtime, never compiler/parser, I/O, clocks or OS randomness.
All admitted catalogs use common rules and catalog-independent structural capacities.
Profiles request consumers through reviewed capability sets, never name selectors.
Foundations and attachments are hosted; only foundations is search-enabled.
Later damage/blocking/sacrifice development admissions do not broaden those sets.

## Supported behavior and remaining work

[Mechanics](game-engine-mechanics.md) defines the supported families and rejection
boundaries. Frozen fixtures/manifests and strict Rust validation resolve exact
card membership. Parameterized operations execute complete source semantics;
card names, quantities and IDs never dispatch rules or strategy.

The only active Card IR schema is **1** (`mtg_contracts::CURRENT_IR_VERSION`).
The shared temporary-keyword family supports reviewed unique combinations of Flying,
Deathtouch, Indestructible, Lifelink and Hexproof, optionally preceded by a fixed
signed stat modifier. Targets may be ordinary creatures or creatures controlled
by the invocation controller; old unfiltered subset requirements and consumer/pool
admission remain unchanged. Current inputs do not provide a control-changing
effect, so same-incarnation dynamic-control scenarios remain unqualified.
The bounded [Ramos basic cascade family](game-engine-ramos-casting-contract.md#m3b-complete-basic-cascade-checkpoint)
now lowers complete frozen Agent/Bloodbraid sources and executes owned resolution
casting with self-cast triggers, sequential public exile and private random bottom.
It is private source qualification; full M3 and profile/consumer admission remain
open. Private snapshot 23 is reserved for its owned frame; public/context/query
forms remain 18. Snapshot 22 belongs to the independent Aura lane; parent
integration remains a separately qualified source.
Features evolve in place with strict capability/shape validation and exact build/
catalog fencing; old historical IR numbers describe earlier evidence only.
Snapshots, bundles, public projections, query sidecars, AST archives and protocol
schemas have independent identities. See [build/version policy](game-engine-builds.md).

The product owns account/session UX, Home and owned-deck setup. The table preview
is a qualified captured sample. Standalone client acceptance includes authenticated
joining and opening choices; full decisions, real automated launch, account-to-seat
provisioning and arbitrary library-deck admission remain unfinished. Product
sign-in is not Rust seat authorization. See [application](multiverse-app.md) and
[client](game-client.md) contracts; ignore legacy solo playtester guidance.

## Evidence boundaries at the platform baseline

The platform baseline was `c62777b`; integrated engine/platform work is on `main`.
Start new independent work from current main, not former `feat/engine-plan`.
Acceptance is source-qualified, never inherited by later code:

| Boundary | Status |
| --- | --- |
| D2c-c `a7f5571` | Accepted ordinary cast triggers at historical IR23 |
| D2c-d `394301d` | Accepted declared-attack triggers at historical IR24, including reviewed demo/Bundle lifetimes |
| Later D2c-e/IR25 and finite trigger/keyword/interaction additions | Candidate/focused source qualification; no renewed combined acceptance or P7 performance claim |
| Schema-1 reset | Regenerated 29 active pools, 1,357 definitions unchanged; focused checks only |
| Latest controlled temporary-keyword family | Source `b505e32cfb29bbfdcf5183b7480fbbd41a750ce1`; 794 compiled rows; focused qualification only, with exact producer and omissions in evidence guide |
| Client/hosting consumers | Retain their own exact producer/schema/catalog/build qualification, separate from current engine HEAD |

[The evidence guide](game-engine-evidence.md) retains exact source/digest and proof
locations, historical failures and omissions. At the platform baseline trusted
snapshot/bundle formats were 19/20, public projection/context/sidecar 16/16/15.
The architecture refactor preserved snapshots/public formats and advanced query
work sidecars to 16 with one charged initialization unit; old sidecars reject.
Read the source constants before changing any current format.

Historical plans/transcripts are recoverable from Git; they are not current
operating instructions. A focused pass is not a full-engine release, full-game
browser acceptance or a promise that every game fits resource ceilings.
