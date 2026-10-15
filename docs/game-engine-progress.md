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

## Ordinary combat and complete scenario games — October 13, 2026

The [reviewed combat contract](game-engine-combat-games.md) freezes two additional
whole configurations on merged main `bb3f245`, including the turns/permanents and
departure-ordering fixes. The five exact source definitions and IR v3 remain
unchanged; runtime admission now recognizes seven reviewed configurations.
Existing profiles retain their previous unsupported boundaries.

Ordinary creature combat now supports complete attacker declarations across
multiple opponents, defender-specific complete blocks, multiple blockers, and
explicit damage splits under the pinned current rules (no historical ordering or
lethal-first restriction). Continuous control determines attack readiness; new
incarnations and controller changes reset it. The shared typed event path records
declarations, simultaneous damage, marked damage distinct from toughness, and
SBA waves with common pre-state candidates. Relevant zone changes remove combat
membership without clearing an attacker's blocked status. All simultaneous
losers leave before departure cleanup and outcome adjudication.

The same turn loop handles top-first ordered libraries, failed draws, cleanup
discard and damage clearing, and repeated turns with land, mana and untap rules.
The ordinary priority/instant-response stack remains the interaction path. The
reachable pool requires lethal-damage, life-loss and failed-draw SBAs; private
kernel fixtures additionally verify zero toughness, all-player loss/draw and the
cleanup-SBA priority exception without widening production admission.

The outside-core policy runner consumes only player-facing observations and
requests and submits responses through budgeted validation. Its G witness ends
on turn 8 with P0 winning at life 4. Along the way, two instants target the same
blocker and the lower spell fizzles; creatures die simultaneously; P3 discards
during cleanup; P1/P2 lose to life totals and P3 loses after a failed draw.
A second game chooses no attackers and ends on turn 4 when
P1, P2 and P3 have failed their draws. Two interaction witnesses compare 1/1 and
2/0 damage splits and simultaneous P1/P2 losses. Interaction completion, host
budget exhaustion and faults are distinct from game outcomes.

The new `expected-combat-games.json` retains the reviewed outcomes and complete
journals while preserving historical fixtures. The verified witness counts are:

| Policy | Decisions | Events | Recovery boundaries | Result |
| --- | ---: | ---: | ---: | --- |
| Integrated | 281 | 723 | 1,014 | P0 wins, turn 8, life 4 |
| Empty attacks | 69 | 205 | 273 | P0 wins, turn 4 |
| Split evenly | 20 | 49 | 67 | Simultaneous P1/P2 loss; P0/P3 remain |
| Split concentrated | 20 | 50 | 67 | Same player losses plus a lethal blocker |

All four journals contain no concessions. All earlier flat gameplay events and
mixed journals were compared unchanged. The twelve witness families run in
three trace modes, yielding **36 retained executions** across seven configurations.

Snapshot/bundle v8, projection v6 and query-sidecar v5 extend the existing
continuation, characteristic, event, observation, query, replay and recovery
paths. Complete journals and compact boundary/digest witnesses reconstruct every
new decision and automatic boundary for snapshot restoration. Full-game recovery
restores every recorded semantic boundary with diagnostics off and compares the
entire detailed final trace with detailed replay; independent continuation and
departure tests also recover detailed snapshots. This avoids redundant copies
and repeated serialization of growing diagnostic histories, without skipping
semantic boundaries. Independent review approved this split. It does not change restore's
replay-based reachability proof or make work units wall-clock guarantees.

Independent implementation and adversarial review covered declarations, stale
handles and decisions, hidden information, response windows, blocker departures,
control readiness, simultaneous damage/losses, queries, recovery, forks and
budget/trace equivalence. Review fixes preserve priority progression after a
last-unpassed departure during stabilization, include readiness changes in typed
turn-transition evidence, and reject oversized response vectors before handle
conversion. The independent interaction sweep covers 48 boundaries and 188
ordered-concession branches. Six additional branches exercise actor/unrelated
departures before/after accepted cleanup discard and departures between failed
draw and SBA. All twelve independent black-box tests pass in debug and release.

### Verification completed — October 14, 2026

Verification resumed from the requested stopping point without changing runtime
source or reviewed fixtures. Formatting and Clippy with warnings denied passed.
All **132 Rust tests passed in debug and release**, including the complete-game
policies and source-to-runtime bundle recovery/replay test. `npm run check:engine`
completed successfully, including all **7 Node cross-language/CLI checks**.
The Node checks now run the optimized CLI after the full debug Rust gate. This
reviewed harness-only change preserves all assertions and executes the complete
demonstration live; it neither uses cached output nor changes the retained native
source closure.

`npm run retain:engine` passed: it retained the exact official rules and all
1,581 vendored dependency files, rebuilt from retained sources with frozen/offline
Cargo, an initially empty Cargo home and a fresh target directory, and verified
replay with the independently rebuilt executable. Verification covers all
**36 trace-mode executions**, **1,421 new semantic recovery boundaries**, detailed
final traces, historical checkpoints, source-to-IR equality and query recovery.
Independent final audit matched all 50 retained source files to both the bundle
and current worktree, checked every dependency hash, and verified bundle, rules
and executable hashes. No independent adversarial findings remain unresolved.

Retained identities:

- Release build: `b01f9c3d889ef98efe86aa0d01f21477f4a0bc6bccbcab2b3f0c58a4de841b0d`.
- Catalog: `885924ca7234ad41d81b4a01c50c07e5d693850b36c3c5b1236d5ca2df86570e`.
- Integrated game final state: `ad1c33020249e83218f3ad18792e96675cf256d59ac828cebb523c3b66d38052`.
- Empty-attacks game final state: `7256a3d53feebeb3f59e41859d96cba733bbe8b1aeff4c6a0c21d6bb905bf6da`.
- Baseline v8 final state: `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.
- Replay bundle: `8f73f7c116c609f308c5a452bf41b2b77bdc349529beb669db2e2344e1d20c98`.
- Rebuilt executable: `a6bd4c6a7db805229f40ed62b89010230f5278b49e795975d1e7ad84b4b3b870`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.

Evidence lives in
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-8f73f7c116c609f3-UayNiD/`.
The ignored worktree summary is `generated/p2-retention.json`; final gate logs
are retained under `generated/combat-verification/`. Complete journals and
expected outcomes are tracked in
`packages/game-engine/tests/fixtures/expected-combat-games.json`. Downloaded
rules, source/vendor closure, executables and bundles remain outside version
control. The earlier successful demo and intentionally interrupted logs remain
under ignored `generated/combat-checkpoint/` as historical checkpoint evidence.

Restore still proves reachability by replaying execution prefixes. Full recovery
gates therefore take substantial CPU time; these checks do not establish a
wall-clock performance guarantee. This delivery is committed locally for review
and must not be merged without a later instruction.

Full legal Commander games, arbitrary decks, keywords, triggers, replacements,
continuous effects, setup and randomness remain excluded. This is a connected
scenario delivery, not P3/P4/P5 completion. All work is isolated on
`feat/engine-combat-games`; no dirty parser work from the main checkout was copied
or edited by this delivery.

## Remaining delivery gates

Remaining P2 foundations include broader functioning-zone/ability discovery,
interaction-specific event compositions, and wider recovery invariants.
Atomic batches and the three trace modes are implemented for the closed profiles;
general replacement/trigger consumers and additional SBA families still need their own
source/runtime slices. Commander designation/history and departure
foundations are implemented in the bounded follow-up described above. Command-zone
casting/tax and hand/graveyard/exile owner return choices are now implemented in
the three exact [Commander casting profiles](game-engine-commander-casting.md).
Three additional [damage/library profiles](game-engine-commander-damage-library.md)
now implement physical Commander combat-damage accounting, its loss check,
library-top replacement and the subsequent draw replacement;
ordinary creature combat is implemented in the reviewed combat configurations.
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
intrinsic mana, and the bounded turns/land/targetless-creature loop, ordinary combat, damage/SBAs,
ordered draws, cleanup and complete small scenario games are now implemented for
exact configurations, now including command casting/tax and connected return/recast
choices; broader stack/cost families remain. P4 still needs general replacement/
prevention families, trigger scheduling/APNAP, continuous effects and broader
event interactions. P5 validates full legal decks/pods, a closed reachable pool,
and seeded complete games. Unsupported runtime paths currently fault or reject;
legal-match mode is rejected. Random operations, search, server, and UI remain
outside this first slice. No product decision currently blocks P2 work.

## Checkpoint and fork performance — October 14, 2026

The complete debug bundle fixture now passes in **150.19 seconds** in an isolated
run (150.225 seconds including the Cargo command). The preceding unoptimized
debug run was stopped after approximately 42 minutes while still running, so
that baseline is a lower bound, not a completed timing. The prior completed
release fixture took 1,305.90 seconds. These are local elapsed-time observations;
they do not establish representative match throughput.

The follow-up removes full snapshot creation/parsing during scalar witness
capture, borrows state/trace for snapshot serialization, and verifies ordered
checkpoint sequences using one private independent forward replay. Each boundary
still undergoes strict decoding, identity/hash/invariant checks and complete
state/trace equality. Standalone restore starts from the initial state. Changed
or backward prefixes, rehashed forgeries, same-boundary queued concessions,
pending inputs, faults without boundary advancement and failed-verifier rollback
are covered by regressions. Returned games retain independent receipt scopes.

Player histories and native trace records share immutable 32-entry chunks across
forks/staging; branch mutation copies the affected chunk. Their serialized arrays
and all existing semantic/diagnostic hashes remain unchanged. Native event slices
are materialized lazily with cache invalidation on mutation. The test profile
uses optimization level 2 with debug assertions and overflow checks enabled.
Native bundle v9 adds effective optimization/debug settings to build identity
and offline rebuild metadata. Snapshot v8, IR v3, projection v6 and query-sidecar
v5 remain unchanged. No existing expected fixture was edited.

An isolated eight-turn replay probe with optimization level 2 measured a final
fork at 7 microseconds, versus 1,800 microseconds before chunk sharing. Complete
replay took 112.75 ms versus 2,000.06 ms, and standalone final restore took
201.52 ms versus 2,075.26 ms. These are single-run component measurements with
debug checks enabled, rather than whole-suite speedup claims.

Completed validation:

- `npm run check:engine`: formatting, Clippy with warnings denied, **138 Rust
  tests** in the optimized test profile, and **7 Node checks** passed.
- `cargo test --workspace --locked --release`: all **138 Rust tests** passed.
- The final full test-profile run took 23.07 seconds for 111 core unit tests,
  9.47 seconds for 12 independent combat tests, and 166.52 seconds for all three
  simulator tests. The corresponding release timings were 19.43, 9.18 and
  156.88 seconds. The seven Node checks completed in 143.87 seconds.
- `npm run retain:engine`: a fresh empty Cargo home/target rebuilt the retained
  closure with frozen/offline Cargo and verified all **36 trace-mode runs**,
  **1,421 combat recovery boundaries**, historical checkpoints, complete detailed
  final traces, source-to-IR equality and query recovery.
- Final artifact audit matched all **52 source files** to the current worktree,
  checked all **1,581 dependency files**, and verified bundle, executable and
  official rules checksums.

Retained release identities:

- Build: `4b7a876be19f45ce1e3704e71eb3d41d2f853ddf53d501beb6ff6864e9460b94`.
- Bundle: `30d40cec61470317cc81becd2ab6c01162016d321ddcebf8ebcbd46e5164d9b0`.
- Rebuilt executable: `d8a4864f5b65227c8f31694407f7f4e67f9c6b1d6dd813ea119aa83dc90664d4`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.

Retained closure: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-30d40cec61470317-4kajxw/`. Generated bundles,
component timing logs and the retention summary remain ignored or outside the
worktree.


## Command-zone casting and owner return choices — October 15, 2026

The [frozen Commander contract](game-engine-commander-casting.md) is implemented
for three new exact four-seat configurations. The compiler adds the exact
historical Plains source to a separate six-definition catalog; the original five
definitions, catalog and reviewed gameplay references remain unchanged. IR stays
v3. No parser source was changed.

Command-zone casting uses ordinary creature timing, the existing casting-time
mana/payment interpreter, and exact per-physical-commander/per-caster tax.
Announcement freezes total cost; only completed command-origin casts advance
history. Failed payment restores the original zone/incarnation and preserves the
reviewed keep-all/undo-all mana behavior. A hand recast pays the printed cost
without incrementing command history.

Owner choices now suspend and resume ordinary execution. Hand return is replaced
before its move; lethal combat and default-controller departure actually move
the commander into graveyard/exile before a fresh-incarnation SBA return choice.
Declining consumes that opportunity. A started resolving invocation survives its
caster's departure, while owner departure invalidates the choice without
resurrecting owned objects. Player-facing policies consume only permitted
observations/requests and use budgeted response validation.

Seven reviewed policy journals cover both tax-payment reversal choices, hand
recasting, lethal simultaneous creature deaths, and exile after a default
controller leaves:

| Policy | Decisions | Events | Recovery checkpoints |
| --- | ---: | ---: | ---: |
| `tax_keep` | 28 | 45 | 82 |
| `tax_undo` | 29 | 48 | 84 |
| `hand_recast` | 24 | 38 | 72 |
| `grave_return` | 34 | 72 | 115 |
| `grave_decline` | 19 | 41 | 66 |
| `exile_return` | 7 | 18 | 25 |
| `exile_decline` | 1 | 6 | 7 |

The new witnesses retain **451 exact recovery checkpoints**. Their complete
journals, events, final observations and physical history expectations are in
`packages/game-engine/tests/fixtures/expected-commander-scenario.json`.
All previous semantic final hashes, accepted journals and flat gameplay traces
were checked against their unchanged fixtures. Historical projections are
compared after the explicit observation-format version change.
Snapshot v9, bundle v10, projection/search v7 and query-sidecar v6 preserve the
new choices, frozen casting cost and fresh-return eligibility.

Completed verification:

- `npm run check:engine`: formatting, Clippy with warnings denied, **162 Rust
  tests**, and **9 Node cross-language/CLI checks** passed.
- `cargo test --workspace --locked --release`: all **162 Rust tests** passed.
- Independent reviewers added **13 black-box admission/adversarial tests**;
  tests cover owner/caster departures before and after accepted return input,
  stale tokens/receipts, restored query work, fork isolation, forged histories,
  costs, origins, fresh eligibility and nested resolution continuations.
- All seven policies agree across one-unit/unlimited execution, resumed query
  budgets and off/compact/detailed traces. Every new checkpoint restores via an
  independent replay, and complete final traces match journal replay.
- `npm run retain:engine`: retained **60 native source/fixture files** and
  **1,581 vendored dependency files**, rebuilt with frozen/offline Cargo in an
  initially empty Cargo home and fresh target, and replayed with that independent
  executable. It verified **57 trace-mode executions**, **1,421 historical combat
  boundaries**, all **451 Commander checkpoints**, historical legacy checkpoints,
  both exact source-to-IR catalogs and the retained query sidecar.
- Independent artifact audit matched the retained source closure to the worktree,
  both exact record arrays and catalogs, rules, trace bindings and vendored
  package checksums. No review findings remain unresolved. Review fixes reject
  malformed commander snapshots without panicking and bind every retained source
  record, including unsupported witnesses, to the build's exact fixture bytes.

Retained release identities:

- Build: `4ca15964f31c6d04243fe0aaadc372c8ee1ada126658161a8de93fffbffb3cad`.
- Replay bundle: `cc4b238906ea45d48230bdbbb3def5ee95f9d5bbac99770d0c9f365a0eb72aae`.
- Rebuilt executable: `0ec446ed6da703e9803a8ae0e8037cd7d86130eae6b5cadc0177e428998faeab`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Historical baseline final state: `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.

Retained closure: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-cc4b238906ea45d4-8CtGsj/`.
Its `evidence.json` records file hashes, offline flags and replay verification;
`generated/p2-retention.json` and `generated/commander-verification/` are ignored
worktree summaries and gate logs. Downloaded rules, dependencies, executables and
replay bundles remain outside version control.

This delivery is local on `feat/engine-commander-casting` and is not merged.
Commander-to-player combat damage faults before committing damage; library-return
replacement, general replacement ordering/triggers, arbitrary decks and full
legal Commander matches remain unsupported. This advances connected P3/P4
scenario evidence and does not claim either broad milestone complete.


## Commander combat damage and library/draw replacement — October 15, 2026

The prior Commander casting delivery `ee93696` was fast-forward merged into local
main before this work began. All 368 unrelated modified/untracked paths were
hash-checked unchanged by that merge. This follow-up is isolated in
`feat/engine-commander-damage-library`.

The [frozen damage/library contract](game-engine-commander-damage-library.md)
is implemented for three additional exact four-seat profiles. Authentic historical
Time Ebb and Yargle and Multani records were extracted from the existing pinned
archive, with complete source/AST/provenance verification. No parser source was
changed. The original five- and six-definition catalogs are unchanged; damage
uses a separate seven-definition IR v3 catalog, while library movement uses a
seven-definition IR v4 catalog. Rust and generated JSON Schema reject the new
sorcery/library vocabulary under v3, and runtime admission enforces each exact
profile's catalog and version separately.

Actual combat damage now updates a physical commander's per-recipient ledger
through the existing atomic damage interpreter. Typed evidence retains consumed
history and actual changes. The next SBA wave includes Commander damage loss at
21 alongside ordinary life and failed-draw losses, before simultaneous departures
and outcome adjudication. New profiles enable this mechanic; historical Commander
profiles retain their explicitly frozen unsupported boundary.
Yargle and Multani starts on the battlefield with empty history: its first hit
leaves P2 live at 22 life with 18 Commander damage; the second leaves 4 life and
36 Commander damage, causing a Commander-damage loss. Borrowed-controller runs
then concede P1 and demonstrate exile/owner return while preserving physical
history for the departed recipient. Its printed {3}{B}{B}{G} cost is preserved;
monochromatic profile resources cannot successfully recast it, and paired failed
casts reverse correctly without growing tax.

Time Ebb lowers its complete sorcery program to explicit owner-library-top
movement using ordinary main-phase timing and existing targeting/payment paths.
Library insertion retains the previous top and order. Owner replacement accepts
command directly or declines into the library. Drawing that commander then
requires a second, independent replacement under CR 903.9b. The draw continuation
moves the original top exactly once to hand or command and preserves the next
card. Its public request carries an existing viewer-scoped commander designation
and `object: None`, never a hidden library row or handle. Visible-zone choices
carry `Some(handle)`. Hand and command recasts use their actual origins and the
existing tax ledger. A noncommander Bears witness proves top placement and an
ordinary draw without Commander choices.

Seven new reviewed journals retain these results:

| Policy | Decisions | Events | Recovery boundaries |
| --- | ---: | ---: | ---: |
| `commander_damage` | 159 | 406 | 574 |
| `borrowed_return` | 193 | 498 | 700 |
| `borrowed_decline` | 193 | 497 | 700 |
| `tuck_return` | 51 | 114 | 172 |
| `draw_return` | 52 | 114 | 174 |
| `draw_hand_cast` | 52 | 114 | 174 |
| `ordinary_tuck` | 72 | 178 | 255 |

The **2,749 new recovery boundaries** retain decision count, concession prefix,
rule boundary and semantic hash; full journals/final snapshots reconstruct every
exact continuation. Expected events, observations, choices, first-hit evidence
and physical histories are in
`packages/game-engine/tests/fixtures/expected-damage-library-scenario.json`.
Historical gameplay events, journals, final semantic hashes and diagnostic
verification references pass against unchanged fixtures; projection comparisons
adjust only their explicit version tags. Recovery formats are snapshot v10,
bundle v11, projection/context v8 and query-sidecar v7.

Completed verification:

- `npm run check:engine`: formatting, Clippy with warnings denied, all **184 Rust
  tests** and **11 Node cross-language/CLI checks** passed.
- `cargo test --workspace --locked --release`: all **184 Rust tests** passed.
- **11 additional independent admission/adversarial tests** cover catalog/version
  boundaries, source/owner departure before and after accepted choices, stale
  handles/receipts, hidden-draw query recovery, failed mixed-cost casts and forged
  ordered-library/continuation snapshots without panics. Private typed-event
  tests cover exact 20/21 boundaries, independent physical sources/recipients,
  life/noncombat distinctions, control/zone persistence, simultaneous losses and
  atomic damage/library-order conflicts.
- All seven policies agree across one-unit/unlimited execution and resumed query
  budgets with off/compact/detailed tracing. Independent ordered replay restores
  every new boundary, and standalone final restore matches detailed journal
  replay. Missing/reordered policy coverage, missing checkpoints, forged roots
  and fully formed changed unsupported records reject.
- `npm run retain:engine`: retained **69 native source/fixture files** and all
  **1,581 dependency files**, rebuilt with frozen/offline Cargo from an initially
  empty Cargo home and fresh target, then verified with that rebuilt executable.
  Evidence covers **78 trace-mode executions** across 13 exact configurations,
  all **4,621 combat/Commander recovery boundaries** (1,421 historical combat,
  451 prior Commander, 2,749 new), legacy checkpoints, all four exact source-to-IR
  catalogs and the retained query sidecar.
- Independent final audit recomputed the build closure, every embedded/worktree/
  retained source hash, four catalog/profile/schema bindings, exact source arrays,
  trace/state bindings, pinned rules, all dependency hashes, executable and bundle
  checksums. The retained manifest copies, offline flags and replay result match.
  No review findings remain unresolved.

Retained release identities:

- Build: `154b26c4611f36c767dd757698f8f3499e5652c38e385dba3d18d473aa5a5e76`.
- Replay bundle: `05eba9cedd81fa2ecb0465a347b4e29600b31c0acf2492c29fa46890d8ef63be`.
- Rebuilt executable: `4c57ccaca689c373ba13347de9861a87da16b30b1afc69c378149d608f550016`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Historical baseline final state: `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.

Retained closure: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-05eba9cedd81fa2e-yJM9s4/`.
Its `evidence.json` records hashes and offline verification;
`generated/p2-retention.json` and `generated/damage-library-verification/` contain
ignored worktree summaries and gate logs. Downloaded rules, dependency/source
closures, executables and replay bundles remain outside version control.

This new delivery is committed locally for review and is not merged. It advances
P3/P4 scenario evidence; arbitrary decks, general replacement/prevention ordering,
triggers/APNAP, continuous effects, mixed-color spending choices, shuffle/mulligan
setup and full legal Commander match admission remain later work. Source-backed
small scenarios are not P5 match-support evidence or a performance guarantee.

## Action choices and mixed-color payment — October 15, 2026

The [frozen action/payment contract](game-engine-actions-payments.md) adds three
connected mechanic families on `feat/engine-actions-payments`, stacked on the
completed damage/library slice. All work is isolated in
`/home/josh/Code/mtg-engine-actions-payments`.

Four exact configurations use a separate eleven-definition IR v5 catalog:
the ordinary action scenario, borrowed Hapless Researcher, empty P1 library,
and mixed-mana Yargle command casting. The twelve exact source records include
authentic Hapless Researcher, Swamp and paired unsupported Blood Artist. Cruel
Edict now lowers completely; historical five/six/seven-definition catalogs
explicitly classify it as lowered but unadmitted. Their definitions, gameplay
journals/events and semantic hashes remain the regression references.

An affordable spell in these four profiles requires an explicit spending map
after the mana window. It pays the exact total and all colored requirements,
rejects nonpositive entries, and cannot overdraw the available pool. Two legal
payments for Yargle leave different colors and change whether Cruel Edict can
be paid afterward. The connected recast witness completes a real command cast,
returns Yargle via Unsummon, reverses an insufficient taxed attempt without
incrementing history, then pays the eight-mana taxed cost on P0's next turn.
Failed casts retain the existing keep/undo receipt semantics and original zone.

Cruel Edict targets a live opponent at announcement, including an opponent with
no creatures. Its target chooses a controlled creature during resolution;
ownership determines the graveyard. The effect sacrifices rather than destroys.
Departure before resolution makes the sole target illegal; departure during a
started choice produces no sacrifice without redirecting the subject. Accepted
choices invalidated by departure requery the remaining legal domain. A sacrificed
commander enters its owner's graveyard, the spell completes, and only then does
the fresh Commander return SBA offer a choice.

Hapless Researcher creates a noncard stack entry with a distinct stack identity,
an opaque viewer-scoped ability handle, and historical source references. Its
source is sacrificed as a cost and never becomes the stack object. Its activator
remains the meaning of "you" while draw and mandatory discard resolve. Borrowed
source-owner departure preserves a surviving activator's ability; activator
departure before resolution removes it. A started invocation retains its frozen
controller and skips impossible personal actions after that controller leaves.
Commander draw replacement resumes the pending discard, and Commander discard
return waits until the complete ability finishes. Failed draw still completes
discard before the post-resolution loss SBA. A source-driven regression also
bounces, recasts and immediately activates Hapless while it cannot attack.

Fourteen deterministic player-facing policies retain **1,120 new recovery
boundaries**, with identical accepted choices and states under one-unit/unlimited
execution, resumed query budgets, and off/compact/detailed tracing. Together with
the prior families, the demo covers **120 trace-mode executions**, **17 exact
configurations**, and **5,741 combat/Commander/action recovery boundaries**, plus
the legacy checkpoints. Bundle v12, snapshot v11, observation/context v9 and
query-sidecar v8 retain these nested choices. Queries mint no handles, reveal no
library rows, and cannot use historical source references as current authority.

Independent review covered the new compiler/IR/source closure, runtime event and
continuation paths, ownership/departure behavior, projections/query work, and all
fourteen static policy outcomes. Eight independent adversarial integration tests
and seven payment tests cover invalid selections, keep/undo, accepted-spend
departures, forged snapshots, stale/foreign recovery work and receipts, nested
replacement/discard, and newly cast activation. No unresolved review findings
remain within the admitted closure.

Two additional independent external Cargo probes verify both mixed stack orders:
Hapless activation above a completed Edict, and Unsummon above a completed Hapless
ability. Draw/discard, sacrifice domains and completion order follow the top
entry; the source stays in its graveyard. Both restore every one-unit boundary
and match final replay. Their ignored harness/log are in
`generated/actions-verification/stack-probe/` and `stack-probe.log`; the frozen
native source and retained build identity remain unchanged by these probes.

The expanded retained bundle also exposed a tooling limit: its 555 MB JSON file
exceeded Node's maximum string length. The retention wrapper now reads a typed
metadata projection through Rust's streaming reader and hashes/copies the full
bundle as bytes. An actual 555,476,320-byte bundle produced 4,449,219 bytes of
metadata with all 80 embedded source hashes checked. Required-field/type,
escaped nested unknown-field and truncated-input tests cover extraction;
metadata never replaces full bundle identity/replay verification. Each capture
uses a fresh directory, preserving and excluding historical bundle versions.

Verification of the final native source:

- `npm run check:engine` passes the complete toolchain/format/Clippy gate,
  **211 Rust tests** and **13 Node cross-language tests**, including exact
  source/parser hashes, all five schemas/catalogs and the combined CLI demo.
- Formatting and workspace/all-target Clippy with warnings denied pass.
- `cargo test --workspace --locked` and the separate optimized release run each
  pass **211 Rust tests**, including the historical gameplay references.
- `npm run retain:engine` captures **80 native source/fixture files** and all
  **1,581 dependency files**, rebuilds with frozen/offline Cargo from an initially
  empty Cargo home and fresh target, and verifies the full replay with that rebuilt
  executable. All 120 trace runs and 5,741 recovery boundaries pass.
- Independent final audit recomputes the source/build closure, every embedded,
  worktree and retained source hash, all five catalog/profile/schema bindings,
  exact source arrays, trace/state bindings, query sidecar, pinned rules, vendor
  file and manifest hashes, executable and complete bundle checksums. The store
  manifest and worktree summary match; offline flags and replay results agree.
  No review findings remain unresolved.

Retained release identities:

- Build: `7f74bcc9cda8e51f16d93b13340e871761694040f880da335bebe648b966dcc5`.
- Replay bundle: `a3e32662e7a0f4720e49f9909b081c33204bf7ca2c801738a878c4e61eb7a1cf`.
- Rebuilt executable: `c307fc800a4bf4d42184a69f63806a007d1f6212d653e9722bcfee1c362b9f33`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Historical baseline state: `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.

Retained closure:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-a3e32662e7a0f472-aAX8eV/`.
Its `evidence.json` records the offline verification; ignored
`generated/p2-retention.json` and `generated/actions-verification/` retain the
worktree summary, gate logs and final independent audit. Downloaded rules,
dependency/source closures, executables and replay bundles remain uncommitted.

The next engine chunk can add source-backed stack targeting/countering and
mana/tap activation costs, with complete announcement, departure and replay
evidence. Trigger scheduling/APNAP, general replacement/prevention ordering,
continuous effects, shuffle/mulligan setup, arbitrary decks and full legal
Commander admission remain later work. This delivery advances bounded P3/P4
evidence; it does not claim complete match support.

## Stack targeting, counters and activation costs — October 15, 2026

The [frozen stack/cost contract](game-engine-stack-costs.md) adds spell/ability
countering and tap/mana activation costs on `feat/engine-stack-costs`, stacked on
the completed action/payment delivery. Work is isolated in
`/home/josh/Code/mtg-engine-stack-costs`.

Four exact configurations share a separate fifteen-definition IR v6 catalog:
the stack/cost scenario, borrowed activated sources, empty P0 library, and
initial battlefield commander for library replacement. The sixteen exact source
records add authentic Counterspell, Stifle, Merfolk Looter and Azure Mage to the
preceding action pool, retaining Blood Artist as paired unsupported input. All
eleven earlier definitions and all five historical catalogs remain unchanged.
The compiler lowers complete source programs and costs; additional/narrowing
fields on otherwise empty source filters or tap costs reject rather than vanish.

New player-facing stack handles bind distinct invocations independently of their
physical source. Counterspell targets completed other spells, including the
caster's own other spell. Stifle retains its complete activated-or-triggered
domain; only nonmana activated entries are reachable in the admitted pool.
Candidates exclude the announcing invocation itself, wrong kinds and incomplete
entries. Saved references remain historical after an entry leaves and cannot
rebind to another ability from the same source or a recast card.

Countering a spell removes its stack entry and moves its physical card to its
owner's graveyard. Countering a noncard ability removes only that invocation,
leaving its source and paid costs unchanged. Typed outcomes retain consumed
stack facts and actual movement; aggregate stack writes reject simultaneous
composition. Commander cast history records successful casting even when the
spell is later countered. Fresh commander grave return waits for the complete
counterspell to finish. A connected witness returns the countered commander,
reverses an unpayable taxed recast, then completes the taxed cast after untapping.

Merfolk Looter requires an untapped source and continuous-control eligibility to
pay its tap cost. A source-driven regression bounces, recasts and checks its new
incarnation cannot activate until the next own turn. Azure Mage uses an explicit
mana announcement and spending decision; it can activate while tapped or
attacking and can produce two independent paid invocations from the same source.
Failed payment removes only the proposal and applies the chosen keep/undo policy
to its receipt-recorded mana activations, preserving preexisting mana and combat
taps. Source-owner departure during payment preserves a surviving activator's
frozen invocation without restoring the source. Countering a completed ability
never refunds its costs, draws, discards or emits successful ability completion.

Eighteen player-facing policies cover a counter war, own-spell countering,
countered commander return/decline/recast, departed targets, Stifle against a
sacrificed source and paid abilities, exact selection between two abilities from
one source, repeated mana activation, failed keep/undo, borrowed-source departure,
tap-source bounce, nested commander draws with/without discard, and failed draw
followed by discard before loss. All agree across detailed/compact/off tracing,
one-unit/unlimited execution and resumed query budgets. Two captures of the
reviewed report match exactly.

The new suite adds **1,707 recovery boundaries**. Combined evidence covers
**174 trace-mode executions**, **21 exact configurations**, and **7,448
combat/Commander/action/stack recovery boundaries**, plus legacy checkpoints.
Bundle v13, snapshot v12, projection/context v10 and query-sidecar v9 retain the
new handles, announcements, target references and outcomes. Historical references
normalize only the outer projection versions; their gameplay journals, events,
semantic hashes and trace verification remain exact regression assertions.

Independent review and focused verification cover complete source/IR/schema
binding, counter event atomicity and provenance, precise target instances,
costs, source/activator/target departure, tap readiness and source reincarnation,
recovered query work/foreign receipts, and rehashed unreachable snapshots. Eight
focused core tests and nine independent adversarial tests pass. Existing core
tests, compiler/contracts tests, source-parser/catalog/schema checks and scoped
Clippy also pass. No unresolved review findings remain within the admitted pool.

Final `npm run check:engine` passed toolchain, formatting, Clippy, **232 Rust
tests** with debug assertions/overflow checks, and **15 Node integration tests**.
Separate release verification passed the same **232 Rust tests**. The retained closure
rebuilt with `--frozen` and offline dependencies, an initially empty Cargo home,
and a fresh target directory; its executable independently replayed the complete
bundle. The final audit found no discrepancies across **90 source files**,
**1,581 dependency files** from **31 locked packages**, six catalogs, 21
configurations, 174 trace executions, and 235 snapshot envelopes/state hashes.
All sixteen source records also agree with the parser in the main checkout.

Retained identities:

- Build: `cfd5f2095714d0aa4cb144a156be72151abc1b04450bfbba165344776948c039`.
- Bundle: `aa759095c89065ca33e7fb60b2b47b915c2c9b6e83e29a695bfee0ecb084795c`.
- Rebuilt executable: `febc932cdd59fc0bbefd503e8603b5d4ab3657caab677c8d236ae5932d53dffb`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Pinned rules: `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
- Historical baseline state: `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.

The retained closure lives at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-aa759095c89065ca-y1YAkk/`.
Its `evidence.json` is byte-identical to ignored `generated/p2-retention.json`.
Gate logs and the final independent audit remain under ignored
`generated/stack-costs-verification/`. Source/dependency closures, downloaded
rules, executables and replay bundles remain outside the commit.

The next bounded engine work can add source-backed damage/life instructions and
trigger scheduling/APNAP. General replacement/prevention ordering, continuous
effects, shuffle/mulligan setup, arbitrary decks and full legal Commander
admission remain later work. These remain closed scenarios rather than a
match-support or performance claim.

## Noncombat damage and direct life changes — October 15, 2026

The [frozen damage/life contract](game-engine-damage-life.md) adds complete
source-backed damage and life programs on `feat/engine-damage-life`, stacked on
`19eaca7`. Work is isolated in `/home/josh/Code/mtg-engine-damage-life`.

Five exact configurations share a separate twenty-one-definition IR v7 catalog:
ordinary forty-life play, four-life loss/order tests, a borrowed activated source,
an empty P0 library, and a tucked-Commander draw profile. Twenty-three authentic
source records add Lightning Bolt, Prodigal Sorcerer, Soul Feast, Healing Hands,
Sacred Nectar and Mountain. Flame Rift's aggregate each-player scope and Blood
Artist's triggered program remain paired unsupported sources. The six historical
catalogs and all fifteen preceding definitions remain separate exact references.

Mixed target requests preserve player seats and viewer-scoped creature handles
as distinct kinds. The complete any-target domain retains planeswalkers and
battles abstractly; neither has an inhabitant in these admitted configurations.
Any-player life targets include self, while Cruel Edict stays opponent-only.
Prodigal Sorcerer chooses targets before paying its tap cost, uses no mana
window, and requires continuous-control eligibility. Paid invocations remain
independent after bounce, death or source-owner departure; their typed damage
retains the actual current/last-known source incarnation, controller, tap state,
marked damage and characteristics. Noncombat damage marks creatures or lowers
player life, without advancing Commander combat history. Lethal damage uses
ordinary SBAs and the existing Commander graveyard-return continuation.

Direct life gain/loss has separate typed outcomes with exact before/after totals.
No SBA interrupts a complete resolving program: Soul Feast self-targeting at four
life reaches zero and returns to four before checking losses. Another target at
four loses only after the controller's gain and spell completion. A sole illegal
target cancels the entire program, including later gain or draw. Sacred Nectar
resolves without targets. Healing Hands gains life before its ordered draw,
defers failed-draw loss until completion, and preserves the mandatory owner
replacement while drawing a tucked Commander without exposing a library handle.

Twenty-eight player-facing policies cover player/creature/self damage, lethal
Commander return/decline, bounce/departure target failure, Counterspell and Stifle,
Sorcerer self-destruction and detached source provenance, source-owner departure,
combined damage sources causing loss, ordered self/opponent life changes,
targetless gain, failed draws, and both nested Commander-draw outcomes. All agree
across off/compact/detailed tracing and one-unit/unlimited/seven-unit execution
and query budgets. The new suite adds **1,198 recovery boundaries**, bringing
combined evidence to **8,646 boundaries**, **26 exact configurations**, and
**258 trace-mode executions**, plus legacy checkpoints. Snapshot v13, bundle v14,
projection/context v11 and query-sidecar v10 retain these additions. Historical
reference comparisons normalize only outer projection versions.

Independent review identified and fixed three announcement/departure gaps:
accepted mixed targets invalidated by departure, self-targeted player choices
incorrectly treated as opponent-only during unrelated concessions, and borrowed
sources leaving before their targeted tap cost was paid. Thirteen independent
adversarial tests cover these regressions, query purity and authorization,
source reincarnation/tap readiness, marks accumulation/cleanup, transient zero
life, started-resolution departures, hidden Commander draws and stale receipts,
and rehashed source/target forgeries. Compiler/schema tests retain complete-card
semantics, source-block ordering, exact quantities, capability closure and
version gating. No parser edits or generated bulk artifacts are included.

The reviewed twenty-eight-policy report was captured twice with identical bytes
(SHA-256 `82d3bde1ebc34094db4d70e87a4c395313c145c1f4fd98f8d589b6aa4d84cfb3`).
Final `npm run check:engine` passed the pinned toolchain, formatting, Clippy,
**258 Rust tests** with debug assertions/overflow checks, and **17 Node integration
tests**. Separate release verification passed the same **258 Rust tests**. The independent
capture audit verified **103 native source files** totaling **8,399,776 bytes**,
**1,581 dependency files** from **31 locked packages**, all seven catalogs,
26 configurations, 258 trace executions, and the snapshot envelopes/state hashes.
All 23 source ASTs also match the main checkout parser; the audit binds its
199 TypeScript source files at closure SHA-256
`f8fa76be25830c61fdbb45fb82661201df7a74d64abbedeed795d9973ff65a3d`.

Retained identities:

- Build: `e82ce4cc8450c149d67c15c34aa508f7f4e187900b344d59b4f414b625e206d5`.
- Bundle: `dc6b881da771a85f884cefe9b0847fca6381aa62c39e3b8fdef15342c76a6ee2`.
- Rebuilt executable: `5a3efb8861d5a11e1face39efd8e4f38fb4060b4b8ce8f9ea9f62608707f0702`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Pinned rules: `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
- Historical baseline state: `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.

The retained closure lives at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-dc6b881da771a85f-un91zr/`.
It rebuilt with frozen offline dependencies, an initially empty Cargo home and a
fresh target directory, then independently replayed the complete retained bundle.
The final independent audit found no discrepancies: ignored
`generated/p2-retention.json` matches the store's `evidence.json` byte for byte,
offline-build flags and replay results agree, and the rebuilt executable's actual
hash matches its manifest. No review findings remain unresolved.
Logs and independent audit outputs remain under ignored
`generated/damage-life-verification/`; downloaded rules, vendor/source closures,
executables and replay bundles remain outside the commit.

The next bounded chunk is source-backed trigger collection, scheduling and APNAP
ordering, including target choices, simultaneous events, source departure and
countering triggered invocations. General replacement/prevention ordering,
continuous effects, shuffle/mulligan setup, arbitrary decks and full legal
Commander admission remain later work. These are closed rules scenarios.

## Ordinary triggers and APNAP — October 15, 2026

The [frozen ordinary-trigger contract](game-engine-triggers.md) extends the
verified damage/life base `168a53b` in `/home/josh/Code/mtg-engine-plan` on
`feat/engine-plan`. The migration restored the task's two tracked and three
untracked paths from the handoff archive, verified all five SHA-256 values, and
cleaned only those exact original paths after checking them. Unrelated parser
edits in the main checkout remain separate.

Five exact configurations share a new twenty-four-definition IR v8 catalog.
Blood Artist, Elvish Visionary and Essence Warden compile from complete archived
source/AST records. Deathgreeter's optional trigger and Flame Rift's aggregate
scope remain unsupported complete sources. The historical catalogs retain their
exact definitions; Blood Artist now lowers completely but is unadmitted in the
older action, stack/cost and damage/life pools. No parser changes are included.

Ordinary triggers match actual typed movement occurrences. Simultaneous deaths
share the pre-event observer view, including dying Artists; entries use the
post-event view and the entering successor incarnation. Ordered event groups
advance their observer view. Pending identities and source/controller facts are
separate from stack entries. Stabilization drains SBAs and Commander return
choices before APNAP placement. Controllers choose bottom-to-top order and
mandatory targets without gaining priority. Stifle counters triggered entries
without moving source cards; Counterspell remains spell-only. Departures
revalidate accepted mandatory choices, preserve a living controller's detached
invocations, and remove a departing controller's entries. Already placed targets
that leave cause full invocation failure rather than retargeting.

Twenty-three policies cover entry/draw/gain order, countering and bouncing,
failed draws, self-death and sacrifice, simultaneous deaths, controller/source-
owner/target departure, accepted-target invalidation, Commander return/decline,
nonactive activation and trigger-driven active-player elimination. Exact typed
evidence contains 118 collections, 114 placements and 105 completions. Three
Stifled invocations never execute; four surviving opponent invocations fizzle
after the active player is eliminated. Independently checked life changes,
chosen placement order, LIFO completion, source/subject pairs, actual occurrence
bindings and visibility agree. Two dying Artists produce ten unique observer/
subject pairs from one simultaneous death wave.

The new suite adds **2,960 recovery boundaries**, bringing combined evidence to
**11,606 boundaries**, **31 exact configurations**, and **327 trace-mode
executions**, plus legacy checkpoints. Snapshot v14, bundle v15,
projection/context v12 and query-sidecar v11 retain the additions. Historical
reference comparisons normalize only outer projection versions. The 23-policy
report was captured twice with identical bytes and recaptured unchanged after
including the reviewed golden fixture (SHA-256
`e091b08b8cc58858a79a5afb422e94e8a5884b181e7204210ab4313998adb4ea`).

Independent review found and corrected two setup/evidence defects before
freezing: the natural combat table needed actual Bears at IDs 18/30, and entry
subjects needed the entering successor rather than the old zone incarnation.
Eight independent adversarial tests and seven focused private runtime tests
cover simultaneous versus ordered observers, nondeath boundaries, atomic failure,
query purity/authorization, departure-invalidated choices, pending/placed
restores, stale handles, and structurally valid rehashed provenance forgeries.
The current main-checkout parser still produces the exact abilities of all
26 source fixtures. Its 199-file TypeScript source closure is
`08e415d3ff55b794b7e549067f18b8efef8dfcfb389ccd07ff3e3298873b0d2a`;
this read-only compatibility check preserves its unrelated binding edits.

Final `npm run check:engine` passed formatting, warnings-denied Clippy,
**278 Rust tests** with debug assertions/overflow checks, and **19 Node integration
tests**. Separate release verification passed all **278 Rust tests**. The
independent final audit checked **112 native source files** totaling
**12,422,469 bytes**, all eight catalogs, 31 configurations, 327 trace executions,
439 snapshot envelope/state-hash checks and every retained recovery boundary.

Retained identities:

- Build: `91d072044a8c77a729625ac76296cb55f2929a655d073603fc4604b52e836324`.
- Bundle: `bbf30191baf6b31c5ea7bd5c61150224deabd59270ba17060d2118a90b10c24e`.
- Rebuilt executable: `da461ae0cb62c61b18f094e8fdbdd34b63d793a8ba5643f2436f21f6f82a4e08`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Pinned rules: `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
- Historical baseline state: `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.

The exact closure lives at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-bbf30191baf6b31c-rX7NoT/`.
It rebuilt with 1,581 retained dependency files from 31 locked packages, an
initially empty Cargo home, a fresh target directory and frozen offline mode,
then independently replayed the complete bundle. Generated/store manifest bytes,
actual executable hashes, source closure and final replay all agree; no review
findings remain unresolved. Logs and audit reports are under ignored
`generated/triggers-verification/`; large bundles, rules and build outputs stay
outside the commit.

The next frozen P4 slice is [temporary characteristics and damage interactions](game-engine-effects.md):
Giant Growth, Disfigure, Fog, Samite Healer, Furnace of Rath and Yoked Ox. Its
compiler/runtime and independent tests are being prepared separately from this
verified trigger checkpoint. Full legal Commander setup/admission and named
playable-profile evidence remain later gates.


### Temporary characteristics and damage interactions verified

Implemented the complete source-backed Giant Growth, Disfigure, Fog, Samite
Healer, Furnace of Rath and Yoked Ox slice. Thirty definitions and 35 exact
source records retain nine separate catalogs and 36 exact configurations.
Temporary signed P/T modifications bind zone incarnations; cleanup removes
marks and expires effects atomically. Prevention, finite shared shield budgets
and battlefield damage doubling preserve chosen application order, original
occurrences, source departures and simultaneous final damage. Actual damage
alone changes Commander history and drives ordinary death-trigger collection.
Righteousness, modal Healing Salve and Gratuitous Violence remain complete-card
negative boundaries in this historical profile.

Forty-one player-view policies add 3,494 recovery boundaries, bringing the
combined evidence to 15,100 boundaries and 450 trace executions. Independently
reviewed typed proofs contain 36 effect creations, 27 applications and 34 damage
commits. The report was reproduced byte-for-byte after the reviewed golden
fixture (SHA-256
`82b66f5c8d0f04ab4557263511ca31ea9e473f1ff6404831dd56b86bd730b17a`).
Snapshot v15, bundle v16, projection/context v13 and query-sidecar v12 retain
these continuations. Historical semantic references remain unchanged.

Final `npm run check:engine` passed formatting, warnings-denied Clippy,
**303 Rust tests** and **22 Node integration tests**. Separate release verification
passed all **303 Rust tests**. Twelve independent adversarial tests and focused
runtime tests cover atomic rollback, stale identities, effect ordering, shared
budgets, simultaneous damage, cleanup, departure and forged recovery metadata.
The full gate caught one stale trigger-test sidecar-version assertion; it was
corrected, independently reviewed and all gates rerun on the final bytes.

The independent final audit checked all **127 native source files**, totaling
**33,739,979 bytes**, 603 snapshot envelope/state hashes, all configurations and
all recovery boundaries. All 35 source abilities matched the main checkout's
parser in a read-only compatibility audit; unrelated parser work was preserved.

Retained identities:

- Build: `ca7ad1f890b27860961d774ef20ba6f1190698f9466e6dac17b70728a0b75c6e`.
- Bundle: `9debc71c69aa531018001dec947260fd3b7e799f61c7e83352b927ced8bd8bbd`.
- Rebuilt executable: `5c055bfc14ce855993090bee2443019b949e72693b8a4c48f9dadbf04dc8eddf`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Pinned rules: `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.

The closure is retained at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-9debc71c69aa5310-1Ju2o8/`.
It rebuilt with 1,581 retained dependency files from 31 locked packages, an
initially empty Cargo home and fresh target directory in frozen offline mode,
then independently replayed the complete bundle. No review findings remain
unresolved. Generated reports, large bundles and build outputs remain ignored.
The following slice adds modal casting, separate target occurrences and
intervening-if trigger checks before legal Commander setup and admission.


### Modes, target occurrences and intervening conditions verified

Completed Healing Salve's source-ordered modal program, Seeds of Strength's
three independent target occurrences, and Slaughterhouse Bouncer's intervening
condition at event matching and resolution. Three-point prevention admits
complete integral allocation vectors, including split prevention across
simultaneous occurrences. Boros Charm and Lavaborn Muse remain complete-card
negative boundaries. Historical catalogs and gameplay references remain separate.

The newest catalog contains 33 definitions and 39 exact source records. Six new
configurations bring the retained total to ten catalogs, 42 configurations,
540 trace executions and 17,627 recovery boundaries. Thirty player-view policies
add 2,527 boundaries. Independently reviewed proofs include 32 effect creations,
60 typed proofs, seven pair applications, two full allocations, 18 damage commits
and cleanup. Snapshot v16, bundle v17, projection/context v14 and query-sidecar
v13 bind the new continuations. The compact report reproduced byte-for-byte in
the working checkout: SHA-256
`bed59e69e22de194d43159be809ebbacee4731a4a5d6a52260bd161e8fcd85ed`.

Formatting, warnings-denied Clippy, **330 Rust tests in debug and release**, and
**24 Node integration tests** passed. Fifteen independent adversarial tests cover
modes, target occurrences, condition timing, shared prevention budgets, atomic
allocation, departures and forged recovery. The full gate exposed a stale
snapshot-version assertion; it was corrected before all native gates reran.
The isolated Node stage initially lacked its parser dependency; copying the
201 committed dependency files allowed all Node checks to pass. No unrelated
main-checkout parser edits were changed.

Independent offline audit passed with zero findings: **138 native source files,
50,788,074 bytes**, 723 snapshot envelope/hash checks, all ten catalogs and all
recovery boundaries. The closure rebuilt using 1,581 retained dependency files
from 31 locked packages, an empty Cargo home, a fresh target directory and frozen
offline mode, then replayed the complete bundle.

Retained identities:

- Build: `c4cea8c9f06d0f925e345db4ee3a99d55616e2b09286779d9e72e2c6305d0c77`.
- Bundle: `3caacd0056f164b278faea6227b66162dea594a657fa31d1270bf29f70a9c247`.
- Rebuilt executable: `b484ed20a2552a2b2fe45347cf2713058a5c070726078bbc4c159f17a16a4eb7`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Pinned rules: `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.

The closure lives at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-3caacd0056f164b2-krpOXC/`.
Logs and audits are under ignored `generated/choices-staging/generated/choices-verification/`.
The next gate is full legal deck/pod admission, deterministic shuffle and
mulligans, and seeded completion evidence for one named finite Commander profile.
