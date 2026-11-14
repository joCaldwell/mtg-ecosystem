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
admits finite legal headless matches for a reviewed damage-trigger pool. Its separate
[v2 profile](game-engine-damage-match-profile-v2.md) extends that pool;
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
| Later finite trigger extensions | Separate capabilities cover [opponent damage](game-engine-opponent-damage-triggers.md), [optional draws](game-engine-optional-draws.md), [event-player discard](game-engine-event-player-discard.md), [event-player sacrifice](game-engine-event-player-sacrifice.md), [filtered sources](game-engine-filtered-damage-sources.md), [controller discard](game-engine-damage-controller-discard.md), [damage-quantity draws](game-engine-damage-quantity-draw.md), and [attacking keywords/Snow](game-engine-attacking-creature-keywords.md) (originally developed across IR26–IR33). The subsequent [event-time appearance correction](game-engine-damage-match-appearance.md) preserves their semantic contracts. Their focused validation does not renew combined acceptance or expand hosting/search. |
| Later digest reuse and November tooling changes | Separate focused-work evidence, not part of the frozen D2c-d gate; see [trace digests](game-engine-verification-trace-digests.md) and [fast checks](game-engine-fast-checks-20261109.md) |

At the platform baseline trusted snapshot/bundle formats were 19/20 and public
projection/context/query-sidecar formats were 16/16/15. The architecture refactor
retains snapshot/bundle/public formats but advances query work sidecars to 16
and adds one charged initialization unit; old sidecars reject. `mtg_contracts::CURRENT_IR_VERSION`
is the current IR authority; see [version and build checks](game-engine-builds.md).
A separate [intrinsic indestructible foundation](game-engine-indestructible.md)
adds complete Myr/Relic source support without pool or consumer admission.
Its separately bounded equipped-creature extension supports complete Darksteel
Plate and common-prior keyword authority for simultaneous destruction, including
destructible typed grant controls. It uses schema 1 and the existing
`indestructible_v1` capability. That source slice preserved catalogs and
host/search sets; later headless admission is qualified separately below.
Its source-qualified evidence is identified in that contract, separately from
consumer acceptance.
Completed SBA checks consume deathtouch observation windows; ordinary damage
marks remain until cleanup. Later protection loss can expose lethal marked
damage, while a completed nonlethal deathtouch window cannot cause another death.
The separate `temporary_indestructible_spell_v1` capability supports complete
Withstand Death source as an ordinary single-creature indestructible grant.
Its resolved effect captures an exact incarnation and expires atomically with
damage cleanup; intrinsic and equipped protection still union normally.
This source foundation adds no pool, hosting/search or consumer admission.
The separately fenced `temporary_flying_spell_v1` foundation supports complete
Jump source through the same exact-incarnation keyword union and cleanup.
`compound_indestructible_spell_v1` supports complete Mortal's Resolve as exactly
+1/+1 then indestructible on one shared target; the pure-grant contracts remain
unchanged. Both foundations are scoped in the [keyword contract](game-engine-indestructible.md).
Damage-source LKI admits validated spell grants while replay authenticates the
historical appearance; the guard retains duplicate and departure invariants.
The earlier combined Plate admission, blue default and temporary-grant source at
`b19498657bf73402bcfe559017e0956eb5b696fe` has copied producer
`9cb71fca155ea489d4d204ad77e0236c99e78ea1af44ccaf7500cabb3a434555`,
with all 750 native source rows verified. Its focused combined pass ran 84 tests,
workspace all-target compilation, production compiler/core/simulator Clippy,
catalog freshness and CLI smoke/replay; the exported schema matches the
temporary-grant producer. Exact copies and receipts are retained under
`generated/parallel-round/native01/`. The blue full-game witness remains
qualified to its separately named producer; no combined full-completion,
release/profile/runtime matrix or downstream consumer qualification is claimed.
The only active IR schema is 1 while there are no external consumers; new
features evolve it in place with capability validation and exact build fencing.
Current in-place support covers [grouped power-qualified damage and opponent
casting prohibition](game-engine-grouped-damage.md); its capability scope is
validated independently of schema numbering.
The [controlled-creature damage replacement](game-engine-controlled-damage-replacement.md)
extension uses the same unreleased schema with separate current/LKI source facts.
The [blocking-target extension](game-engine-blocking-targets.md) adds complete
Righteousness lowering, authoritative blocker status and payment-time target
binding correctness. Its subsequent [headless development profile](game-engine-blocking-matches.md)
adds explicit separate-pool admission and public-view policy support, with a
paid accepted-pod combat/replay witness and one fixed-index completion game.
The Ertai blue-default addition preserved the then-45-definition pool. Its
separate fixed-index game finished naturally and replayed, with paid blue
cast/activation coverage qualified in that contract. Subsequent Withstand Death
and Jump admission brings the blocking pool to 47 definitions: each black/green
default replaces one Forest with protection (39 Forests), and blue replaces one
Island with Jump (89 Islands). Conservative public-IR policy responds to supported
threats or lifts an actual attacker before blockers. Paid legal-pod prefixes verify
protection/combat/cleanup/fresh destruction and flight/blocking/commander damage,
including atomic rejection, snapshots and replay. Hosting/search, consumer
qualification and the earlier pools remain separate.
Current development also supports [tapped-permanent target filters](game-engine-tapped-targets.md),
with exact Swift Response and Assassinate source fixtures. The existing headless
development pool now admits both; hosting and search capability sets remain unchanged.
Separate [ordinary permanent untapping](game-engine-untap-permanents.md) supports
complete Burst of Energy lowering and genuine source-backed untap/revalidation
witnesses. Its subsequent headless admission adds a paid accepted-pod defensive
response witness and narrowly scoped public-view policy support.
Complete [Shrapnel Blast source](../packages/game-engine/tests/fixtures/d3c623f3-c7c9-47c0-8096-6dee3267baff.json)
now lowers its exact one-artifact additional sacrifice cost through the existing
[sacrifice/payment contracts](game-engine-sacrifice.md). This compiler source
completion adds no IR fields, capabilities or schema version. The subsequent
finite sacrifice-pool admission includes it in the red default deck; hosting,
search and downstream consumer qualification remain separate.
The subsequent [indestructible sacrifice-pool admission](game-engine-sacrifice.md#profile)
adds complete Myr/Relic data and a singleton red-default Relic, with a genuine
paid Shrapnel sacrifice/replay prefix and conservative public-keyword destruction
policy. Subsequent Darksteel Plate admission brings that pool to 159 definitions
and replaces one white-default Plains with Plate. Its public-IR equip policy
retains equal-value protection and moves only for useful improvement, with a
paid combat/snapshot/replay prefix and separate synthetic policy probes.
Source-qualified headless evidence remains separate from hosted/search
and downstream consumer qualification.
The [ordinary tap primitive](game-engine-untap-permanents.md#creature-tap-and-controller-draw)
supports complete Pressure Point lowering and creature-tap/draw execution, with
explicit scope validation. Its subsequent headless admission adds Pressure Point
to the target-status development pool; hosting/search and consumer qualification
remain separate.
Ordinary [permanent return to owner's hand](game-engine-removal.md#permanent-owner-hand-movement)
supports complete Regress and Disperse sources through the existing move and
Commander-replacement path. A separate capability fences this broader target
domain from the existing creature-bounce hosting/search support. The later
[headless sacrifice-pool admission](game-engine-sacrifice.md#profile) adds both
spells to the blue default deck; hosting/search and consumer qualification remain
separate. A separately fenced
[activated artifact tap](game-engine-untap-permanents.md#activated-artifact-tap)
source/runtime slice supports complete Icy Manipulator through existing payment,
targeting and tap events. Its later [headless admission](game-engine-blocking-matches.md#current-activated-tap-admission)
adds paid activation/combat/replay evidence and public-IR policy that avoids
already-tapped targets. Hosting/search and consumer qualification remain separate.
A separate [tap-source destruction](game-engine-removal.md#tap-source-destruction)
slice supports complete Royal Assassin and pays battlefield-targeted tap costs
through the existing activation completion path, without pool or consumer
admission.
A separate [activated spell-counter foundation](game-engine-stack-costs.md#activated-spell-counter-foundation)
supports complete Ertai, Wizard Adept with stack-target routing, composite
payment and invocation/snapshot validation. Their subsequent
[headless interaction admission](game-engine-blocking-matches.md#current-activated-interaction-admission)
adds Royal to black/green defaults and permits Ertai as a blue commander, with
paid legal-pod and commander-return/replay proofs. Hosting/search and consumer
qualification remain separate.
[Activated permanent return](game-engine-removal.md#activated-permanent-return)
supports complete Temporal Adept through existing cost and movement paths, with
private ownership, self/source bounce, reentry and Commander/replay witnesses;
its later [headless admission](game-engine-blocking-matches.md#43-definition-permanent-return-admission)
adds a paid legal-pod owner-choice and hand-recast witness. Hosting/search and
consumer qualification remain separate.
A separately fenced [tap-source creature untap](game-engine-untap-permanents.md#tap-source-creature-untap)
slice supports complete Seeker of Skybreak through existing activation payment,
creature-filtered permanent targets and untap events, with private casting,
natural-aging, mana reuse, self-target and source/target departure replay proofs.
Its later [headless admission](game-engine-blocking-matches.md#current-tap-source-untap-admission)
adds Seeker and Llanowar to black/green defaults, with a paid mana-reuse pod and
public-IR policy excluding untapper cycles. Hosting/search and downstream consumer
qualification remain separate.
Read Rust types and validation at the edited source; old reports retain their
original version numbers. Active catalogs are regenerated at schema 1; distinct
source/admission pools remain separate. Historical decoder gates and downgrade
matrices have been removed; source archives and external evidence remain pinned.
No new quiet P7 performance, offline rebuild, full-profile or combined-acceptance claim follows from a focused check.

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
