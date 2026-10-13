# Compiler and engine development

Rust owns contracts, offline compilation, deterministic core, and the scenario
CLI. Read `docs/game-engine-first-scenario.md` for frozen inputs and restrictions.
This package is scenario-only; do not advertise playable Commander matches.

Run from repository root, using pinned Rust 1.99.0:

- `npm run check:engine`: toolchain check, formatting, Clippy, Rust tests, and
  the Node cross-language fixture contract check.
- `cargo test --workspace --locked` and `cargo test --workspace --locked --release`.
- `cargo run --locked -p mtg-sim -- demo`: verified source-to-runtime demonstration.
- `cargo run --locked -p mtg-sim -- schema`: export the executable JSON Schema.
- `npm run retain:engine`: retain source/dependency closure and verify an offline
  rebuild/replay with a fresh Cargo home and target directory.

Contracts' Rust types are the IR authority; schema output is generated and
ignored. Source archive/AST, IR, and internal snapshots have separate versions.
Core imports contracts only, never compiler/parser. Network, filesystem, clock,
OS randomness, async tasks, and agent inference belong outside core. Shared
immutable catalogs and owned match tables are the baseline. No unsafe code.

Card facts come from the frozen exact source fixtures. Cite pinned official
rules in implementation comments. Compile the complete source card or reject
it; unknown semantics never disappear. Admission checks build/profile and
whole-scenario configuration separately from lowering. Test both successful
source-to-runtime behavior and paired unsupported/invalid cases.

Keep physical IDs, zone incarnations, stack IDs, owners, and controllers distinct.
Use exact game quantities and checked structural arithmetic. Frames, inputs,
events, and outcomes serialize as data. Queries cannot mutate semantic state.
Restore/replay must preserve behavior and clone branches must be independent.
Use explicit `Budget` with `advance`. Player-facing code consumes `Observation`,
`PlayerRequest`, and `PlayerResponse` and budgeted query outcomes; raw IDs,
`decision_for`, snapshots, and full-state getters are trusted host APIs. Never
send query recovery sidecars or validation receipts to clients. Queries cannot
mint handles or observation history. See `docs/game-engine-p2-slice.md` for
handle lifetime and work invalidation/recovery contracts.
Do not update a failing expected result without reviewing its source and rules.

Commit small reviewed fixtures and gate summaries. Keep downloaded corpora,
bulk IR, schema exports, replay bundles, timing reports, and Cargo target output
ignored. Add mechanics only after their compiler/runtime/interaction evidence;
do not broaden the first pool as an incidental fix.
