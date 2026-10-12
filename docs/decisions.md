# Architecture decisions

## Current decisions

| Decision | Status | Rationale |
| --- | --- | --- |
| Hand-written TypeScript tokenizer and recursive-descent parser | Accepted 2026-08-01; reaffirmed 2026-10-11 | Build typed nodes directly, keep context-sensitive grammar explicit, reject unsupported text |
| Structural correctness before coverage | Accepted; reinforced 2026-09-05 | Acceptance alone cannot detect lost logic, event grouping, ownership, or scope |
| Typed supported AST, future semantic validation/lowering | Accepted direction | Parsing success is distinct from resolving references and proving engine capability |
| Per-set JSON IR | Proposed storage direction; emitter unimplemented | Decide printing membership, versioning, reference binding, and update policy before emission |
| Rust semantic compiler, engine, native simulation/search, and native agent policies | Adopted 2026-10-11 | Keep executable contracts and the simulation loop together with explicit ownership and data layout |
| Rust game server and authoritative agent/session orchestration | Adopted 2026-10-11 | Direct core calls with one rules implementation; isolate CPU work from network scheduling |
| TypeScript ingestion/parser/archive, deck-builder, browser applications, and client SDK | Adopted 2026-10-11 | Retain existing implementations and use TypeScript at the browser/protocol boundary |
| Browser-local rules/search from the Rust core compiled to WebAssembly | Adopted 2026-10-11; feature delivery deferred | Reuse the same rules implementation inside TypeScript browser applications |
| Cargo alongside npm workspaces | Adopted 2026-10-11; native scaffold pending | Separate package toolchains while preserving shared versioned JSON contracts |
| Commander as the only game product format | User requirement, 2026-10-11 | Build multiplayer and Commander mechanics into the foundation; four-player headless pods are the first planned delivery |
| Canonical executable definitions with separate printing/set indexes | Proposed refinement of per-set storage | Keep Oracle semantics separate from reprints and preserve language-neutral artifact contracts |

The language stack is final. Its component assignments and boundaries are in
[runtime-language-decision.md](runtime-language-decision.md). Execution design
and delivery evidence remain in [game-engine-spec.md](game-engine-spec.md) and
[game-engine-plan.md](game-engine-plan.md). Performance measurements guide
optimization within the adopted stack.

No Cargo workspace, executable compiler, engine, or server exists yet. Adoption
fixes implementation direction; each component still needs its planned
correctness and performance evidence. Card IR remains versioned JSON data.

The current implementation and invariants are documented in
[oracle_parser.md](oracle_parser.md). The source of truth for the experimental
AST is `packages/oracle-parser/src/ast.ts`. Generated IR is not committed.

The 2026-09-05 foundations pass preserves filter conjunction/disjunction,
trigger event grouping and zone endpoints, effect destinations and controllers,
and shared modifier durations. It replaces raw activation restrictions with
typed supported forms, rejects unconsumed modal options, enforces cursor
rollback, retains original parse diagnostics, and adds reproducible acceptance
reporting and atomic cache replacement. Reduced acceptance from rejecting
unsupported constructs is an intentional correctness improvement.
