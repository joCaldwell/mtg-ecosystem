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

This is the historical combat-checkpoint checklist. Later milestone sections
below record the completed foundations, legal-match and sampled-search work;
use the final section for the current hosting release status.

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


### P5: finite legal Commander games verified

Released [`commander-foundations-v1`](game-engine-matches.md), with a separate
34-definition IR v11 catalog, 41 semantic source records, and a 21-card
construction pool. Four-player admission validates complete 100-card decks,
singletons, full color identity, supported commander eligibility and pinned
construction policy before allocating the pod. The policy binds 42 named bans
and seven offensive-card exclusions to exact archived source objects and retained
official HTML. Isamaru and Yargle and Multani are the admitted commanders;
this release makes no claim about arbitrary Commander decks or strategic strength.

Setup uses explicit host-seeded ChaCha20, budgeted unbiased shuffling, the free
multiplayer mulligan, paid bottom choices and ordinary first-turn draws. Initial
and redraw rounds expose no intermediate private queries. Defender is lowered
as a complete ability; Reach remains a whole-card rejection. Multiple fresh
commander return choices collect APNAP answers before one simultaneous movement.
Snapshot v17, bundle v18, projection/context v15 and query-sidecar v14 bind these
continuations and randomness. Historical catalogs and scenario hashes are preserved.

All **18 frozen seeded games** completed naturally with zero faults or host
truncations: **25,286 accepted decisions** and
**80,564 rule steps**, with 27 life losses and 27 physical
Commander-damage losses. The separate no-action all-white witness completed at
turn 371 after 12,560 decisions and 32,949 steps, with failed-draw losses in
seat order 2, 3, 0. Its final State SHA-256 is
`43b08ffe50d7844d399f229cd76b825ff1b5ba1d091a9e3484c1db41c213bffb`.
Neither completion policy uses host concessions to substitute for an outcome.

Resource diagnostics for the pre-release candidate
`ff3c8304a716fda5e0659b4e5f56407a68b8d37b8390c1aa36f034813d9a93ae`
are retained separately from the final rebuild: the second fixed 18-game batch
took 217.867 seconds including replay, with a 113.567-second native-game sum,
709.4 native rule steps/second and maximum RSS 4,083,908 KiB. Those measurements
ran alongside development checks. The separate completed long draw witness, on
build `0c4ba4b350b1de7dba59aa1d89bb3de045c3bdc31461d778779d1942f7600940`,
took 457.678 seconds and peaked at 10,410,884 KiB. Its earlier full-Value attempt
was resource-stopped and preserved; the released digest writer streams the exact
canonical history instead. These are correctness-workload diagnostics, not P7
search acceptance or current-build memory promises. All 18 final-capture semantic
results match the measured candidate.

The separate serial eighteen-pod diagnostic therefore measured **0.082619
completed pods/second including replay** over the whole 217.866870-second CLI
process, versus **0.158497 pods/second** from the sum of its per-game timers.
The batch averaged **1,404.778 decisions and 4,475.778 rule steps per pod**;
completion was 18/18 with zero faults or truncations. The per-game timer includes
Prepared admission, Game creation, permitted policy/query/rule execution,
selected checkpoints, final digest, journal cloning and sync. It excludes global
catalog compilation/startup, later evidence encoding/persistence, independent
replay and disposal after the elapsed field. This is not a core-only transition
benchmark. No single/two/four-worker speedup or current-build pod throughput is
inferred from these concurrently collected historical observations.

The original measurement file SHA256 is
`d2a31acdf1853a31ddb3dd100c22698979aa881eef590a47fdc84f423811a8fe`.
The canonical native report at `acceptance-batch-2/report.json` has SHA256
`845186437072e5903b163ae6a9f6f2e2b3ee1e649307c277d9872e072ae020da`;
the separately saved `batch-2-report.json` is exactly those bytes plus one LF,
so its byte hash is intentionally different. Both reports, measurement/stdout,
the fixed manifest, independent semantic review and exact timer source are
copied without modifying the originals into
`generated/server-staging/generated/server-verification/historical-batch-diagnostics/p5/`.

Final formatting, warnings-denied Clippy, **379 Rust debug tests and 31 Node
checks** passed; separate release verification passed all **379 Rust tests**.
Sixteen independent adversarial cases and focused interaction tests cover deck
admission, setup privacy, random continuation/exhaustion, paid mulligans,
simultaneous commander choices, departures and forged snapshots.

The fresh retained-source rebuild passed with an empty Cargo home, a fresh target
and frozen offline mode, then verified all complete match journals and the
historical **540 trace executions / 17,627 recovery boundaries**. Independent
final audit passed with zero findings: **149 native files / 51,802,366 bytes**,
**727 snapshot envelope/state-hash checks**, all eleven catalogs, 42 historical
configurations and 19 complete legal-match journals. The retained vendor closure
contains **2,318 files from 37 external locked packages**. A read-only compatibility
capture checked all 41 semantic records against its frozen 199-file parser closure;
it does not describe later unrelated main-checkout parser edits.

The final audit initially assumed that a policy's card name always equals the
full archived name. Independent review of all 49 excluded records found one
exact front-face name (Erayo). The audit now accepts an exact full name or exact
face name while retaining every source, record, Oracle identity and policy hash
check. Engine source and policy fixtures were unchanged by this audit correction.

Retained identities:

- Build: `e1f94d6a454a78b2fd0fad28cf1073a62626a3beb15268f195ffe55a49214c69`.
- Match catalog: `fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f`.
- Construction policy: `cfe83bd72e477756a7b4f778e22c6ec8e901489671d84d122574ecdc7b90caad`.
- Bundle: `88d0764e9fd2dab3bef522413a32852b8d0b75bcce4efb2e0abe0718965582f2`.
- Rebuilt executable: `fe82aa246c09a605127fe62d804fd4cd36acd9f40bd558e3eb859e5102c03247`.
- Dependency manifest: `b1d3532ff112c3ff3e5bc499289d24a6556ec16f3e401306e96a6aecc217bdcf`.
- Pinned rules: `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.

The closure lives at `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-88d0764e9fd2dab3-gTlAwl/`.
Logs and independent audits remain under ignored
`generated/matches-staging/generated/matches-verification/`. The final frozen
149 native and nine host files were copied to the release checkout and verified
byte-for-byte; all native bytes also equal the retained rebuild inputs. Search
and hosting are the next finite gates, with P6 expansions scoped separately.


### P7: verified sampled-world search for the finite foundations profile

Implemented finite sampled-world search for the released
`commander-foundations-v1` pool without admitting another card or Magic mechanic.
The reconstruction schema supports stable active PrecombatMain priority with
four, three or two living seats. Sealed captures retain permitted history, own
mulligan/bottom knowledge and remembered announced cards; complete hypotheses
supply hidden hands/libraries and independent future randomness. Sample origins,
handle mapping, forks and recovery remain separate from actual match authority.
The contract is [game-engine-search.md](game-engine-search.md).

The frozen revision-2 workload uses twelve named roots, five repetitions each,
up to 32 visible-information candidates and 32 distinct hypothetical assignments,
with at least eight useful distinct worlds per generated candidate. Its trial
horizon is 256 accepted inputs, including nested choices; rule steps are counted separately. The measured native lifecycle includes
capture, sampling, validation/mapping, rollout choices, aggregation and disposal;
fixture construction and trusted artifact encoding are outside the decision
stopwatch. Whole-process memory includes those additional allocations too.

All **60 primary attempts** passed on the reference Ryzen 9 9950X with four
workers pinned to distinct physical cores. Nearest-rank p95 (57th of 60) was
**2,719,864 microseconds** against the 3-second requirement; the slowest was
**2,990,379 microseconds** against the 5-second hard limit. The stronger
whole-process high-water bound was **311,537,664 bytes**, below 2 GiB. Every
candidate met its eight-world minimum. No engine, policy or job fault occurred;
quiescence, source hashes and executable identity were checked.

Independent native replay verified **all 60 reports and 3,859 useful trials**:
3,834 horizon evaluations and 25 terminal results retained exact counts, outcomes
and full State hashes. All **3,341 NotStarted** diagnostics remained non-useful
and were not represented as verified rollouts. All twelve fixed-first-eight
sets and selections matched across repetitions, as did 3,022 overlapping useful
trial comparisons, including optional coverage. These are finite workload
results, not evidence of strategic strength or arbitrary-root support.

Original frozen identities:

- Build: `255fbcd727ef3b22a613a23c717b2c363402bdb0c64663cb817f9d2cc829cb17`.
- Executable: `1d1e7e56d972b9e29d57f0836bb031459356d3ef001585e6041be37580e08732`.
- Primary report: `2c347162a553b8797204db2e08ccd6f367bf614524da154c6aac5706d9fb56dd`.
- Independent all-sixty audit/replay: `ddd656560e2dae3b024859fdec0879ce1d74bb8ed1417c9846566882997545cc`.

The **166 native files / 52,121,033 bytes** and nine host inputs remained frozen
through measurement and independent replay. Formatting, warnings-denied Clippy,
**417 Rust debug tests and 31 Node checks** passed; the separate release suite
also passed all **417 Rust tests**. Fresh retained-source offline rebuild/replay
and the independent retained-bundle audit passed. P7 is released for this frozen
workload. No later server or engine build inherits this exact performance
result by profile name alone.

Evidence remains under ignored
`generated/search-staging/generated/search-verification/`, including
`primary-60-run-1`, its external wrapper report and
`independent-primary-review.json`. Historical P5 artifacts and unsuccessful
development measurements remain separate and preserved.


The P7 final gates used the unchanged 166 native files / 52,121,033 bytes and nine
host inputs. Development checks took 1,756.302 seconds, the separate release
suite 912.376 seconds, and fresh offline retention 1,404.952 seconds. These are
verification durations, separate from the decision latency above.

Retained release identities:

- Store: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-a70d95584dbb10cc-hgUTKX`.
- Native bundle: `a70d95584dbb10cc8e13eaf1e6caca80e3862757096edb79be2109903bd27bb0` (2,087,128,814 bytes).
- Offline executable: `a9894e427db72e1b4c319d5c845b9cf21f3c171434d3a90580a2684f928ba572`.
- Dependency manifest: `9b3e439c94ceaf07c828ebf37a4bcd513a345248345c0f84ad0893a8536e7989` (43 external packages / 5,596 files).
- Independent bundle audit: `095837621170a5b1996b9ccca28c425418a28799b2de767618e03c78ac4f6a77`.

The audit independently checked all eleven catalogs, 41 source records, 42
historical configurations, 540 trace executions, 727 snapshot/hash checks,
17,627 recovery boundaries and all nineteen complete legal-match journals.
The historical initial-scenario State hash remains
`58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.
The parser compatibility capture remains the separately identified historical
199-file parser capture; no current-main parser result is inferred from it.

The offline executable and measured executable have different file hashes while
binding the exact same release Build and native source closure. Performance
claims remain attached to the measured executable above. The original process
inventory matched exact tool names; the later current-build wrapper additionally
recognizes Node26's `node-MainThread` name. Original raw measurements remain
unchanged.

The original sixty-root process took **167.976528 seconds** with
**593.735269 user CPU seconds and 0.642042 system CPU seconds**. Dividing its full
harness denominator gives **0.357193 search roots/second** and **22.973448 useful
trials/second** for the 3,859 useful trials. These are batch diagnostics for the
exact original executable, including trusted preparation/artifact work outside
the consumer decision stopwatch. Horizon evaluations and terminal continuations
are not complete pods from setup, and accumulated worker CPU time is not a
measured parallel speedup. The original wrapper/review are retained under the
P7 external store above and copied into the hosting evidence's
`historical-batch-diagnostics/p7/`. The joint capture contains eight P5 and two
P7 payload files; `historical-batch-diagnostics/capture-manifest.json` has SHA256
`d6fc31f59b9d5f652cfdcc95bf7f66fbc546ddd87d10a010c28116ecb922b421`.
The subsequent P8 build passed its own latency acceptance. Its separately scoped
whole-batch totals appear in [the current-build search report](game-engine-search.md#current-hosting-build-search-verification); the P8 release evidence is below.


The exact measured executable, all sixty raw reports, every independent replay
output, the reviewed drivers and host inputs are retained outside the worktree
under the store's `search-acceptance/` directory. Its 211-file / 25,056,417-byte
manifest SHA256 is
`baea4998acdcc7d3f034cbe8a4b5f67562a6d7f8b3a9c2f9cf733c1cfeaa034c`.
All archived bytes were verified against the reviewed evidence before and after
copying; the parent store retains the native source/vendor/offline replay closure.


### P8: finite local hosting verified

The local Rust server and generated TypeScript SDK release
`commander-foundations-v1` over loopback HTTP. Rust DTOs remain protocol authority;
authenticated per-seat streams, durable idempotent commands, generation fencing,
clocks and bounded queues remain separate from rules outcomes. No deployment,
lobby, spectator policy, search endpoint or arbitrary-deck support is claimed.

The exact native build is
`7e77708cad1f80dfa7ad3be82d198f3ad6eae7f62b9c127879a335206bbc8da5`,
with 192 native files / 52,558,543 bytes, eleven host inputs and 23 SDK sources.
Measured server SHA256 is
`0becfed3cff7cf0882c634e3a4124dd201ce914c54c6516d3f6bb2fbdf26ec28`;
public schema file SHA256 remains
`7b69b76a7eeb8ae6743664405ca735ca539200533295fb500c27da85cc3fb95b`.

| Verification | Result |
| --- | --- |
| Engine development gate | 526 Rust tests, formatting, warnings-denied Clippy and 40 Node checks passed |
| Separate release gate | 526 Rust tests passed |
| SDK | 40 tests and strict TypeScript passed, including fresh relocated offline verification |
| Four actual independent SDK consumers | Natural Win(P2), 65 turns, 2,389 accepted/consumed inputs, 325.489 seconds, zero retries, no CoreConcede |
| HTTP smoke / backpressure | 107 / 55 checks passed |
| Normal completed-game recovery | Ready in 101.712544 seconds under unchanged 120 seconds; all seven tables/head/State exact, generation alone advanced |
| Current quiet search | All sixty jobs passed; p95 2,662,119µs, maximum 2,690,433µs, kernel whole-process upper peak 394,698,752 bytes |
| Independent search replay | All sixty reports; 4,188 useful trials verified, 3,012 NotStarted entries excluded |
| Fresh native retention | Empty Cargo home, frozen/offline rebuild and full replay passed in 1,199.535326 seconds |
| Fresh server/SDK retention | Empty server Cargo home, 92 server tests, offline SDK install/40 tests/strict TypeScript passed in 198.188076 seconds |
| Independent native audit | 151 external packages / 10,704 vendor files, eleven catalogs, 42 configurations, 540 traces, 727 snapshots, 17,627 recovery boundaries and nineteen legal-match journals checked |
| Independent server audit | Exact SQLite/toolchain, 20 locked npm tarballs, 53 cache files, 23 SDK sources, ten fresh generated artifacts and one native test fixture checked |

The completed game has full State SHA256
`0dacaf403068c8dd452e4b253e42f902525b2a03bf6f3c6db6bb5942a3b7b728`,
at durable sequence 4785 / core boundary 6849. Game/recovery worker peaks were
1,049,440,256 / 914,870,272 bytes under actual 4 GiB / swap-zero / 32-task limits
with zero OOM kills. Recovery used a byte-exact closed copy; the original registry
was hash-checked without opening its SQLite database. Owned process groups closed.

The first permission-fixture attempt, overnight cancellation, npm 12 direct-URL
cache rejection and flat SDK test-layout failure remain preserved. The final
companion revalidated four exact passed predecessor phases under their original
freeze/logs; it ran only server retention under the new two-host-file freeze.
All 192 native files and 23 SDK sources remained unchanged. Fourteen focused
host guards and independent transition reviews passed. This qualification does
not relabel the earlier commands as runs of the new host scripts.

The two vanished `/tmp` historical reports were recovered from unchanged external
P7 fixtures by exact canonical encoding; both reconstructed hashes match their
pre-existing recorded authorities. No fresh engine execution is inferred.
The fresh generated SDK has ten required files; an older eleven-file development
directory additionally held an ancillary focused-test log. The final audit checks
the exact ten-file set. Failed audit/driver sources and reports remain retained.

Retained store:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-11fdee9d9b50bb8d-2Y0CLl/`.

| Artifact | SHA256 |
| --- | --- |
| Native bundle, 2,087,587,224 bytes | `11fdee9d9b50bb8d2e4fb345dfc6bbc5f05a04a16dcfd919cf48e5976c7f9ce4` |
| Offline simulator | `731d7690b3367f9eca28bf3af3620ad3caf21461d616f9fa80ea76a56de46ed8` |
| Final native artifact audit | `d1fddb2e4e818de9252757efc63f1d25f288874fa94f8a2dc95af123f97bb4ca` |
| Final server artifact audit | `39546b1f84c1890eaac013f136c6c355826a6d8a6f97259e0987b86bd2082fa8` |
| Server/SDK evidence, `server-b6cUNM/evidence.json` | `87a7a5ac13d9e20d8448d6eb10c4ee84f6d18b69978afce44466abf03bb5afb0` |
| Final five-phase status | `da1c4f116af4c7a6c2fd505f8dcbd3f5a16bebefa0391098df875b6aa3aebbc6` |
| Final source freeze | `de9a6dd3350421cdb3a19df795fb7a246b3c5387f4e0f300ce2f93b8fa047c4a` |
| Private `server-proof-archive/manifest.json` | `a11f203b1cff1334c57e13655a7f93549c138e77dd00c69596238085d600952a` |

The final private proof archive holds 5,522 files / 3,335,347,670 bytes with exact
source/destination hash maps verified before and after copying, private modes,
and no active-prefix omissions. Native source/vendor/offline outputs and the
separate server/SDK reconstruction remain alongside it in the same external store.
Original measured binaries retain their own performance claims; rebuilt file
hashes differ while compiled native Build identities agree.

The scoped server release does not complete the broader performance program.
Named dense interactions/history work, retained-branch stress, fixed-work worker
scaling and the same-strategy labeled omniscient diagnostic remain required.
Current retrospective CPU/allocation profiles and their timer-change warning are
recorded in [the search report](game-engine-search.md). Historical preoptimization
CPU/allocation profiling chronology remains unavailable; no causal ratio or
playing-strength claim follows. P6 remains separately bounded releases.

## P8 release installation — October 17, 2026

Local commit `01f098beddf6005442cd8b0f4de1f5acfae93be6` installs the verified
hosting release: all 236 approved source/document paths were checked against the
candidate and then against the committed Git blobs; 79 paths changed. The
installed SDK passed 40 tests and strict TypeScript, with identical public schema
bytes. The installation archive is `release-installation-01f098b/manifest.json`
under the external store recorded in `game-engine-server.md`, SHA256
`59e7c30fa51bbe2febe0aaa6d8cd626eddfdc6ac3f352436efe5048122c65e7e`
(447 files, 148,989,525 bytes).

The machine's newer Rust packaging revision initially produced a distinct build;
that successful compilation and limited SDK check remain separately qualified.
The original signed Rust `1:1.99.0-1` package was recovered from the local cache
into a private directory, without changing the system compiler. A second actual
root build reproduced every original NativeInfo field, including Build
`7e77708cad1f80dfa7ad3be82d198f3ad6eae7f62b9c127879a335206bbc8da5`,
and passed the same schema/40-SDK/strict-TypeScript checks. Its compiler archive,
signature, binaries and records are retained in
`original-toolchain-rootrebuild-01f098b/manifest.json`, SHA256
`dac7eacf226b16ce26572baf0378d36e8545912136bd31ff60a9448c6bc9a1a5`
(22 files, 126,024,113 bytes). Timing claims remain attached to the original
measured executable, not these path-specific rebuilds. The original complete
gate/proof archive remains closed and unchanged. The documented broader
performance work is still required.

## Broader performance diagnostics — October 17, 2026

The [performance report](game-engine-performance.md) records optimized native
diagnostics for dense interactions, artifact batches, permitted versus labeled
actual-world decisions, ordinary legal transitions, real retained branches,
persistent worker generations and complete-pod attempts. Each result retains
its exact source/executable identity, denominator, resource boundary and failure
qualification. Synthetic geometry and horizon trials do not become legal games
or independent useful-world rollouts.

The current 201-file diagnostic candidate, Build
`3118eff2f2f4abcc2df5f50bacb001c5510a7761d4edeb8d4093f2df8d4d8eee`,
passes 37 unique focused baseline tests, a counter arithmetic guard, formatting
and all-features Clippy. All eleven explicit host-guard scripts pass 162 tests.
All 563 Rust debug tests passed. The original combined gate then failed because
the staging copy lacked the parser sources required by its Node fixture check.
After restoring 86 exact committed parser dependency files, the Node-only retry
passed all 45 checks. The original failed log remains unchanged; this is a
composite verification. All 563 workspace release tests now also pass. The SDK
passes 40 checks and strict TypeScript with exact existing public schema bytes.
The first fresh retention attempt stopped before offline compilation because
the staging copy lacked two pinned policy HTML files. After restoring the exact
archived bytes and verifying both fixture hashes, the retention-only retry passed
its fresh frozen offline native rebuild and complete bundle replay, retaining
10,716 dependency files. Its original failure remains unchanged. This establishes
retention for the 201-file Build above, not the subsequent optimized candidates.
The successful raw log SHA-256 is
`73af24a603d0d81d2e3acd07c688abc020a5ff93bf01f6e3826ab85a7848645d`;
external evidence is in
`performance-retention-201-v2/p2-90cb345b513ec31e-S8nEjr/evidence.json`
under the private engine-evidence root.
The source/executable archive manifest is
`performance-native201-3118eff2/manifest.json`, SHA-256
`f329420c48c87711f14e571f00937a769ac849b22852927afaee3b481db6ab16`.

The current ordinary 1,024-input prefixes and dense-trigger retries preserve
complete reference State/journal/outcome semantics inside their resource
envelopes. A separately identified test-only Rust request counter measures
9,243,136 allocations over the unchanged ordinary core interval. Four persistent
workers achieve a same-build median steady-execution speedup of 3.7360 times,
with complete serial-oracle equality and independent raw-result review.

Memory work is still open: the every-branch ordinary warmup creates 10,000
handles but completes only 6,137 verified first writes before its RSS reserve;
the outer charged-memory guard rejects that run. Natural dense setup is OOM
killed before reaching 256 battlefield objects, and the first complete-pod
warmup is OOM killed before a game finishes. There is no complete-game
throughput result from that attempt. Raw failures remain immutable alongside
successful diagnostics in `performance-third-measurements-3118eff2/manifest.json`,
SHA-256 `f00332750de3c9fadb2a68e4a62390c712326ff473e025388887af19395a0663`.

The separate admission-sharing candidate, Build
`1f57d2e2fcedecde548abef7e5cd348b1c711269cc1d36a5a640df7051e2974d`,
shares immutable Prepared data internally. It passes all 246 release core tests
(including five new admission-sharing tests), 17 distinct simulation tests, a
counter arithmetic guard and core Clippy. The focused tests are a subset of the
246 core passes, not additional tests. Independent review verifies source and
Cargo artifact identities; its simulation per-filter invocation association is
qualified as parent-reported. No performance or complete-workspace acceptance
is claimed for this component. Its external archive is
`performance-native202-prepared-1f57d2e2/manifest.json`, SHA-256
`8984dfbce62314b911295b589a1fd447f59663d1f63844d122dba75a09db7f74`
(243 files, 222,793,970 bytes).

The separate 209-file compact-history and streaming-snapshot candidate, Build
`cde7cdfa57922f14b4582cd3cf2b7561239838a67ee7a00cebf5b0ac1f1ed799`,
passes all 262 release core tests, four strict-reader contract tests and 19
public history/search integration tests, plus formatting and affected-target
Clippy. Its 64 focused core checks are a subset of the 262, not additional
passes. An independent review verifies the integrated source; the actual
production and both simulation test variants are compiled and pinned. The
external archive is `performance-native209-memory-cde7cdfa/manifest.json`, SHA-256
`9019167f4984a97c71de52b8b317600b04fc4e98ac4038429718a70e2fe708d9`
(269 files, 226,733,688 bytes). A subsequent run passes 17 distinct simulation tests and a counter arithmetic
guard with exact argv/executable/feature records. Both the prepared-only and
compact-history counter executables pass their bounded known-vector correctness
control. Those controls ran during concurrent functional work and provide no
quiet timing comparison. Full workspace, schema and performance/resource
acceptance remain pending for that component.

Definition-diversity, shrinking-choice and shared permitted-history exports are
being compiled and tested in further isolated candidates. A real diagnostic
test rejected an invalid synthetic combat assignment; its corrected fixture
follows pinned CR 510.1a and archived card data while retaining the failed run.
These further changes have no complete-runtime or resource-fit claim yet. The released finite profile,
private-information boundary, all canonical history bytes and existing search
budgets remain unchanged.


The separate complete permitted-history export component, Build
`c9f5f5942ff44c6f635f1c9e4c4756a2a43907fd9323ad994fc9994f659c1d39`,
passes all 271 release core tests and 19 public search/history integration
tests, plus typecheck, formatting and core Clippy. The nine new tests and
affected focused core tests are subsets of the 271. Independent application
review passes. Its exact source, core executable and evidence are retained in
`performance-native213-shared-c9f5f594/manifest.json`, SHA-256
`e7ae515bdad0d8a094e4c88c15110b15b328252714602ec46dbaa7a9233c7470`
(261 files, 77,008,568 bytes). Large query-proof Values, complete SavedWork
Strings and internally tagged decode buffering remain separately tracked;
no memory-fit or end-to-end acceptance is inferred.


The Prepared-sharing component is now installed in the engine worktree. A full
`cargo check --workspace --locked --all-targets` also passes, including compiler,
simulator and server consumers; its frozen 202 native inputs and four retained
release executables remain unchanged. This supplements the component correctness
results above and makes no performance or complete-release claim.


The compact-history and streaming-snapshot component is now installed after
its focused and complete-core correctness checks above. The complete Rust
workspace also passes `cargo check --workspace --locked --all-targets`, including
server and simulator consumers. All 209 frozen native inputs and four retained
release executables remain exact. Combined performance/resource acceptance and
final full-workspace runtime verification remain open.


The complete permitted-history export component is now installed after all 271
core tests and 19 public integration checks above. All workspace targets also
typecheck, including server and simulator consumers. Public histories retain
their complete array values and canonical bytes while sharing immutable storage;
no hidden State authority is included. Full release verification and resource
measurements of the combined candidate remain pending.


## Definition/choice diagnostics and complete query proof storage

The reviewed combined 218-file candidate has all 284 normal core tests passing
in the optimized test profile (optimization 2, assertions and overflow checks
enabled); four explicit diagnostics remain ignored. Its release Build
`d472788f8c8cd5c15137b3f08b29e304b7495cfe363614cbda53cc304725c903`
is separately compiled and source-verified by the actual simulator collector.
All 23 affected core and 21 simulator normal release checks pass, including
the nine new query/file-proof tests and four shrinking-choice checks. These
focused counts do not constitute a complete workspace release test result.
Independent source/application review passes (receipt SHA-256
`e4e950f61f5b785a55a2615fd5f9307d286c0ff034215d7d8a2733a0951eb585`).

Large Search reference checks now spool one generic public view per original
work unit and compare every original complete prefix, result and SavedWork
envelope byte. Legal-branch standalone restore consumes a private snapshot file
with both strict-reader passes bound to the complete write receipt. Original
unit budgets, full State/trace/replay checks and successful cleanup remain.
These are test-host proof-storage changes; quadratic I/O, page cache and other
full-String consumers still need actual resource measurements.

The definition-diversity and shrinking-choice native diagnostics and their
49 reviewed Python guards are installed. The separate frozen 205-file definition
component actually passes all 48 declared rows under the original 120-second
and 2 GiB charged-memory/independent-RSS guards, including its in-deadline audit
and process cleanup. This run was a correctness integration during concurrent
functional work, not a quiet timing comparison. Its external archive manifest is
`performance-definition-instrumentation-fca4063e-v1/manifest.json`, SHA-256
`c2ab1c7b4ad95693adf4378624e33aaafea9c97ad4900921d15139e8501468df`.
Synthetic distinct modifier payloads are separate from eight authentic source
programs; there is no 256-authentic-ability or expanded-pool claim. The complete
shrinking-choice workload has not run.


The final SIM generic-proof component is independently reviewed and archived in
`performance-native215-sim-proof-1a47ea39/manifest.json`, SHA-256
`580b98a7f230cfc1153a62d4e2844c2fc93abcf8650e85f7ef875e6ba7174970`.
Its 14 focused normal tests and Clippy/format checks pass on the exact isolated
source. It streams every generic history view and preserves all other State
fields, array order and complete byte comparison against the bounded old
String/Value oracle. The reviewed patch is now integrated with the earlier
diagnostics and query proofs as 220 native inputs. This combined source passes
all 622 workspace tests in each of debug and release, with 12 diagnostics ignored
in each mode, plus 45 Node checks and 40 SDK checks with strict TypeScript. The
debug Build is
`d45d878d4fa8bcddc1298f73f2e104f4ba41c6c592039eedfceff20ae17d8edc`;
the release Build is
`0398e42f215517220e4411d73e908c52c8132220196c082dd0f7c030b67e56c8`. Format and Clippy checks pass; public schema bytes remain exact.
All ten verification process groups are reaped and empty, and source guards
remain equal. The terminal functional receipt SHA-256 is
`7c6468ec443e3d8fea1394cb052ee66b01c81a1933cc68108bacc74956401a64`.
The 14 integrated focused release tests also pass, receipt SHA-256
`3915aa4664b7a66347b308410d9346890d0dc4343e2e088f9941299d94fc63c4`.
Fresh retention subsequently passed for this exact release Build. The unchanged
retainer rebuilt all 220 native inputs with a fresh Cargo home and target directory,
using 10,716 vendored files from 152 packages, and verified the complete retained
bundle. The new store is
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/performance-retention-220-v1/run-1/p2-151d7ee801d1e975-mGNjaB`.
The bundle SHA-256 is
`151d7ee801d1e975412e178b361b5220ad0fe43bacf32bfec1430e26b79d6fef`;
the rebuilt executable SHA-256 is
`8ae5b8438a281a5410325b8e36461d85c9656442976392ead90cd2dd048e0bd7`.
The independently reviewed audit hashes all 11,461 retained files and recomputes
the current Build from the retained source and actual compiler settings. Its
SHA-256 is `ac76e36969e550a033a8e5c5b39e7527b67c501940f03486e9d4c919a1bd15b8`.
The initial empty Cargo-home check remains the pinned producer's observation;
the audit did not repeat the rebuild. Original functional receipts remain
unchanged. All final-source performance measurements remain separate work;
these correctness results make no product resource-fit or timing claim.

Parser coordination for the next chosen slice and eventual main integration is
recorded from the parser agent's reported commits `e5a6761` and `5030267`. Run the
current main parser on candidate cards and compare Rust reference binding with
the TypeScript certificate. Record concrete parser blockers or disagreements in
`docs/parser-requests.md`, including Oracle ID, AST path, required shape, pinned
CR citation and whether the slice is blocked; notify Josh when entries exist.
Keep parser fixes on main with the parser agent. When integrating main, repeat
compatibility checks against its actual AST, including trailing-if
`conditionPosition: "after"`; historical fixture checks do not establish that
compatibility. No new card slice or parser request is introduced by this work.

### History capacity/equality candidate — October 17 night checkpoint

The candidate keeps private immutable history equality proportional to changed
entries when allocation identity proves equality, with complete typed comparison
for independent rows. Generic `History<T: PartialEq>` keeps its original behavior,
including nonreflexive NaN comparisons. A fresh narrow destination vector also
avoids retaining the wide source allocation during observation-row conversion.
The controller helper now preserves the existing user-bus locators needed by
the guarded pod launcher; native payload environment restrictions are unchanged.

Both debug and release workspace compilation passed for the exact 220-file,
53,205,999-byte candidate. All eight new focused tests and fourteen existing
focused simulator tests passed; the host guard suite passed all 36 checks.
The capacity test measured 288-byte public rows and 8-byte stored row slots:
256 rows without source spare capacity retained 73,728 slot bytes with the old
conversion versus 2,048 with the fresh vector. These are vector-capacity facts,
not whole-process memory-fit or throughput measurements. Wire equality, deep
fallback, memo invalidation and copy-on-write isolation remain checked.

Josh requested a stopping place for the night while `check:engine` was running
its long retained-bundle test. The owner received SIGTERM through its reviewed
cancellation path; all twenty phase leaders were reaped and their process groups
were independently confirmed absent. The raw attempt-2 status remains `failed`
with `InterruptedError`, recording the requested cancellation. The full
debug/Node gate is incomplete; the full release suite, SDK/counter completion,
fresh candidate retention and candidate performance acceptance remain pending.
Earlier baseline Build `0398e42f…` passes do not transfer to this candidate.

The private checkpoint is
`generated/final-engine-verification/night-pause-20261018-v1/pause.json`, SHA-256
`39b7b3bb9762935feab3a82ddc2c7eee3a873438656292ce7361d0112103b7e0`.
It pins all 141 preserved run files, six source snapshots, source equality and
cleanup checks. The raw cancelled status SHA-256 is
`6fc3ae650deae1fa7f2f0f6e4e924a98a8980911fc151ef81314025c3ecee566`.

On resume, preserve both earlier attempts and prepare a fresh attempt-3 output
and reviewed producer/release. Explicit cache reuse is possible, but rerun every
required gate and do not turn the interrupted full-suite run into a pass. The
staged identity, retention, archive, measurement and primary-authority adapters
are bound to attempt 2's success path and must be rebound and reviewed for the
new producer/status. Then complete independent functional audit, fresh offline
retention and archive audits before unchanged workload measurements and all
sixty primary search jobs plus sixty independent replays. No new pool, parser
edit, budget change or background workload is part of this checkpoint.

### History capacity/equality candidate — October 18 verified pause

The fresh attempt-3 verification now passes for release Build
`a742650f4d4cfadbec67480901732880e29095a053ff5bd07f3e2b7975df06c3`:
630 Rust debug tests plus 45 Node checks, 630 Rust release tests, and 40 SDK
tests with strict TypeScript. Twelve ignored diagnostics remain excluded in each
Rust mode. Formatting, Clippy, all eight new focused tests, fourteen existing
focused simulator tests, five normal vectors and the separate allocator control
also pass. The independent functional audit SHA-256 is
`394de68c9df992d14f1e1e23a2f77c89dbc3c95a9bc255071688c944af66bb14`.
All 220 native source inputs stayed exact. The normalized compiled-identity
receipt SHA-256 is
`3a3de2bf92012b5f711f36a177e2c16cd6089aed5c41a1fdcfd5fa4bca02f4ff`.

The host toolchain changed globally during the pause. Verification used the
original pinned Rust 1.99.0 and Node 26.11.1 binaries restored privately from
signature-verified cached packages, without changing the global installation.
The first independent audit rejected an expected npm compile-cache directory;
its failure remains preserved. The corrected, independently reviewed auditor
permits only the exact 93 hashed cache files and rejects unknown residue. The
original functional logs and status were unchanged.

Josh requested another night checkpoint during fresh offline retention. The
reviewed supervisor received SIGTERM, reaped its child and emptied its owned
process group; the supervisor and group were independently checked absent.
The raw retention status remains `failed_preserved_no_retry` with cancellation,
and its logs and partial external store remain preserved. This candidate has
no fresh-retention or performance acceptance yet. Passed historical baseline
measurements do not transfer to the new Build. All existing subagents are idle.

The private pause receipt is
`generated/final-engine-verification/night-pause-20261018-v2/pause.json`, SHA-256
`9b5d97e4cce88ffc9962690d94f2a9df5e4acd767e70f67a028bd0c162bf4448`.
Resume instructions and reviewed adapter locators are in
`generated/final-engine-verification/active-continuation-v23.json`. Preserve
the cancelled run; use fresh retention output and evidence directories after
the existing guards and independent preflight. Then complete retention and
archive audits before the unchanged full workload measurements and all sixty
primary search jobs plus sixty independent replays. Keep the existing budgets
and parser coordination requirements. No engine or parser source changed in
this checkpoint.

### History capacity/equality candidate — October 18 resumed retention

Fresh retention now passes for the same `a742650f…` candidate, following the
explicit goal resumption. The cancelled attempt remains unchanged. A new
producer target and the retainer's fresh Cargo home/target rebuilt the exact
220 source inputs offline and verified the full retained replay in 779.611
seconds. The bundle SHA-256 is
`26e80ebd16e1abe3317a77c45591ac65711ab844c65121a12bbbca7b039717d1`;
the rebuilt executable SHA-256 is
`e1943970e9020f88073ad045b703a12c35eaa74e0e0605d273e449cba7e90151`.
The store is `candidate-offline-retention-run3-v1/run-2/p2-26e80ebd16e1abe3-ZHjw0D`
under the existing external evidence root. Its independent audit checked all
11,461 files, including 10,716 vendor files from 152 packages; audit SHA-256
`0d2b624a6595b21d2f1c3f8b9df4eb170eb500b3fe5f813f5c1f414ea940253f`.
The initial empty Cargo home remains a pinned producer observation.

The separate source/functional archive contains 1,033 files totaling
1,129,452,452 bytes, including current source, tools, executable roles, host
code and passed/failed/cancelled evidence. Its external directory is
`performance-candidate-source-functional-a742650f4d4cfadb-v1`; manifest SHA-256
`8dfed18be105f92f252326d3c00c11529402d5ddac0b8e77269c18d3df8253d2`.
The independent archive audit SHA-256 is
`51b2be51d35ea75f5c4ac20787d6e4acc4cded59f320db46d13d6d59b8c2e3c4`.
All original/copied bytes, namespaces, private modes and candidate anchors
match. These functional gates do not establish product resource fit or timing.
Current-build correctness references and the unchanged eight-suite diagnostic
and primary-search metadata are prepared; their reviews, actual measurements
and independent replay remain separate pending work.

### History capacity/equality candidate — October 18 measurement checkpoint

The same 220-file Build `a742650f…` now passes all sixty primary search jobs
and all sixty independent semantic replays. Nearest-rank p95 is 2,610,138µs,
maximum 2,633,648µs, and the kernel whole-process RSS upper bound is
213,286,912 bytes. There are 5,218 useful trials (5,178 horizon completions
and forty terminal completions); 1,982 unstarted trials are excluded. Every
generated candidate meets the eight-distinct-world minimum under the unchanged
five-second maximum, three-second p95, 2 GiB and four-worker product limits.
The independent review is
`generated/final-engine-verification/candidate-primary-runs-a742-v1/run-1-independent-primary-review.json`,
SHA-256 `c6d5e84de3f0c3743b74965067f6b3220427074bee0aa0b84ece8241e269c39c`.
The original producer wrapper retains its earlier replay-pending status; the
separate review supplies the completed replay proof.

The broader unchanged thirty-case diagnostic program has producer-verified
completion for indices 0–16: ordinary work, artifact production, twelve kernel
and interaction cases, and the four-seat selected/first-write and three-seat
selected legal-branch cases. Each completed legal case includes all twenty-four
sweeps through 10,000 genuine branches. The four-seat first-write case now
finishes in 77.734 seconds under its original 120-second limit. Ordinary case 0
also has an independent actual-output audit, SHA-256
`b46c535e4a9972b65de751eb86a5e97bab8229aa03211960f7a40e42ee8cde3e`;
this is a verified subset, not an independent pass of the entire program.

Index 17, three-seat ordinary first-write, exceeded that same deadline after
nineteen of twenty-four complete sweeps. Its original failed/truncated result,
partial progress and raw logs remain unchanged. Charged cgroup memory peaked
at 1,632,460,800 bytes with no OOM event; this failed host receipt does not
provide process RSS. All eighteen attempted resource groups have been reaped
and are absent. Indices 18–29 remain unstarted. Separate allocation and
omniscient measurements, the remaining independent diagnostic audits and final
measurement retention are pending. No full-program performance pass is claimed.

Josh requested a night stopping point. No implementation or parser source
changed during this checkpoint; all 220 native inputs (53,205,999 bytes) were
checked against their frozen hashes. Resume from
`generated/final-engine-verification/active-continuation-v36.json`. The private
pause receipt is `generated/final-engine-verification/night-pause-20261018-v3/pause.json`,
SHA-256 `ce23848e84322322d2cb8a3239a0e95a12b600f5eeb3f406f7b64d46bcfe541d`.
First inspect the saved stage-cost diagnosis and profiling readiness, then
profile the three-seat timeout before choosing the next optimization. Preserve
the full work, resource limits, failed outputs and unrelated main parser work.
The incoming background completion for the historical 201-file Node/release
logs was hash-checked separately; it does not establish this Build's gates.

### Capacity/equality a742 — completed evidence before successor verification

The a742 functional result remains 630 debug / 630 release Rust tests, 45 Node
checks and 40 SDK checks with strict TypeScript; fresh offline replay and the
sixty-job primary plus all sixty independent semantic replays pass. The terminal independent audit `e85d599e…` verifies all seventeen completed
cases 0–16 and preserves case 17 deadline-limited and
case 20 deadline-limited during setup before any sweep. Case 20 is not an OOM
or a completed ready-root proof. Eleven cases and separate omniscient remain
unstarted; no full-program pass is claimed.

The separate full 1,024-input Rust allocation phase and zero controls pass, with
7,171,042 allocation requests and 100,869 reallocations; the independent subset
audit is `fde4ad99…`. The separate legal17 hardware capture remains a partial
18/24-sweep diagnostic, reviewed as `a80e1469…`. It supports investigating the
proof comparator's repeated zero-fill, without assigning exact CPU-time savings.

The old source/functional, measurement and profile archives now retain 1,033,
600 and 167 payload files respectively. Exact manifest hashes, byte totals and
qualifications are recorded in [game-engine-performance.md](game-engine-performance.md).
All cancelled and failed receipts stay unchanged. The successor's collection
sharing and comparator scratch changes require separately identified verification
and unchanged-workload measurements; no old pass transfers to it.

### Shared collections and scratch: source closure and measured search

Source commit `92a7863`, release Build
`faf9ca241b7e1bf9469a77c9d88080ef18501bc851dd107811a758c24afa7318`,
passed 637 debug / 637 release Rust tests, 45 Node checks and 40 SDK tests with
strict TypeScript. Independent functional audit `a56edc5c…`, identity
`6f857743…`, fresh offline/full-replay audit `82f6d9be…` and source/functional
archive audit `d4e77633…` are closed. The source archive retains 893 files /
945,588,802 bytes (manifest `860a6a40…`).

The implementation shares equal immutable predecessor-view arrays while retaining
complete public wire values and owned exports, and reuses one 64 KiB comparator
scratch buffer per comparison. Seven new normal tests preserve full equality,
mutation isolation and error/EOF behavior; they do not establish a speedup.

Ordinary case 0 completes the unchanged 1,024-input prefix with one warmup and
five measured repetitions, full semantic proofs and verified resource cleanup.
Independent audit `2d782f41…` verifies this case only; the thirty-case program
does not yet pass. The separate allocation phase now passes the same 1,024-input
semantics against the current feature-OFF reference and both zero controls:
6,944,647 allocations, 6,913,186 deallocations and 100,869 reallocations,
independently audited as `a9377d54…`. These are process-global Rust request
counts, not live-heap or production-latency evidence.

The current primary sixty jobs and all sixty independent semantic replays pass
unchanged budgets: p95 2,619,498µs, maximum 2,630,390µs and whole-process RSS
upper bound 174,325,760 bytes. Review `68f460b8…` verifies 5,501 useful trials
(5,461 horizon, 40 terminal) and excludes 1,699 unstarted trials, with the
per-candidate eight-world minimum preserved. The raw wrapper remains
replay-pending; the independent review establishes completed replay.

Broader cases 0–17 complete; case 18 retains its 120-second timeout after 22/24
full sweeps (`f3d1a818…`). Cases 19–29, separate omniscient and final measurement
retention remain pending completion and review; no full-program pass is claimed.
Exact proof locators, requested-byte counts and timing scopes are in
[game-engine-performance.md](game-engine-performance.md); no a742 pass transfers.
