# Language decision for Layers 0–3

Status: **adopted, 2026-10-11**. The language assignments below are final for
implementation. The initial compiler/engine/scenario slice now exists; full
simulation/search and server implementation remain future work. See the
[first scenario brief](game-engine-first-scenario.md).
Primary workload: **high-volume headless simulation and agent search for
Commander**, followed by interactive Commander hosting.

Related documents: [engine specification](game-engine-spec.md),
[implementation plan](game-engine-plan.md), [architecture](architecture.md),
and [decision record](decisions.md).

## 1. Adopted languages by layer

| Layer / component | Language and runtime | Responsibility |
| --- | --- | --- |
| Layer 0 — ingestion, Oracle parser, AST archive/export | TypeScript on Node.js | Preserve and extend the existing tested parser and source-data pipeline |
| Layer 0 — semantic compiler and Card IR emitter | Rust | Own executable binding, semantic validation, lowering, and capability requirements; share contracts with the core |
| Layer 0 — card-data artifacts | Versioned JSON data | Language-neutral definitions and indexes; no executable source or separate runtime |
| Layer 1 — rules engine | Rust | Authoritative deterministic rules, state, decisions, events, observations, replay, and branching |
| Layer 1 — simulation, search, benchmarks, native agent policies | Rust | Keep repeated queries, forks, rollouts, and policy execution in process |
| Layer 2 — game server and authoritative agent/session orchestration | Rust | Call the core directly; own sessions, persistence, clocks, networking, and remote-agent coordination |
| Layer 3 — browser game client, application UI, generated client SDK | TypeScript, with HTML/CSS for presentation | Consume the versioned server protocol and render permitted observations |
| Layer 3 — deck-builder, including its backend and MCP tools | Existing TypeScript on Node.js and in the browser | Continue the standalone application without migration |
| Browser-local rules/search module | The same Rust core compiled to WebAssembly | Reuse authoritative rules in the TypeScript browser client when this capability is delivered |
| Repository ingestion/reporting and cross-package command glue | TypeScript on Node.js | Coordinate the source pipeline and invoke native build/compiler tools at process boundaries |

Tests and fixtures follow their implementation: Rust tests for native compiler,
core, simulation, and server code; TypeScript tests for parser and application
code; shared JSON fixtures verify interchange. External agents use the versioned
protocol. Their implementation is outside this repository's language policy.

## 2. Architecture rationale

The simulation loop owns state branching, legal-choice queries, rules evaluation,
and repeated transitions. Rust keeps those operations together with explicit
ownership and data layout, shared immutable definitions, and one mutable owner
per match. The Rust server links that same core directly and schedules CPU work
apart from network I/O. Browser-local execution compiles the same rules code to
WebAssembly.

TypeScript retains the existing parser and deck-builder investment and provides
the browser application and client SDK. Parsing and semantic compilation run
offline, so their versioned artifact boundary stays outside simulation loops.
No rewrite of the parser or deck-builder is part of engine development.

Rust ownership provides compiler-checked control of memory and mutation; see
[the ownership model](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html).
The architecture still requires careful algorithms, correctness evidence, and
measured optimization. There are no engine benchmarks yet, and this decision
makes no quantitative throughput or memory claim.

## 3. Workspace and toolchain policy

Use a Cargo workspace alongside the existing npm workspaces. Rust owns the
compiler/core/simulator crates under `packages/game-engine/` and the server
under `packages/game-server/`. Existing TypeScript package commands remain in
place. Exact crate boundaries follow the
[source organization](game-engine-spec.md#9-proposed-source-organization).

Pin a stable Rust toolchain and dependencies when the native workspace is
scaffolded; retain the lockfile and build settings in reproducibility bundles.
Pin the supported Node.js version through the existing TypeScript package
workflow. Wire root checks to the implemented workspaces when Cargo exists;
do not report native checks as passing before there is native code to check.
Framework, library, schema-generator, and transport selection are implementation
details within this adopted stack. The milestones do not include language
selection; they deliver and verify the assigned components.

## 4. Rust implementation constraints

- Use stable typed IDs and owned tables for game objects, rather than a web of
  references with lifetimes spanning mutations. Separate physical-card IDs,
  zone-incarnation IDs, stack IDs, and observation handles.
- Share immutable card definitions across games. Keep mutable match state owned
  by one executor. Parallelize independent games or branches before attempting
  parallel evaluation within a match.
- Keep serialized continuations as data. Borrowed closures and live call stacks
  cannot be the save/replay format.
- Start with a clear clone-based branch implementation. Optimize with chunked
  copy-on-write or reversible mutations only after profiling and correctness
  tests. Rust does not make a large deep clone inexpensive automatically.
- Prefer exhaustive enums and explicit errors. Do not use `unsafe`, dynamic
  code loading, or custom allocators in the first implementation.
- Choose dependencies individually. “Zero dependencies” in the parser manual
  is a parser constraint, not a requirement imposed on the Rust workspace.
- Pin the Rust toolchain, dependencies, and build profile in benchmark results.
  Keep integer semantics identical in debug and optimized builds. Cargo profiles
  can change optimization and overflow-check behavior; use explicit arithmetic
  policies, not build-mode assumptions. See [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html).

Safe Rust supports shared immutable data and synchronized ownership, but its
threading facilities do not make the order of concurrent game mutations
deterministic. The one-owner-per-match design is a project choice. See
[Rust shared-state concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html).

## 5. Parser-to-Rust boundary

The parser stays TypeScript. Export an explicitly versioned
parser artifact containing source metadata, faces, parsed ASTs, source
provenance, and diagnostics. The existing archive is nonexecutable; a compiler
may consume its schema-3 records through a version-pinned adapter, but must
never treat `status: parsed` or `fullyBound` as execution certification.

The production semantic compiler owns executable binding and lowering. The
current TypeScript reference analyzer remains useful evidence and diagnostics;
it is not a second independently maintained execution semantics implementation.
The compiler can reuse its resolved graph only after validating its version,
source association, and meaning against the supported compiler contract.

The initial production compiler binds directly from the supported source
AST subset; keep the existing graph as diagnostic comparison evidence. Reuse
of graph edges is an optional later optimization with contract tests, not an
implicit second authority. Do not port every current TypeScript binding case
before the first source-to-runtime scenario.

Adopt explicit discriminators and a machine-readable interchange schema with
cross-language fixtures. Source AST import types and executable IR types are
separate. A source construct can be typed yet unsupported by the compiler.
Reject unknown semantic fields and variants rather than silently dropping them.
Serde supports [tagged enum representations](https://serde.rs/enum-representations.html);
its [container attributes](https://serde.rs/container-attrs.html) document that
unknown JSON fields are ignored by default and that `deny_unknown_fields` is
incompatible with `flatten`. Decoder design must account for those limitations.

Preserve complete source objects as provenance while strictly validating their
consumed fields; additional unrelated Scryfall metadata is not an unknown IR
operation. The schema-3 adapter must verify its existing serialization/hash
rules with TypeScript/Rust fixtures. New IR canonicalization is a different
versioned contract, not a replacement for archive hash verification.

The compiler records required intrinsic rule families and ability functioning
zones as well as parsed Oracle behavior. Runtime characteristics determine
dynamic intrinsic abilities. A lowered artifact is not executable admission:
the engine validates the whole scenario/pod against a selected build/profile.
See the [compiler contract and examples](game-engine-spec.md#34-intrinsic-behavior-and-functioning-zones).

Use one schema authority per contract; choose the initial IR schema/type
generation path at P1a after reviewing the examples. Source AST, IR, internal
snapshots, and external client protocol need separate identities. Protocol
generation waits for its consumer at P8. Shared mechanic descriptors live below
compiler and core in the dependency graph; no runtime dependency on the parser
or compiler, and no compiler/core import cycle.

Read JSON at artifact/network boundaries; load it once into validated runtime
structures. Do not deserialize an AST or look up card text during each action.
Binary caches are optional derived artifacts, keyed by IR and engine version.

## 6. Native, browser, and agent integration

The Rust simulation runner and native policies link the core and keep state and
card tables in process. The Rust server calls that same core directly. Remote
agents and TypeScript applications submit decisions over the versioned protocol;
server coordination of external inference stays outside authoritative rule
execution. Use coarse batch operations for remote simulation consumers rather
than network/JSON crossings for every internal search step.

Private-information agents receive SearchContexts and validated sampled worlds,
not authoritative hidden-state handles. The first search profile proposes open
decklists and priority-boundary sampling; unknown-list inference and private
continuation reconstruction remain later feature work. Information boundaries
are identical for native and browser-local execution.

The browser application is TypeScript. Its local rules/search module uses a
WebAssembly build of the Rust core. Delivery is deferred until a browser-local
consumer exists, but the language and reuse boundary are settled. Keep the core
free of network, filesystem, wall-clock, and OS-randomness dependencies. The
[wasm-bindgen type guide](https://wasm-bindgen.github.io/wasm-bindgen/reference/types.html)
describes the Rust/JavaScript boundary. Keep repeated search work inside the
compiled module and measure marshalling separately. Bindings must specify handle
lifetime, cancellation, errors, and concurrent access.

## 7. Performance validation and delivery

Measure the one production implementation. P1b measures the executable
source-to-runtime slice; P2 measures branching, queries, memory, serialization,
and budgeted execution. P3/P4 add event, replacement, and Commander lifecycle
costs. P5 supplies complete-game results; P6.n adds named interaction profiles.

At P7 evaluate the named
[end-to-end search workload](game-engine-plan.md#named-end-to-end-search-workload),
including permitted-context sampling, candidate generation, fork/apply,
rollout, aggregation, and disposal. Record consumer latency/memory/useful-trial
budgets before optimization. Retain exact source/build/toolchain identities,
fixtures, seeds, raw samples, and reproducibility bundles. Use optimized native
builds and measure protocol/browser boundary costs for their actual consumers.

Investigate algorithmic complexity, copying, allocation, data layout, caches,
and batching when budgets are missed. Benchmarks guide implementation and
capacity within the adopted stack. Delivery dates and support
claims depend on the implementation plan's correctness and performance evidence.

The adopted language decision is complete. The first implementation brief freezes
P0 scope, source snapshots, consumer workload, and reference environment. The
initial native compiler/runtime scenario exists; remaining delivery gates stay
in the implementation plan.
