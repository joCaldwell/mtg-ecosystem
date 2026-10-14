# Compiler and engine implementation progress

## First scenario verified on October 13 2026

P0 is complete for the [frozen first scenario](game-engine-first-scenario.md).
The initial P1a/P1b slice is implemented and verified: four source definitions
lower to executable IR, a separate fixed-scenario admission check runs, and a
four-seat land-mana / targeted return-to-owner-hand action executes through the
same typed event and continuation paths used by restore, fork, and replay.
One paired complete-source definition remains unsupported. This is scenario
evidence, not a released playable Commander profile or blanket P1 signoff.

The reviewed fixture has seven accepted inputs and thirteen events. The creature
is owned by P1 and controlled by P2; it returns to P1's hand with a new
incarnation. The spell finishes in P0's graveyard. The full input/event sequence
and final semantic hash are checked in `expected-scenario.json`.

Validation completed:

- `npm run check:engine`: formatting, Clippy with warnings denied, 19 Rust
  tests, and 2 Node cross-language tests passed.
- `cargo test --workspace --locked --release`: the same 19 Rust tests passed
  with optimized code and overflow checks enabled.
- Release CLI demo: uninterrupted, restored, cloned, and replayed runs agreed.
- Retained executable and bundle verified successfully from a fresh directory,
  using only retained inputs. Dependency vendoring and a fresh offline rebuild
  are still P2 retention work; this check does not claim them.
- No parser source edits were made in this worktree. Ongoing parser changes in
  the main checkout are separate from these pinned historical inputs.

The tests cover corrupt/unknown interchange, rehashed unsupported semantics,
source block association, exact arithmetic, unsupported configurations,
functioning zones, owner/controller distinction, invalid/stale choices,
payment reversal, whole-spell multi-block target legality, query purity,
pending-input recovery, receipt isolation, and unreachable rehashed snapshots.

## Retained identities

The release uses Rust 1.99.0 on x86_64 Linux, with Cargo.lock and build settings
retained. Build identity hashes engine/compiler/contracts/CLI source files,
reviewed fixtures, Cargo manifests/lockfile, toolchain declaration, compiler
identity, target, Cargo profile, and Rust flags. IR canonicalization is compact
sorted-key JSON; archive verification separately retains JavaScript serialization.

| Artifact | SHA-256 |
| --- | --- |
| Release build identity | `dee3f9efc8da625a8afa6834495f0383e712340c16f61ba31dcdc352247cace8` |
| Definition catalog | `1ecd2712be49d27c411af2ca06de471375a081136e87ef42cd123ca8c1ca466c` |
| Scenario profile/configuration | `3159762e11cd1657add028e49e0e1ca2bfade40fadce1621817ba1407a5be2ea` |
| Final semantic state | `93258caa3c9da37b71fc06e39e64986d97991dc57ae8f993a961b575260f8c32` |
| Replay bundle | `788a5a91a3aecd8abae40451ab2374fadaa53fc6013422687fbcfe1e864143e6` |
| Retained executable | `2f5008d31ff3e7a2d6bb49693f454ca51fdf659950cfdad65902522e6f983a88` |

The bundle and executable are retained outside the worktree under
`~/.local/share/mtg-ecosystem/engine-evidence/p1/`, named by their content hashes.
The bundle includes complete selected source records, IR, source/build closure,
configuration, accepted decisions, initial/announcement/final snapshots, and
expected results. Generated reports are additionally in `generated/` locally.
Small reviewed source/IR/trace fixtures remain in version control.

Initial release observations on the frozen Ryzen 9 9950X machine were 429 ns
mean clone-and-dispose, 8 ns mean priority-request query, and 467,223 ns across
the four equivalent tiny runs. These single-run calibration observations
include a three-object state and do not establish Commander throughput, memory
capacity, p95 latency, or search performance.

## Budget and observation slice verified on October 13 2026

The [bounded P2 slice](game-engine-p2-slice.md) is implemented. Execution uses
explicit work budgets and suspends without inventing a game outcome. Observation,
decision, response-validation, and SearchContext queries resume from actual row
or stage cursors. Live work and receipts bind to executor and exact state epoch;
automatic execution invalidates them even when the accepted-input revision stays
unchanged. Recovered query work recomputes its saved prefix before rebinding.

Player-facing types expose own hands, public objects, and hidden-zone counts,
using viewer-specific handles that expire on movement or lost visibility.
Observation history and handle allocation happen during deterministic transitions,
never queries. SearchContext contains permitted observations/history, open deck
compositions, public definition/profile identity, and a caller-supplied sampling
seed. Raw internal IDs, game-state hashes, snapshots, frames, and game seeds are
absent from these projections. The sampler remains future work.

Snapshot version 2 restores pending inputs and suspended payment/resolution
frames at the recorded rule-step boundary. The original seven inputs and thirteen
events remain unchanged. The new state hash includes persisted visibility and
the ten deterministic rule steps; it is recorded separately from the P1 hash.
Library, command, and exile zone types are represented without admitting their
new gameplay. Legal-match mode remains rejected.

Validation completed:

- `npm run check:engine`: formatting, Clippy with warnings denied, 25 Rust tests,
  and 3 cross-language/CLI projection tests passed.
- `cargo test --workspace --locked --release`: all 25 Rust tests passed.
- Execution budgets of 1, 2, 3, 7, and unlimited units produced equal final
  states/events. Every pending/suspended boundary restored exactly.
- Query checks covered purity, zero-budget incompleteness versus complete empty
  domains, stale/foreign work, scoped/stale handles and decision tokens, receipt
  invalidation, tampered recovery scratch, and equal permitted contexts despite
  different hidden internal IDs.
- `npm run retain:engine`: retained 1,581 dependency files and native source
  closure, rebuilt with `cargo --frozen` in an initially empty Cargo home and
  target directory, and verified replay plus the observation-query sidecar with
  the rebuilt executable. Preparing the vendor closure fetched a locked
  target-specific dependency; the independent rebuild was offline.

Current release identities:

| Artifact | SHA-256 |
| --- | --- |
| Release build identity | `317ef0d4de787ede292276b47c2b1f6843715f13516ad271af14ff5c03d12772` |
| Final version-2 state | `ab6e126940e3af4e4847f2e9afdba7a788c9efc388b969faf96b4b678e7aa60f` |
| Replay bundle | `5af01a788e07ac3e8310d641308d2385b99048e8fe8d3f5fe27d81c96185a3a4` |
| Rebuilt executable | `ac20768f94483477d024258dafa43c9d491224c24d21b1019e20e656a11d08c4` |
| Dependency file manifest | `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f` |

The catalog and scenario-profile digests are unchanged. Current evidence,
sources, vendor files, and rebuilt executable are retained in
`~/.local/share/mtg-ecosystem/engine-evidence/p2-5af01a788e07ac3e-B1D47x/`;
`evidence.json` records every dependency file hash and verification result.
The P1 bundle and executable remain separately retained for their original build.

Tiny-scenario release observations: 6,061 ns mean clone-and-dispose; 256 retained
branches increased process RSS by 12,296,192 bytes; version-2 snapshots encoded
32,772 bytes. These include the added four-viewer history/knowledge state and
do not establish representative Commander throughput, memory capacity, or search
latency. Copying retained histories and journals still grows with run length.

## Commander and departure follow-up — October 13, 2026

The [frozen departure brief](game-engine-departure-slice.md) is implemented for
both exact scenarios. IR v2 adds an explicit legendary characteristic and the
source-verified Isamaru fixture; complete source/AST hashes still use archive
schema 3. The catalog contains five reviewed definitions and one paired
unsupported source record. Snapshot/bundle v3 retain original seats, absent
active-player state, last-known departed-player facts, default/control-effect
provenance, a physical Commander ledger, and a mixed decision/external journal.
Observations and SearchContext v2 expose only permitted commander designation
handles and public history, without authoritative physical IDs.

Concessions invalidate work/receipts immediately and run before ordinary steps.
Tests cover owned objects leaving, default-controller exile, ending grants,
static-source versus persistent grants, live priority and prior passes, casting
reversal before payment, source departure, target removal before/during
resolution, final-survivor victory, budget partitions, and forged recovery.
Command-zone tax and per-commander combat history have exact domain checks;
combat accumulation is test-only until the damage interpreter is implemented.
Neither admitted scenario casts its commander or deals damage.

Validation: `npm run check:engine` passed formatting, Clippy, **38 Rust tests**
and **3 Node cross-language checks**. `cargo test --workspace --locked --release`
passed the same 38 Rust tests. `npm run retain:engine` rebuilt with
`cargo --frozen` using 1,581 vendored files, an initially empty Cargo home, and a fresh
target directory. The rebuilt executable verified both mixed journals, all
thirteen departure checkpoints, and the query recovery sidecar. The original seven decision inputs and
thirteen events remain byte-for-byte equivalent as JSON values. The current
baseline state hash is
`862f13d9e8f38ce272b7664f672875395cd522d2a6481a1143c3e581c49f8778`.
The departure demo uses five decision inputs and three concessions, verifies
thirteen exact recovery checkpoints, and ends with P1 winning. Its state hash is
`5aa8dca40556c64ca229a4c091fac00cce9faf60413da89fcd9ac0c985a17998`.
Reviewed trace/state witnesses are in `expected-departure-scenario.json`.

Departure-slice release retention:

- Build: `91db7bf35112686e578fa931607a2c851e9affcab369167feeb7df4f1b868cfd`.
- Catalog: `7ca94c66fd9c8d1884fa13ccbd5066f0d62c2ed08c0c67d741f4b260d6b92a66`.
- Baseline profile: `758cade50b263c744cd7dc5db08f20685496f73f82adacb8a57da8dfff942272`.
- Commander profile: `87fca35a63ff9ccc5e22a48ae9c71d92ee6c3c8b0ae4766206bc80d7ebef4588`.
- Bundle: `68b7847d11c977f3593f92b36973d214bb06250ace8738a12db67f2971e86e16`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Executable: `2cba0516b0a69f85a1a36475feeea8209219096c2abad656a17ded4e8f6dcdb7`.

Retained closure: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-68b7847d11c977f3-VMkaSX/`.
The historical evidence manifest is `evidence.json` there.
`generated/p2-retention.json` points to the most recent capture in this worktree. Generated artifacts stay ignored.
For this tiny scenario, 256 retained branches increased RSS by 13,217,792 bytes
and snapshots encoded 38,037 bytes. These observations are not representative
Commander performance or memory guarantees.

## Atomic events and trace modes — October 13, 2026

The [event brief](game-engine-event-slice.md) is implemented without adding any
card definitions or IR vocabulary. Private batch commits capture root/occurrence
pre-state, typed cause/rule references, hierarchy paths, and actual outcome slots.
Intrinsic mana is one ordered tap/add batch; casting, payment, instructions and
completion use singleton batches. Owned objects leaving during departure form
one simultaneous removal batch. Independent simultaneous writes use a common
pre-view; conflicting dependencies, empty/oversized groups, stale IDs, negative
payments, wrong zone owners, and structural overflow reject atomically.
General replacements, triggers, simultaneous damage, and SBA batches remain
unsupported. No public batch mutation API bypasses legality.

Diagnostic traces now live outside semantic state. `off` retains no evidence,
`compact` retains a rolling verification digest/count, and `detailed` adds records
and the unchanged flat events. Logical counters and typed results are shared by
all modes; player observations and state hashes do not contain trace payloads.
Snapshot/bundle v4 verify semantic state and diagnostic evidence independently by
exact-boundary replay. IR and player projection formats stay at version 2.

Validation: `npm run check:engine` passed formatting, Clippy, **45 Rust tests**
and **3 Node cross-language checks**. Tests compare all three modes at nineteen
pending/suspended departure boundaries and multiple execution-budget partitions,
check rollback and branch isolation, and reject forged diagnostic recovery.
The native bundle contains all six scenario/mode runs, the original source/IR
closure, thirteen departure checkpoints, and the saved query-work sidecar.
The seven-input/thirteen-event P1 trace and prior departure flat trace remain
unchanged. New witnesses are retained in `expected-event-scenario.json`:

- Baseline state: `f24c5dcc785a849ef7d58e373734c9b74465541bae5d641288be982cc56eb922`.
- Departure state: `8908bda0c86d8e0b34b95b3be52f1ae0b2ab9f353ca9721d8f0ba561d321a806`.

`cargo test --workspace --locked --release` passed the same 45 Rust tests.
`npm run retain:engine` rebuilt the source/dependency closure with `cargo --frozen`
in an initially empty Cargo home and a fresh target directory. The rebuilt
executable verified all six scenario/mode runs, thirteen detailed departure
checkpoints, source-to-IR equality, and the query-work sidecar. The vendored
closure still contains 1,581 files. Event-slice release retention:

- Build: `4adc62121fe1a2db9a8222912b58856427a6f1c360296b5bf39b2496dd41b0c9`.
- Catalog: `7ca94c66fd9c8d1884fa13ccbd5066f0d62c2ed08c0c67d741f4b260d6b92a66` (unchanged).
- Baseline profile: `758cade50b263c744cd7dc5db08f20685496f73f82adacb8a57da8dfff942272` (unchanged).
- Bundle: `ba07fea135b0bf0fdfcb5217e1a47245d902cce5af7698f0bc1fb7dd88d74620`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Rebuilt executable: `90da901395700b110b9c70bb5a85cd3df4ee165fc17c9c6d31a292f5c8f3da07`.

Retained closure: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-ba07fea135b0bf0f-vzMiV8/`.
Its `evidence.json` contains the offline verification and dependency checksums;
`generated/p2-retention.json` is the current worktree summary. These remain
ignored. Baseline snapshots encode 36,353 bytes off,
36,443 compact, and 48,447 detailed. These are artifact-size
observations for the tiny fixture, not representative Commander memory/latency
limits. Histories, snapshots, and cloning still grow with run length.
This advances P2 foundations; it is not full P2 signoff or legal-match support.

## Four-player instant responses — October 13, 2026

The [frozen stack brief](game-engine-stack-slice.md) is implemented. The third
exact configuration gives every seat an Island and Unsummon, plus the original
borrowed Bears; all nine physical cards reuse the existing five definitions.
Each opponent can respond, completed entries remain below the announcement,
and each live-player pass round resolves exactly the top spell. A four-entry
chain resolves in reverse order; later spells recheck stale target incarnations.
No new compiler grammar, card-name dispatch, source definitions, or randomness
is introduced. General phase scheduling and permanent casting remain unsupported.

Casting reversal restores the saved pass round and original hand incarnation,
filtering departed seats. Losing a lower source preserves the top frame.
A resolving frame now owns its frozen spell instance: this corrects the earlier
source-departure limitation under CR 608.2m. Supported choice-free effects keep
resolving after their source leaves, skip an already absent source movement,
and then release priority. No resolution-time chooser reassignment is claimed.
The earlier source-departure regression was updated only after reading that
pinned rule and adding detached-source replay/recovery evidence.

Projection v3 exposes bottom-to-top stack order and per-viewer announcement
references. Expired targets expose no current handle; projections never follow
the physical card into another player's private hand. Queries budget stack rows
and still allocate no handles/history. Snapshot/bundle v5 and sidecar v2 retain
these contracts. Restore validates each lower program separately, unique/order
identities, top invocation binding, and exact diagnostic replay. Fault recovery
now accepts the original pending input only after replay proves the failed
transition; unsupported empty-stack advancement remains a fault, never a win.

Validation: `npm run check:engine` passed formatting, Clippy, **55 Rust tests**
and **4 Node checks**; release tests passed the same 55 Rust cases. Tests cover
all three opponents, four entries, invalid timing/ownership, unpaid response
reversal, affected versus unrelated departure, detached invocation, query
sidecars, private-hand/target projections, rehashed malformed metadata, and
multi-program lower-entry validation through a private interpreter fixture.
Old baseline/departure flat traces stay unchanged. Current witnesses are in
`expected-stack-scenario.json`: the ordinary response has 16 decisions, 27
events, and 41 checkpoints; the detached-source run has 16 decisions, one
concession, 30 events, and 43 checkpoints. Twelve retained trace-mode executions
cover three configurations and both response paths.

`npm run retain:engine` completed a fresh offline `cargo --frozen` rebuild with
an initially empty Cargo home and target directory, using 1,581 vendored files.
The rebuilt executable verified all twelve mode executions, the thirteen older
departure checkpoints, 41 normal-response checkpoints, 43 detached-response
checkpoints, source-to-IR equality, and query recovery. Current release identity:

- Build: `31e66f91677319cb67d6bc75c8640dc250f6524c28a0d024f12f4eb913254c38`.
- Catalog: `7ca94c66fd9c8d1884fa13ccbd5066f0d62c2ed08c0c67d741f4b260d6b92a66` (unchanged).
- Baseline profile: `758cade50b263c744cd7dc5db08f20685496f73f82adacb8a57da8dfff942272` (unchanged).
- Response profile: `cdb88fe5c65483c5bdfb15f62f4477efcd2495a25423ad39fc0a4bce120fcb54`.
- Baseline state: `beae413ee28be2dd85e3d2ff4d8bdc7438bcbaecdc8282bf035c9ce03fd2b96e`.
- Ordinary response state: `c51afc9730efa945d8541f061a7996e9dff75a3a6fe1c305a85c3313cd002331`.
- Detached response state: `b764c77fc97e27466372d668a78b1443131eb957034c0cd452003bec78e0ecad`.
- Bundle: `f2f2844f0769ea0884e87d8aa1533e8db37d5953acf47a127c5c0b08f30ce3e1`.
- Rebuilt executable: `92f21ebed0b0385548d85a320a7b34ab9cffc43f578001295e78ce9cd1420e6a`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.

Retained closure: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-f2f2844f0769ea08-p91nDd/`.
`evidence.json` there and `generated/p2-retention.json` in this worktree record
verification and dependency checksums. Generated artifacts stay ignored.
Recovery still replays the small closed profile; these checks do not establish
representative full-game throughput or general timing/choice correctness.
This completes the bounded slice; full P2/P3 and Commander match gates remain.

## Casting-time mana follow-up — October 13, 2026

The [frozen payment brief](game-engine-payment-slice.md) is implemented. All
admitted profiles offer a caster-only mana window after targets, even with an
already sufficient pool. Intrinsic mana abilities use the typed tap/add path
without priority or stack entries. FinishMana attempts full payment; it cannot
cancel an affordable spell. On failure, the player can keep all or undo all
casting-time mana activations. Exact receipts and typed reversal outcomes
preserve pre-cast mana, zero-entry shape, original spell incarnation, and
intervening departures. A retained mana action interrupts the old pass round,
including when its caster subsequently leaves. No player can respond during
target announcement, the mana window, payment, or resolution.

A fourth exact admitted configuration uses two source-backed Forests against
Unsummon's U cost. It witnesses an unpayable attempt after floating G before
casting and producing G during casting: undo restores the second Forest and
leaves G=1; keep leaves it tapped and G=2. No source/AST/IR definitions change.
Snapshot/bundle v6, Observation/SearchContext v4, and sidecar v3 record the new
stages. Historical flat gameplay traces still agree, with explicit mana-window
decline inputs added to current witnesses. `expected-payment-scenario.json`
records current state hashes and journals; prior fixtures remain historical.

Validation: `npm run check:engine` passed formatting, Clippy with warnings
denied, **73 Rust tests** and **5 Node checks**. Optimized release tests passed
all 73 cases. New coverage includes successful casting-time mana, affordable
extra activation, keep/undo failure, pre-floated mana and zero-entry shape,
wrong seat/timing/stale handles, atomic reversal failures, departed sources and
payer history, accepted activation invalidation, saved query/decision boundaries,
rehashed forged receipts, trace/budget equivalence, and replay. Independent review
found a caster-departure pass-round bug; the fix and a production lower-stack
regression passed. No unresolved correctness findings remain in this slice.

The success witness retains 8 decisions, 13 events and 23 recovery checkpoints;
failed undo retains 5 decisions, 8 events and 14 checkpoints; failed keep retains
5 decisions, 7 events and 14 checkpoints. The ordinary/detached response witnesses
now have 18 decisions with 45/47 checkpoints, preserving their 27/30 flat events.
The Commander/departure witness has 15 checkpoints. All seven witness families
run in off/compact/detailed trace modes (**21 retained executions**).

`npm run retain:engine` rebuilt the native source and 1,581 vendored dependency
files with an empty Cargo home and fresh target directory, using offline frozen
Cargo. The independently rebuilt executable verified all trace-mode runs,
recovery checkpoints and the query sidecar. Retained identities:

- Release build: `c06c2b25d566c9bdd9c0c84816e07c6c55881404cf9b46c023a06fb33eda3596`.
- Baseline v6 state: `8a96749b46b382c93b61daa151b1498f40b3a65771f5e0aede172fb66b4da057`.
- Replay bundle: `91393f2636a9950296d4572d2601fe0089da6290800f469efaddefedbea15e07`.
- Rebuilt executable: `d55abf5b4edda6ae7e9d63452cc24758fca7871411d9f6fbe99f24ca1ed6a688`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.

Evidence lives in `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-91393f2636a99502-aKZgnA/`; `evidence.json`
records the source/dependency closure and verification. The ignored worktree
summary remains `generated/p2-retention.json`. No parser edits or bulk artifact
commits are part of this work. Keep-all/undo-all is a restricted reversal policy;
subset choices, dependent mana abilities, control-changing source exile,
general costs, permanent casts, full phases and legal Commander games remain
later slices. This completes the bounded payment slice, not full P3 signoff.

## Departure boundary review fixes — October 13, 2026

Review found two gaps in the accepted-input/concession boundary. Departures now
revalidate pending priority actions as well as casting-time mana actions. An
accepted cast whose only target domain disappears is discarded before proposal,
preserving its journal entry and original hand incarnation; unrelated departures
still preserve valid pending casts. This prevents an unrecoverable intermediate
snapshot followed by an impossible empty target request.

Queued concessions now drain before ordinary resolution starts. A pass round
cannot trigger unsupported empty-stack advancement before queued departures
produce a winner, and resolution cannot capture target legality before another
queued departure removes that target.

Three new regression tests failed before the fixes and pass afterward. They
exercise all three trace modes, single-unit versus unlimited budgets, recovery
at intermediate boundaries, journal replay, public requests, and trace equality.
`npm run check:engine` passed formatting, Clippy, **76 Rust tests**, and **5 Node
checks**; optimized release tests passed all 76 cases. The external review
harness also checked 2,990 boundaries without recovery failures or empty-domain
deadlocks. Existing expected gameplay fixtures and semantic hashes are unchanged.

`npm run retain:engine` verified a fresh offline frozen rebuild and all 21 retained
trace-mode executions. Release build:
`8496fd782a125704a4dc0299fa815ecccf250a28cd8f410f0e177690f093601f`.
Replay bundle:
`1109cc1c16e78e2615e236dcbb4058f81f0cec3a26039b627137281e33a50133`.
Evidence lives in
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-1109cc1c16e78e26-v0qMKo/`;
the ignored summary is `generated/p2-retention.json`. No scenario admission,
schema versions, or supported mechanics were broadened.

## Explicit turns, land plays, and creatures — October 13, 2026

The [frozen turns brief](game-engine-turns-slice.md) is implemented on a fresh
`feat/engine-turns-permanents` worktree from merged main `5ab3cba`. It adds one
exact eight-card profile using the existing source-verified Forest and Grizzly
Bears records. IR v3 distinguishes instant/instruction programs from main-phase,
targetless permanent programs and records land-play/creature-cast capabilities.
Compiler success still does not admit arbitrary definitions or configurations.
All five configurations are checked against exact reviewed definition digests.

P0 plays a land and casts Bears through its caster-only mana window, payment,
and stack; the creature resolves to the battlefield with a new incarnation.
Explicit step/entry continuations then rotate the four seats through mana
clearing, untap, singleton-library draws, forced-empty combat, and restricted
cleanup. P0 reaches turn 5 precombat main and plays another land after its
allowance resets. Lands and creatures require active-player main-phase priority
and an empty stack. Empty-stack progression is enabled only for this profile;
earlier profiles preserve their unsupported boundary and all three merged
departure-ordering regressions.

New typed atomic events capture land usage, scheduler facts, simultaneous untap,
and mana loss. Snapshot/bundle v7, Observation/SearchContext v5, and query-sidecar
v4 retain those facts and entry-work continuations. Departed-active turns finish
without replacing their active player; queued concessions still drain before
ordinary work. Queries remain pure, budgeted, and private. No core compiler,
parser, network, filesystem, clock, or random-source dependency was introduced.

The witness has **122 decisions, 316 events, and 321 recovery checkpoints**.
Its final state is P0 priority on turn 5, one land played, three untapped Forests
and untapped Bears on the battlefield, each player holding their single drawn
Forest, empty libraries/stack/mana, and unchanged life. The new
`expected-turns-scenario.json` records current semantic/trace identities while
preserving older fixtures as historical evidence. Every earlier flat gameplay
trace and mixed journal was compared and remains unchanged. Eight witness
families run in three trace modes (**24 retained executions**).

Independent rules review verified the official pinned rules bytes. Independent
implementation tests cover timing/land counts, creature payment and reversal,
both main phases, pending decisions, departures at each scheduler/casting stage,
query/snapshot recovery, stale handles/receipts, branch isolation, atomic failure,
trace/budget equivalence, and explicit unsupported boundaries. Final independent
review found an event-evidence completeness gap; recording priority, live seat
order, and stack emptiness in pre-event turn facts resolved it. No unresolved
findings remain. An additional external review harness recovered/replayed
**3,318 boundaries across 900 ordered-concession branches**; its source and
review summary are retained alongside this evidence in `independent-review/`.

Optimized `cargo test --workspace --locked --release` passed **101 Rust tests**.
`npm run check:engine` passed formatting, Clippy with warnings denied, the same
**101 debug Rust tests**, and **6 Node cross-language/CLI checks**.
`npm run retain:engine` retained 1,581 vendored dependency files and the exact
official rules bytes, rebuilt with frozen/offline Cargo using an initially empty
Cargo home and fresh target directory, and verified all 24 trace-mode executions,
all saved checkpoints, source-to-IR equality, and the query recovery sidecar.
Retained identities:

- Release build: `10f448aed613a9664347a6c57370850e5a6874815359623e9a107ddad5d512b5`.
- Catalog: `885924ca7234ad41d81b4a01c50c07e5d693850b36c3c5b1236d5ca2df86570e`.
- Turns profile: `fc9026bc69f850898fa03a0ed9eb8828b8235bbaa270925498fe2a9ddbaa6d7a`.
- Turns final state: `83d53d69430b42fde1cad1e3f7cb02d53d0e49fd1d08036a373bd8ee192ce19d`.
- Baseline v7 final state: `675636d8307c09165ca7c6df997ea1b112c90571eb7327f6a2055e252e6e6060`.
- Replay bundle: `5d627e95abff2e2b572db5239ee05cec7993bc1af5d033e8185f520aa0cd41cf`.
- Rebuilt executable: `e61aa81c10cb8bfc26396a621b8a72ba7bbc5935dc191b78bb19b5c2b15016c9`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.

Evidence lives in `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-5d627e95abff2e2b-qyyHzm/`.
The ignored worktree summary is `generated/p2-retention.json`. Downloaded rules,
source/vendor closure, executable, and bundles remain outside version control.
All 148 dirty parser files in the main checkout were hash-checked unchanged;
none were copied into this worktree.

Creature combat (even where summoning sickness might prohibit attacks),
empty/multiple-card-library draws, cleanup discard, triggers/replacements,
additional costs, other casting zones, and Commander lifecycle remain explicit
unsupported boundaries. This is a deterministic bounded scenario loop, not full
P3 signoff or playable Commander matches. Snapshot reachability still uses replay;
recovery-heavy debug checks and history copying grow with execution length.

## Remaining delivery gates

Remaining P2 foundations include broader functioning-zone/ability discovery,
interaction-specific event compositions, and wider recovery invariants.
Atomic batches and the three trace modes are implemented for the closed profiles;
replacement/trigger/SBA consumers still need their own source/runtime slices. Commander designation/history and departure
foundations are implemented in the bounded follow-up described above; production
command-zone casts and actual combat damage remain later mechanics.
Recovery still replays the small scenario to prove reachability. Rules work units
are not wall-clock or process-memory guarantees; larger profiles require measured
limits and finer resumable work. Core authoritative getters remain trusted host
APIs; player-facing consumers use the new projection/query types. This slice is
not full P2 signoff.

P1 contracts also need refinement as later slices arrive: broader source-AST
schema export, complete printing/membership indexes, structured build/admission
reports, and additional mechanic/functioning-zone descriptors. No parser rewrite
or port of the entire TypeScript binding analyzer is required.

P3 still expands general cost families, broader permanent and turn behavior,
and resolution-time choices. Ordinary supported instant responses, casting-time
intrinsic mana, and the bounded turns/land/targetless-creature loop are now
implemented for exact scenario configurations; broader stack/cost families remain. P4 adds the Commander lifecycle and
event interactions. P5 validates full legal decks/pods, a closed reachable pool,
and seeded complete games. Unsupported runtime paths currently fault or reject;
legal-match mode is rejected. Random operations, search, server, and UI remain
outside this first slice. No product decision currently blocks P2 work.
