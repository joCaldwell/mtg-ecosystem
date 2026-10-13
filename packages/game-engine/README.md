# Game Engine (Layer 1)

This directory contains the first Rust semantic compiler, deterministic scenario
kernel, executable contracts, and native scenario CLI. General Commander match
support, search, and the bulk Card IR pipeline remain later work.

The first slice compiles four frozen source definitions and runs a four-seat
land-mana / targeted return-to-hand scenario with strict admission, serializable
announcement, owned/incarnation identities, events, restore, fork, and replay.
It admits only the reviewed scenario; it cannot start a legal Commander match.
Commander tax, damage, command-zone rules,
multiplayer priority, and player departure are foundation requirements:

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
cargo run --locked --release -p mtg-sim -- demo --bundle generated/p1-bundles
```

`mtg-sim compile` emits the fixture catalog; `schema` exports its generated
JSON Schema; `report` includes the paired unsupported source definition;
`verify-bundle <file>` recompiles retained records and checks recovery/replay
against their pinned build. Replay bundles contain source files and dependency
lockfiles, not vendored dependencies; offline rebuilding in a clean dependency
store remains a P2 retention check. `npm run build-ir` still rejects the
unimplemented bulk pipeline.
