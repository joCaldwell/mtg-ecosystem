# Game Engine (Layer 1)

This directory contains the first Rust semantic compiler, deterministic scenario
kernel, executable contracts, and native scenario CLI. General Commander match
support, search, and the bulk Card IR pipeline remain later work.
The current P2 slice adds budget suspension, resumable query work, private
observations with scoped handles, permitted SearchContext, and recovery during
payment/resolution. See the [P2 slice contract](../../docs/game-engine-p2-slice.md).
The next [bounded departure slice](../../docs/game-engine-departure-slice.md)
adds physical Commander history, surviving turn/priority state, and ordered
concessions with recovery through casting and resolution.
The [event slice](../../docs/game-engine-event-slice.md) adds atomic ordered and
independent simultaneous batches, pre-event facts, typed occurrence results, and
off/compact/detailed tracing with equal semantic hashes.
The [instant-response slice](../../docs/game-engine-stack-slice.md) adds a third
exact profile with four physical supported spells, public stack order, prior-pass
restoration, and choice-free resolution after a source leaves the stack.

The [casting mana slice](../../docs/game-engine-payment-slice.md) adds a caster-only
window after targets, immediate intrinsic mana activation, and keep-all/undo-all
choices on failed payment. A fourth exact profile uses two source-backed Forests
to demonstrate unpayable U costs and preservation of pre-cast mana.

The compiler now emits five frozen source definitions and runs a four-seat
land-mana / targeted return-to-hand scenario with strict admission, serializable
announcement, owned/incarnation identities, events, restore, fork, and replay.
The [turns slice](../../docs/game-engine-turns-slice.md) adds a deterministic
four-seat rotation, explicit phase/step entry, basic-land plays, and targetless
creature casting through the existing mana/payment/stack paths. The
[combat games slice](../../docs/game-engine-combat-games.md) adds ordinary
multiplayer combat, simultaneous damage and required SBAs, ordered draws, cleanup
discard, and rules-defined elimination. Seven exact configurations are admitted.
Two deterministic small games finish under player-facing policies: an eight-turn
combat/response game and a four-turn failed-draw game. Additional witnesses cover
multiple blockers and simultaneous losses.

Legal Commander matches, command-zone casting/return/tax/damage, keywords,
triggers, replacements/prevention, continuous effects, shuffling and arbitrary
deck admission remain later work:

- [Implementation plan](../../docs/game-engine-plan.md): milestones, exit evidence, tests, search benchmarks, and open decisions.
- [Engine specification](../../docs/game-engine-spec.md): compiler/IR boundary, state, decisions, rule execution, observations, replay, and branching.
- [Language decision](../../docs/runtime-language-decision.md): adopted languages, workspace policy, and TypeScript/Rust boundaries.
- [First scenario brief](../../docs/game-engine-first-scenario.md): frozen inputs, decisions, expected behavior, and restrictions.
- [Implementation evidence](../../docs/game-engine-progress.md): completed checks, artifact identities, and remaining gates.
- [Package manual](AGENTS.md): commands and development invariants.

From the repository root, install Node development dependencies with `npm ci`
and use Rust 1.99.0:

```sh
npm run check:engine
cargo test --workspace --locked --release
cargo run --locked -p mtg-sim -- demo
cargo run --locked --release -p mtg-sim -- combat-games
cargo run --locked --release -p mtg-sim -- demo --bundle generated/p2-bundles
```

`mtg-sim compile` emits the fixture catalog; `schema` exports its generated
JSON Schema; `report` includes the paired unsupported source definition;
`verify-bundle <file>` recompiles retained records and checks recovery/replay
against their pinned build. The demo bundle includes recovery checkpoints for the original and departure
scenarios, ordinary/detached response runs, and casting-time
mana success/keep/undo runs, and the eight-card turns witness in all three trace
modes, plus four combat policy witnesses (thirty-six mode executions across
seven configurations). `combat-games --recover` additionally restores every
accepted-input and automatic execution boundary. Combat recovery records retain
complete journals plus boundary/digest witnesses; offline verification reconstructs
and restores every exact semantic boundary with diagnostics off, then restores
and compares the entire detailed final trace against detailed replay. Independent
continuation/departure tests also restore detailed snapshots. This avoids storing
or repeatedly serializing redundant growing histories at every game boundary.
Native trace evidence is separate from semantic state and remains trusted host
data; only detailed mode retains the flat `events()` compatibility stream. `observe <player>` and `context <player> <seed>`
emit player-facing projections of the initial scenario. They do not emit the
authoritative bundle, snapshots, or internal IDs.

`npm run retain:engine` captures a release bundle and complete vendored dependency
closure outside the worktree, rebuilds with `cargo --frozen` in an empty Cargo
home/target directory, and verifies replay with the rebuilt executable. The
capture phase may download locked dependencies; the rebuild is offline. It also
retains the official pinned rules bytes from `.scryfall-cache/rules-20260925.txt`
(or `MTG_ENGINE_RULES_PATH`), requiring the SHA-256 recorded in the first scenario
brief. Download the linked official September 25 rules before a fresh retention
capture; runtime and replay never fetch rules.
`npm run build-ir` still rejects the
unimplemented bulk pipeline.

The test profile is optimized at level 2 with debug assertions and overflow checks
enabled. Bundle v9 records the effective optimization/check settings for its build
identity and offline rebuild. Snapshots remain v8 with the same ordered history
and trace arrays. `SnapshotVerifier` verifies ordered checkpoint sequences using
one independent forward replay while applying the complete standalone restore
checks at every boundary. Histories share immutable chunks across native forks;
branch mutations remain independent.
