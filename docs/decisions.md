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
| Cargo alongside npm workspaces | Adopted 2026-10-11; initial scaffold implemented 2026-10-13 | Separate package toolchains while preserving shared versioned JSON contracts |
| Commander as the only game product format | User requirement, 2026-10-11 | Build multiplayer and Commander mechanics into the foundation; four-player headless pods are the first planned delivery |
| Canonical executable definitions with separate printing/set indexes | Proposed refinement of per-set storage | Keep Oracle semantics separate from reprints and preserve language-neutral artifact contracts |
| Top-level Multiverse application owns account UX, navigation, browser features and process composition | Adopted 2026-11-14; PR #24 merged | Keep product ownership outside the deck-builder feature; preserve the existing account/auth/Actor/SQLite backend behind public host and type-only contracts |
| Shared game display and separate qualified preview adapter | Implemented 2026-11-14 | Both consumers use `game-ui`; the client preview is router-free, style-scoped and independent of native generation, with build-time fixture provenance checks |
| First product game journey: one human and three automated opponents | Owner direction, 2026-11-14 | Home and real owned-deck setup precede a local sample preview; real launch still needs admission, account-to-seat provisioning and complete browser decisions |

The language stack is final. Its component assignments and boundaries are in
[runtime-language-decision.md](runtime-language-decision.md). Execution design
and delivery evidence remain in [game-engine-spec.md](game-engine-spec.md) and
[game-engine-plan.md](game-engine-plan.md). Performance measurements guide
optimization within the adopted stack.

The engine/server now support reviewed finite Commander matches, sampled search
and local hosting; [the current map](game-engine-current.md) owns exact scope.
Bulk per-set IR emission and arbitrary-deck product launch remain outside that
acceptance. The [first scenario brief](game-engine-first-scenario.md) preserves
the initial source/rules inputs and Rust schema authority as historical context.
Card IR remains versioned JSON data. The [application architecture](multiverse-app.md)
separately defines product, backend, game display and session ownership.

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
