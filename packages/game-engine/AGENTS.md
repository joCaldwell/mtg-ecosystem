# Compiler and engine development

Rust owns contracts, offline compilation, deterministic core, and the scenario
CLI. Read `docs/game-engine-first-scenario.md` for frozen inputs and restrictions.
The current follow-up contracts are `docs/game-engine-p2-slice.md` and
`docs/game-engine-departure-slice.md`; the latter adds the exact Isamaru scenario,
IR v2 and the exact Isamaru profile. The next contract is
`docs/game-engine-event-slice.md`: atomic event groups, nonsemantic trace modes,
and snapshot/bundle v4. `docs/game-engine-stack-slice.md` adds the exact
nine-card instant-response profile, detached resolution, snapshot/bundle v5,
projection v3, and query-sidecar v2. `docs/game-engine-payment-slice.md` adds
casting-time intrinsic mana, keep-all/undo-all failed-payment choices, the exact
four-card Forest reversal profile, snapshot/bundle v6, projection v4, and
query-sidecar v3. This package is scenario-only; do not advertise playable
Commander matches.

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

Concessions are trusted host external inputs queued at recorded rule boundaries;
process them before ordinary work and expose no query while they are pending.
Keep original seating, departed-player facts, and physical Commander history.
Default control is separate from ownership and explicit control grants.
Command-zone casting and combat damage remain unsupported; ledger domain tests
do not constitute production lifecycle or combat support.

Batch mutation is private and atomic. Capture consumed pre-event facts and use
actual typed outcome slots. Simultaneous groups admit independent read/write
footprints only; general event/replacement interactions remain unsupported.
`Game::with_trace` selects off, compact, or detailed retention. Diagnostics never
enter semantic hashes or player projections. Native `events()` is populated only
in detailed mode; `trace()` is trusted evidence, not a client stream.

Stack projections run bottom to top. Saved announcement handles are historical
references; they cannot rebind to a later incarnation or authorize responses.
Lower entries remain completed while the top is announced. Reversal restores
prior passes filtered by living players. Resolving frames own frozen instances
and continue supported choice-free effects after their source leaves (CR 608.2m).
Only the caster answers target and mana-window requests; no player receives
priority during announcement or resolution. Always offer casting-time mana,
even when already funded. FinishMana attempts complete payment and cannot
cancel a payable spell. Undo subtracts only receipt-recorded casting-time mana
and untaps surviving sources; preserve pre-cast mana and departures. Retained
legal mana activations clear prior passes. A removed caster never restores its
last-known mana or resurrects sources. Empty-stack phase
advancement still faults; restore proves fault reachability by replay.
