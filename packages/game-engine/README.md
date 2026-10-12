# Game Engine (Layer 1)

This directory is planned for the Rust semantic compiler, deterministic MTG
rules engine, and native simulation/search runner.
It will consume generated, language-neutral Card IR from `packages/card-data/`.

No engine code, executable Card IR, or package toolchain is implemented yet.
The current plans target Commander only, prioritizing four-player headless
simulation and agent search. Commander tax, damage, command-zone rules,
multiplayer priority, and player departure are foundation requirements:

- [Implementation plan](../../docs/game-engine-plan.md): milestones, exit evidence, tests, search benchmarks, and open decisions.
- [Engine specification](../../docs/game-engine-spec.md): compiler/IR boundary, state, decisions, rule execution, observations, replay, and branching.
- [Language decision](../../docs/runtime-language-decision.md): adopted languages, workspace policy, and TypeScript/Rust boundaries.

Rust is adopted for the compiler, core, simulator, and native agent policies.
The TypeScript parser supplies the versioned source archive. Freeze the P0
scenario/source inputs before creating the Cargo scaffold; language selection
is complete. Browser-local rules execution uses this same Rust core compiled
to WebAssembly when that feature is delivered.
