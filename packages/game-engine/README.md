# Game Engine (Layer 1)

This directory contains the first Rust semantic compiler, deterministic scenario
kernel, executable contracts, and native scenario CLI. General Commander match
support, search, and the bulk Card IR pipeline remain later work.
The current P2 slice adds budget suspension, resumable query work, private
observations with scoped handles, permitted SearchContext, and recovery during
payment/resolution. See the [P2 slice contract](../../docs/game-engine-p2-slice.md).
The [bounded departure slice](../../docs/game-engine-departure-slice.md)
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

The compiler now verifies five frozen source definitions and runs a four-seat
land-mana / targeted return-to-hand scenario with strict admission, serializable
announcement, owned/incarnation identities, events, restore, fork, and replay.
The [turns slice](../../docs/game-engine-turns-slice.md) adds a deterministic
four-seat rotation, explicit phase/step entry, basic-land plays, and targetless
creature casting through the existing mana/payment/stack paths. The
[combat games slice](../../docs/game-engine-combat-games.md) adds ordinary
multiplayer combat, simultaneous damage and required SBAs, ordered draws, cleanup
discard, and rules-defined elimination. These slices admit seven historical exact configurations.
Two deterministic small games finish under player-facing policies: an eight-turn
combat/response game and a four-turn failed-draw game. Additional witnesses cover
multiple blockers and simultaneous losses.

The [Commander casting slice](../../docs/game-engine-commander-casting.md) adds
three exact configurations and a separate six-definition catalog with archived
Plains. Command-zone casting applies per-physical-card/per-caster tax; failed
payment restores the original zone. Resumable owner choices handle hand-return
replacement and fresh graveyard/exile SBA return, followed by recasting. Seven
player-facing policy witnesses cover keep/undo, hand recast, lethal combat, and
default-controller departure. The original profiles keep their five-definition
catalog and gameplay references.

The [damage and library slice](../../docs/game-engine-commander-damage-library.md)
adds three exact profiles with source-backed Yargle and Multani and Time Ebb.
Actual Commander combat damage updates physical history and causes a loss at 21.
Time Ebb uses sorcery timing and explicit ordered-library top placement, including
owner replacement and the second owner choice when drawing the tucked commander.
Hidden draw choices expose a public designation and no library object handle.
Seven more policies demonstrate natural damage losses, borrowed-source departure,
replacement/draw/recasting branches, and ordinary creature tuck/draw.

Legal Commander matches, combat keywords, additional trigger/effect families,
shuffling and arbitrary deck admission remain later work:

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
cargo run --locked --release -p mtg-sim -- commander-games --recover
cargo run --locked --release -p mtg-sim -- damage-library-games --recover
cargo run --locked --release -p mtg-sim -- demo --bundle generated/p2-bundles
```

`mtg-sim catalog build packages/game-engine/tests/fixtures/catalogs/NAME.json`
compiles and verifies any catalog manifest; `--write` regenerates its expected IR
and digest. The emitted report names the IR output file. This replaces all
historical per-catalog compile commands. The damage catalog remains IR v3; the library
catalog uses IR v4 for sorcery timing and explicit top placement. `schema` exports their generated
JSON Schema; `report` includes the paired unsupported source definition;
`verify-bundle <file>` recompiles retained records and checks recovery/replay
against their pinned build. The demo bundle includes recovery checkpoints for the original and departure
scenarios, ordinary/detached response runs, and casting-time
mana success/keep/undo runs, and the eight-card turns witness in all three trace
modes, plus four combat policy witnesses (thirty-six mode executions across
seven historical configurations), plus seven Commander witnesses and twenty-one
additional trace-mode runs in three new configurations. `commander-games --recover`
restores all 451 Commander checkpoints, including queued/pending input boundaries.
The damage/library suite adds 2,749 recovery boundaries and twenty-one trace runs,
for 78 retained trace-mode executions in all. `damage-library-games --recover`
checks every new boundary, including recorded concessions, with independent replay.
`combat-games --recover` additionally restores every
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
`bundle-metadata <file>` streams the build/source metadata needed by retention,
skipping large replay fields. Metadata extraction does not verify a bundle;
`verify-bundle` still checks the entire retained artifact. The retention wrapper
hashes and copies its complete bytes without creating a single Node JSON string.
`npm run build-ir` still rejects the
unimplemented bulk pipeline.

The test profile is optimized at level 2 with debug assertions and overflow checks
enabled. Bundle v17 records the effective optimization/check settings for its build
identity and offline rebuild, all ten exact source catalogs, and all 540 trace-mode
runs. Snapshots are v16, player projections/context v14 and query sidecars v13.
`SnapshotVerifier` verifies ordered checkpoint sequences using
one independent forward replay while applying the complete standalone restore
checks at every boundary. Histories share immutable chunks across native forks;
branch mutations remain independent.

The [action/payment slice](../../docs/game-engine-actions-payments.md) adds four
exact configurations, explicit mixed-color spending and real taxed Yargle
recasting, Cruel Edict opponent/sacrifice decisions, and Hapless Researcher
source-sacrifice activation with draw/discard continuation. `catalog build …/catalogs/action.json`
verifies its eleven-definition IR v5 catalog. `actions-payments-games --recover`
verifies fourteen source-backed policies and all 1,120 new recovery boundaries.
Together the five catalogs cover seventeen exact configurations, 5,741
combat/Commander/action recovery boundaries, and the historical checkpoints.
Blood Artist now lowers completely but remains unadmitted by the action catalog;
Cruel Edict now lowers completely but remains unadmitted by historical catalogs.
These remain closed scenarios, not arbitrary decks or legal Commander matches.

The [stack/cost slice](../../docs/game-engine-stack-costs.md) adds four exact
configurations with Counterspell, Stifle, Merfolk Looter and Azure Mage.
`catalog build …/catalogs/stack.json` verifies its fifteen-definition IR v6 catalog;
`stack-costs-games --recover` verifies eighteen player-facing policies and all
1,707 new recovery boundaries. Its stack handles distinguish different abilities
from the same source. Tap costs enforce continuous control; mana-cost abilities
retain payment/reversal, source departure and nested Commander draw choices.
Together all six catalogs cover 21 exact configurations, 174 trace-mode executions
and 7,448 combat/Commander/action/stack recovery boundaries, plus legacy checkpoints.

The [damage/life slice](../../docs/game-engine-damage-life.md) adds five exact
configurations with Lightning Bolt, Prodigal Sorcerer, Soul Feast, Healing Hands,
Sacred Nectar and Mountain. `catalog build …/catalogs/damage-life.json` verifies its twenty-one-definition
IR v7 catalog; `damage-life-games --recover` verifies twenty-eight player-facing
policies and 1,198 new recovery boundaries. Mixed target selections distinguish creature handles from player seats;
targeted tap abilities retain independent source identity after departure. Ordered
life programs defer SBAs until completion, including self-targeted loss/gain and
nested Commander draw replacement. Flame Rift remains unsupported; Blood Artist now lowers but remains unadmitted
by this historical catalog. The seven catalogs cover 26 exact configurations, 258
trace-mode executions and 8,646 recovery boundaries, plus legacy checkpoints.

The [ordinary-trigger slice](../../docs/game-engine-triggers.md) adds five exact
profiles with Blood Artist, Elvish Visionary and Essence Warden. `catalog build …/catalogs/trigger.json`
verifies its twenty-four-definition IR v8 catalog; `trigger-games --recover` verifies
23 player-facing policies and 2,960 new recovery boundaries. Actual typed events
capture simultaneous death lookback and ETB source incarnations. Pending triggers
wait through SBAs and Commander returns, then use APNAP/controller ordering and
mandatory targets. Detached sources, accepted-choice invalidation, departed targets,
Stifle and active-player elimination retain their distinct outcomes. Deathgreeter's
optional effect and Flame Rift's aggregate scope remain explicit unsupported sources.
The [effects slice](../../docs/game-engine-effects.md) adds five exact profiles
with Giant Growth, Disfigure, Fog, Samite Healer, Furnace of Rath and Yoked Ox.
`catalog build …/catalogs/effects.json` verifies thirty definitions in IR v9; `effects-games --recover`
checks 41 policies across temporary characteristics, cleanup, finite prevention,
affected-recipient ordering and applied damage prefixes. Final damage remains
simultaneous, and distinct Furnace identities apply once per original occurrence.
Together all nine catalogs cover 36 exact configurations, 450 trace-mode executions
and 15,100 recovery boundaries, plus legacy checkpoints.

The [choices slice](../../docs/game-engine-choices.md) adds six exact profiles
with complete Healing Salve modes, three Seeds of Strength target occurrences
and Slaughterhouse Bouncer intervening conditions. `catalog build …/catalogs/choices.json` verifies
33 definitions from 39 source records in IR v10; `choices-games --recover`
checks 30 player-facing policies and 2,527 new recovery boundaries, including
complete three-point prevention allocations. Together all ten catalogs cover
42 exact configurations, 540 trace-mode executions and 17,627 recovery boundaries,
plus legacy checkpoints. These remain closed scenarios. The next gate is legal
deck/pod admission, shuffle/mulligan setup and complete-game release evidence.

### Data catalogs (D1a)

Source acceptance is in `tests/fixtures/accepted-archives.json`; each row in
`provenance.json` names its archive manifest and pins its raw record. The
historical archive's full-file identity is unknown (`file_sha256: null`), so it
cannot be used for extraction. Its committed manifest and record hashes still
validate existing fixtures. The preserved current archive has a full-file pin;
the archive itself is never committed.

With the pinned toolchain on `PATH`, run from the repository root:

```sh
cargo run --locked -p mtg-sim -- catalog build packages/game-engine/tests/fixtures/catalogs/wipes.json
cargo run --locked -p mtg-sim -- catalog build packages/game-engine/tests/fixtures/catalogs/wipes.json --write
cargo run --locked -p mtg-sim -- catalog extract /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json 'Exact Card Name' oracle-id
cargo run --locked -p mtg-sim -- catalog verify /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json
```

`build` compiles and validates every listed record, checking its archive and
provenance hashes and its record-hash pin under the labelled accepted archive. Without `--write`, it checks both expected IR bytes and the
canonical digest. With `--write`, it regenerates both files. Paths inside a
manifest are relative to that manifest; record and archive paths in provenance
are relative to the provenance file. `schema_version` sets the catalog's IR
version. `output_order` is `canonical` (sorted compact JSON), `sorted` (sorted
pretty JSON), or `struct` (typed field order, pretty JSON); these preserve the
existing fixture encodings. In the committed fixture layout, `catalogs/*.json`
is reserved for manifests; expected IR files live outside that directory.
Each manifest also supplies `provenance`, `records`,
`expected_ir` and `expected_digest`. Digest files contain the canonical SHA-256
without a newline and are generated by the command.

`extract` accepts one or more exact, case-sensitive names or Oracle IDs. Every
selector must match exactly one record. It hashes the entire archive before
writing, preserves selected raw lines including their terminators, uses Oracle
IDs as collision-free filenames, extends the accepted archive's record-hash pins,
and appends archive identity, original line number and hashes to provenance. Unparsed records retain a null AST hash and
remain ineligible for executable catalogs. Excluded records may omit binding
and face data; extraction preserves those envelopes too. Existing fixture files and provenance rows are preserved; conflicting
selections fail before writes.
For a fresh extraction directory, copy the metadata from the existing
`provenance.json`, set `records` to an empty array, and place
`accepted-archives.json` and the accepted manifest line beside it. The obsolete
top-level `archive_manifest_sha256` is rejected; archive identities belong to
individual records.

`verify` re-hashes the entire accepted archive and compares every provenance row
for that origin against its `archive_line`, including exact fixture bytes and
line terminators. It rejects missing/out-of-range/duplicate line numbers,
identity mismatches and altered file pins. Rows from other origins are reported
separately. Historical records have no retained full archive, so their origin
pins preserve reviewed D1a attribution without claiming a new full-file proof.

The build embeds catalog manifests, provenance, accepted archive headers/pins
and source records, excluding expected IR, policy copies and other evidence.
Malformed catalog/group data is diagnosed at runtime through `CatalogError`;
missing generated scenario digests fail admission rather than preventing the
diagnostic CLI from building. `record-groups.json`
preserves historical mutation-test/evidence ordering only; catalog membership
comes exclusively from `catalogs/*.json`. The existing named compiler functions
are wrappers over the generic loader. Adding records or building a scratch
catalog needs no Rust edits. Profile admission is data-driven; simulator strategy reads IR features.

### Engine code identity

`BUILD_ID` hashes Rust sources and Cargo manifests under `crates/`, the root
Cargo manifest/lockfile and toolchain pin, plus compiler/target/build settings.
It excludes catalog/profile fixtures, generated outputs and server code.
Snapshots and match identities still bind the catalog digest separately.
Retained source manifests still carry data and server files with per-file
hashes; the independent verifier uses the same code-input selection as the
core build. A new data-only catalog therefore does not invalidate snapshots
for an unchanged catalog.


### Generic completion batches (D1c)

Run every profile with the same release commands:

```sh
cargo build --release --locked -p mtg-sim
target/release/mtg-sim match-batch commander-wipes-v1 generated/my-wipes-batch
target/release/mtg-sim match-replay commander-wipes-v1 generated/my-wipes-batch/game-00.json
```

`match-batch PROFILE OUTPUT` runs 18 seeded games using the profile JSON's
rotation and homogeneous-deck layout, saves every journal and independently
replays each game. `match-replay PROFILE FILE` verifies one saved journal and
rejects a different profile. The output directory must not already exist.
`match-game PROFILE INDEX` runs an individual suite game; `match-replay-live
PROFILE FILE` verifies a live journal. The DrawOnly policy remains solely for
the generic draw/decking witness; no policy variant selects a catalog.

The Develop policy uses target domains, permanent filters, instruction amounts,
static cost reductions and activation costs from the catalog, plus public board
facts. It attacks with eligible positive-power creatures, picks valuable removal
targets, lethal burn targets or the lowest-life opponent, and estimates wipe
losses from the instruction's set and amount. These are simple strategy estimates;
core validates every proposal and owns all rules. No card names or catalog-size
fingerprints select decisions.

`report.json` contains one batch `interactions` summary and per-game counts.
Instruction kinds and exact catalog sites distinguish chosen invocations from
nonempty committed instruction batches; modes and repeated continuation batches
are accounted separately. Trigger counts distinguish collection, placement and
resolution. Keyword counts measure involvement in combat declarations/damage,
including tokens and granted keywords, rather than claiming each keyword changed
the result. The report lists every unreached catalog instruction site and unused
catalog card, so omissions remain visible and need capability witnesses.
See [D1c verification](../../docs/game-engine-cards-as-data-d1c.md).
