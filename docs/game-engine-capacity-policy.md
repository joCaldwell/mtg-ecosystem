# Engine structural capacity policy

D1b-3 corrects the impractical D1b-2 ceilings. These are executor resource
ceilings, not claims that every admitted game terminates within them. A ceiling
never determines a Magic winner. The historical limits and measurements remain
qualified by their original release in the progress ledger and slice documents.

| Resource | Limit | Accounting |
| --- | ---: | --- |
| Owned physical cards per player | 100 | Snapshot/wipe validation bound; deck size is enforced separately by admission |
| Live allocated tokens | 256 | Existing token objects |
| Retained token identities | 65,536 | Includes ceased tokens, whose history is retained |
| Damage occurrences per transaction | 2,048 | Entire simultaneous prepared damage group |
| Stack plus pending triggers | 262,144 | Combined entries, before any new entry |
| Event-group nesting depth | 4 | Checked before recursive collection |
| Event-group occurrences | 1,024 | Total leaves across ordered and simultaneous groups |
| Temporary effects | 256 | Current retained effects, before insertion |
| Retained history charge | 768 MiB | Newly retained encoded rows/nodes, metadata, fixed view/reference overhead, inputs and potential detailed evidence |

`engine/capacities.rs` is the runtime authority. Catalog names, schema versions,
card identities and historical publication order never select a capacity.
Checked identity/cursor arithmetic remains an independent invariant.

## Generic sizing rule

A four-seat admitted pod has 400 physical cards. At most 256 live token objects
can coexist, so at most **656 creatures** can die in one simultaneous group,
even if every physical card is a creature. For each catalog, count all triggered
abilities on nonbasic definitions and multiply by four seats. Singleton admission
bounds each such physical source to one per seat. Basic lands and generated
creature tokens in the current IR have no triggered abilities. This deliberately
counts mutually exclusive commanders, colors, triggers and source conditions.
The largest current catalog has 33 triggered abilities: **132 source instances**
per pod, hence at most **656 × 132 = 86,592** triggers for a simultaneous event.

The single stack/pending ceiling is **262,144**, more than three times that
conservative wave bound, leaving space for preexisting spells/abilities, enters
triggers and subsequent terminal token waves. `core/build.rs` recomputes this
rule for every profile catalog and rejects a build unless the wave fits below
half the shared stack ceiling and basic definitions have no triggers. The
`generic_simultaneous_trigger_bound_fits_shared_stack_policy` regression independently
checks the same rule, without names, lineage or default deck allocation.
A future catalog with more trigger sources must revisit this shared ceiling;
setting a profile flag never bypasses the rules' capacity checks.

Retained token identities use **65,536**, independently of the live limit.
The previous 1,024 ceiling excluded the [sacrifice pool](game-engine-sacrifice.md)'s
4,898-token lifetime bound; the new shared ceiling also exceeds the largest recorded finite-pool
[lifetime bound](game-engine-board-wipes.md) (33,997). Those historical bounds are sizing evidence, not
per-profile selectors or substitute admission proofs. The release probe measures
retention at the actual new ceiling by injecting ceased identities, then creating
and wiping the final 256 live tokens. Replaying tens of thousands of creation
and cessation events would measure quadratic history generation instead of the
retention boundary. The ordinary boundary regression validates the retained
identities, typed overflow and atomic rollback at the new ceiling.

The live-token ceiling stays at 256. **It remains reachable in legal admitted
games**; repeated token creation and sacrifice can exhaust it. The temporary
effect ceiling of 256 can likewise remain reachable through repeated sacrifice
pump activations in a turn. These are deliberate resource boundaries, with typed
atomic faults, not claims of complete legal-game coverage. No legal input has
been shown to reach the new retention or stack ceiling; the generic argument
above bounds simultaneous fanout, not every possible future stack episode or
arbitrary data-only catalog. Damage (2,048) and event members (1,024) exceed the
current 656-creature group plus four players; group depth is structural IR nesting.

## Cost model and validation

Every executed rules step calls `Game::fork` before mutation (`engine.rs`),
copying the owned game tables. Simultaneous event groups fork the game again
for each occurrence (`events.rs::apply_group`). Consequently a wipe's copying
cost grows approximately with occurrence count times whole-state size. Shared
immutable catalogs and shared retained history do not make the mutable game
tables free to copy. Repeated creation/cessation also grows retained token
identity data; bounding only live tokens would leave that cost unbounded.

The D1b-2 review measured a release build creating 2,000 tokens in **3.2 s**, a
single wipe over them in **35.8 s**, and an **83 MB** snapshot. Those are the
review's workload measurements, not measurements of the initial D1b-3 probe. D1b-2's
65,536-token and 4,194,304-stack ceilings could exhaust practical memory or
execution time long before reporting a typed resource fault.

The initial D1b-3 measurement used the reproducible ignored `tokens_tests::capacity_release_probe` with
the pinned Rust 1.99.0 release toolchain. It starts with a real admitted 400-card
wipes pod, then injects private diagnostic token events and stack payloads;
this is a cardinality/cost probe, not a legal-game or P7 throughput result.
Tracing is off. The probe measures creation, a complete simultaneous token wipe,
a wipe after 1,024 lifetime identities, snapshot size, and forks with 1,024
stack entries. Its fork-plus-step measurement excludes visibility refresh;
the synthetic stack is not claimed to be a reachable accepted-input state.
The ordinary regression proves the history ceiling faults before allocating
any further token identity and preserves serialized fault metadata. Existing
live-token tests independently retain accepted-input replay and atomic rollback.

Historical D1b-3 release results at the old 1,024/1,024 ceilings
(microseconds unless marked otherwise):

| Probe | Result |
| --- | ---: |
| Create 256 live tokens, 400 physical cards | 808 µs |
| Wipe all 256 tokens | 61,411 µs |
| Wipe 256 tokens with 1,024 retained token identities | 204,294 µs |
| Snapshot at 256 / 1,024 retained identities | 2,070,202 / 7,796,414 bytes |
| Whole-state fork at 256 tokens, mean of 20 | 191 µs |
| Fork at 1,024 stack entries and 1,024 retained identities, mean of 20 | 808 µs |
| Fork plus one private rules step at those counts | 510 µs |
| Typed stack-overflow check | below 1 µs timer resolution |
| Whole child-process peak RSS | 75,608,064 bytes |

The probe completed in 1.10 s. Per-operation timings are independent warm
samples; they should not be compared as a monotonic decomposition.
The historical raw probe output and separate child-process `getrusage` measurement are
retained under `generated/cards-data-d1b3/`. Builds are excluded from peak RSS.
Concurrent verification work means these are diagnostic capacity validations,
not quiet-machine performance claims.

`OWNED_LIMIT` supplies snapshot and wipe validation arithmetic. It does not
admit a 100-card deck by itself: `validate_pod` independently enforces exactly
100 including the commander under CR 903.5a, singleton names, one commander,
color identity, basic-land colors, and the selected eligibility/ban policy.
Oversized player declarations are bounded by available cards/tokens and their
validated request domains, these structural limits, and server input caps:
65,536 command bytes and 3 MiB IPC frames. Trusted native callers still own the
memory they allocate before passing an input to core.

## Fault semantics and remaining limitation

Exhaustion produces a tagged, serializable `RuleFault` with capacity and
requested usage. Retained-token overflow has its own
`TokenHistoryCapacityExceeded` fault. Mutation runs on staged game state; an
overflowing rule step retains only the fault and fault frame, with no partial
game mutation. Event groups check their total before application and propagate
staged typed faults. Snapshot validation checks both bounds and fault metadata;
restoration still proves accepted-input replay. A fault is never a rules loss,
draw, or silent truncation.

The updated release probes and their exact measurements are recorded below.

The whole-state fork per step and per simultaneous occurrence remains a known
P7 performance limitation. This change makes structural exhaustion practical;
it does not optimize that copying or bound arbitrary exact-integer arithmetic
or every future search workload. The history policy below now bounds retained
accepted-input journals and accumulated public/evidence histories. Any broader P7 performance claim requires fresh measurements
of its actual build and workload. Historical finite-pool hand proofs no longer
set runtime limits or establish current admission guarantees.

## D1b-3 review-fix measurements

Pinned Rust 1.99.0, release overflow checks enabled, tracing off. Both ignored
probes ran as isolated child test processes after compilation, with kernel
`getrusage(RUSAGE_CHILDREN)` peak RSS; builds are excluded. No other task build
or verification ran concurrently. These are diagnostic capacity costs, not a
quiet-machine P7 throughput claim. The exact executable SHA-256 is
`a0dcf22027abd477782b26b1e08629844f5c91ecf07b732d49bd3bceadc64aa3`.
Raw output and per-child timing/RSS JSON are retained in
`generated/final-engine-verification/d1b3-fixes/`.

`death_payoff_release_probe` starts with an admitted four-Yargle pod, injects
all six death payoffs per seat and the full 256 live tokens, then wipes all
276 creatures simultaneously. It collects **3,032 pending triggers** without a
fault. The ordinary 96-death regression also exceeds the former 1,024 ceiling
and resolves the complete trigger workload through checked native responses
and staged rules steps. Injection is not accepted-input reachability evidence.

| Death-payoff probe | Result |
| --- | ---: |
| Simultaneous wipe, 276 creatures | 120,464 µs |
| Fork with 3,032 pending triggers, mean of 20 | 1,123 µs |
| Whole child elapsed time | 0.239171 s |
| Whole child peak RSS | 55,820,288 bytes |

`capacity_release_probe` measures the actual shared ceilings, with synthetic
ceased identities and stack entries. These injected ceiling states are not
claimed to be reachable by legal accepted inputs.

| Shared-ceiling probe | Result |
| --- | ---: |
| Create 256 tokens, 400 physical cards | 783 µs |
| Wipe 256 tokens | 57,169 µs |
| Snapshot at 256 retained identities | 2,070,202 bytes |
| Fork at 256 tokens, mean of 20 | 177 µs |
| Wipe 256 tokens at 65,536 retained identities | 24,866,241 µs |
| Snapshot at 65,536 retained identities | 492,716,732 bytes |
| Fork at 262,144 stack entries and 65,536 retained identities, mean of 20 | 125,081 µs |
| Fork + one stabilization step (including 256 token cessations), without visibility refresh | 32,639,950 µs |
| Typed stack-overflow check | below 1 µs timer resolution |
| Whole child elapsed time | 62.673750 s |
| Whole child peak RSS | 4,109,520,896 bytes |

The enlarged ceilings are deliberately generous safety bounds. Their extreme
states are expensive: roughly 25 seconds for a full-retention wipe, 33 seconds
for the combined-ceiling stabilization step, and 4.11 GB peak RSS. They do not
establish a low-memory hosting guarantee. Immutable token-definition sharing,
whole-state forks and public-history retention remain P7 performance work.

## D1b-3 public-history correction

Retained stack observations now use immutable `Arc` nodes with shared bottom
prefixes, scoped to the same viewer and its immediate predecessor. A priority
pass retains the root; pushing allocates a new node, and popping reuses the
predecessor. A changed row or non-top removal rebuilds the changed suffix after
complete typed prefix comparison. The common trigger cascade therefore retains
O(K) nodes for K triggers, instead of O(K²) wide stack rows. Removed/reordered
rows and duplicate handles keep their exact serialized order and values.
Canonical-byte memos stream stack nodes separately from view metadata, so a
snapshot/hash cannot reintroduce repeated full-stack byte buffers. Player and
commander canonical-array memos likewise follow their shared immutable values,
rather than embedding their growing graveyard/history arrays in each view. Deserialization
reconstructs sharing incrementally. Public JSON arrays, projection/SDK fields and
viewer permissions are unchanged; internal snapshots additionally retain the
auxiliary history charge, proved by accepted-input replay and build identity.
The current Rust/Node snapshot baselines are in
`packages/game-engine/tests/fixtures/d1b3-history-scenario-digests.json`; the original D1b-2 fixture is
preserved. The retained `rebaseline.py` and `baseline-byte-proof.json` first
verify each new snapshot's complete canonical hash, then remove only the new
charge field and reproduce **all 173 original historical state hashes**. No
card facts, outcomes or public-history bytes were rebaselined to mask a change.


The ordinary K=100 hosted-attachments regression drives the complete cascade
through public `advance`/`respond`, retains over 100,000 logical stack entries,
and asserts at most 12K + 16 distinct stack nodes across all viewers. Separate
serialization tests cover complete fields, duplicates, order, target changes,
viewer isolation, warmed memos, fork edits and identical restore charges.
The manual `public_history_release_probe` measures the reviewer's K=480 workload:
a real admitted four-Yargle attachments pod, four Blood Artists plus 116
source-defined creature tokens, simultaneous destruction, then public target,
ordering and priority responses until every trigger resolves. Hosting capability
admission is checked. Battlefield injection is diagnostic setup, not a claim
that the setup was reached through accepted gameplay inputs. Tracing is off.

Pinned Rust 1.99.0 release, overflow checks enabled. The isolated child probe
completed with no typed fault. No task build or verification ran concurrently
with this child. Builds are excluded from its kernel resource measurement.
Engine build ID: `b204d8ea8291ab7e6323859e69feb3d6d315d2185816c6ba825f0f694529406d`.
Test executable SHA-256:
`40a6ee80f81371e3b3c7e0051ad8c5cb728ba859f636e25a1e48df3a2578f73c`.
Raw output, kernel child resources and the measurement script are retained under
`generated/final-engine-verification/d1b3-history/`.

| Public-path K=480 probe | Result |
| --- | ---: |
| Retained views, all four viewers | 24,972 |
| Logical stack entries across those views | 6,000,960 |
| Distinct retained stack nodes, all viewers | 3,840 (8K) |
| Conservative final history charge | 101,253,152 bytes |
| Cascade setup, public execution and allocation-count check | 7,675,645 µs |
| Whole child elapsed time | 7.682438 s |
| Kernel whole-child peak RSS | 64,757,760 bytes |
| `/proc/self/status` baseline / final VmRSS | 5,750,784 / 65,155,072 bytes |
| Observed VmRSS growth | 59,404,288 bytes |

The independent review's pre-fix K=480 run retained approximately six million
wide stack rows and grew by **about 3.9 GB**. The corrected probe retains the
same order of logical history with **about 59 MB growth**. The prior growth is
the reviewer's measurement, not a new run of the defective executable; peak
kernel RSS and sampled VmRSS are separate Linux measurements. This replaces
the incomplete exploratory public-path run with a completed measured workload.
It is diagnostic retention evidence, not a quiet-machine P7 throughput claim.

### Remaining retention and the fault boundary

Sharing does not bound the length of a game. Per-view metadata and changed
object/player/commander rows, changed stack suffixes, viewer-handle maps,
accepted-input journals and detailed event/batch evidence can keep accumulating.
Object collection slots are shared when unchanged, but changed collections still
retain their full slot lists. Full public history and snapshot **output bytes**
can still be quadratic in maximum stack depth; streaming avoids retaining those
bytes internally, but exporting to a native `String` remains caller-owned memory.
Whole-state forks, event preparation, arbitrary exact quantities and native
query/export buffers remain separate allocation costs. The old synthetic
combined-ceiling probe above bypasses ordinary history generation and is not a
4 GiB hosting guarantee.

The ignored `history_draw_release_probe` also runs a complete admitted four-seat
foundations pod using only keeps, passes, empty combat declarations and required
discards. It finishes naturally on turn **371**, winner Player 1, with **no
fault**: 32,949 rules steps, 38,122 evidence records, 112,344 retained views and a
final charge of **495,502,436 bytes**. This longer ordinary game is separate from
the injected K=480 cascade and keeps the historical draw-failure outcome intact.

The historical d652368 implementation used a catalog-independent **512 MiB history charge** to stop accumulated retention
with `RuleFault::HistoryCapacityExceeded { capacity, requested }`. It is a
conservative deterministic policy measure, not measured RSS: four times encoded
payload plus fixed row/node/view and collection-slot allowances. Shared object rows, player/commander collections and stack prefixes are charged
once; per-view scalar metadata and changing collection slots are charged every
time. Potential detailed evidence (including the duplicate Event array) is
charged in **every trace mode**, even Off, so tracing cannot change the resource
boundary. Accepted inputs, external-input slots and a 2,048-byte allowance per
committed rules step cover journals, history chunk/handle overhead and cumulative
small allocations. Charge arithmetic saturates toward exhaustion. No allocator
capacity, pointer address, canonical memo warmth or host clock selects the fault. Per-view fixed overhead
uses the compiled inline view size plus 512 bytes for entry/Arc/chunk slots,
four segment descriptors and allocator headers; canonical metadata payload is
charged separately. This avoids treating a 256-byte inline view as a separate
2 KiB allocation before its payload charge.

The guard checks evidence before retention and each staged public rules step
before and after mutation/visibility refresh. Exhaustion retains the entire
pre-step state plus the typed fault/frame; staged event, life, stack, journal and
visibility mutations do not leak. A submitted input remains accepted and pending
until `advance` processes it, so replay reaches the same attempted step and
fault. An individual accepted input/refresh can temporarily exceed the charge;
only one input can be pending. Hosted command/IPC limits bound that increment.
The 512 MiB policy leaves substantial headroom below the worker's 4 GiB limit
for live tables, staging and serialization; it does not equate an estimate to an
absolute RSS proof for arbitrary trusted native allocations or future IR fields.
A fault has no winner/draw. Atomic
rollback and identical fault boundaries are tested in all three trace modes;
normal snapshot/restore and accepted-input replay tests remain in force.

### Verification scope

The final release build passed SDK generation, **47 Node SDK tests** and strict
TypeScript. The attachments and wipes `match-batch` suites each passed **18/18**
games, including independent replay, with **zero faults or truncations**. Both
reports bind release engine build `b204d8ea8291ab7e6323859e69feb3d6d315d2185816c6ba825f0f694529406d`.
The K=480 and long draw-only release probes each passed. Exact raw logs,
per-game reports, baseline-byte proof and the workspace check log are retained
in `generated/final-engine-verification/d1b3-history/`. The ten optional other
profile batches were not rerun; catalog admission and sizing still run across
all twelve profiles in the workspace gate.

`npm run check:engine` passed formatting, Clippy with warnings denied,
**895 Rust tests** (16 intentionally ignored probes) and **56 Node checks**,
with **zero failures**. The final `summary.json` records these counts alongside
the SDK, release probes, historical-byte proof and batch results. Earlier
failed/interrupted attempts remain separately qualified in the same evidence
directory; they are not counted as passed phases.

## D1b-3 history2 accounting correction

The preceding public-history measurements describe d652368. Review found that
its fourfold payload multiplier, whole-player-array charging and 2 KiB step/
evidence allowances exhausted 512 MiB during ordinary land play at turn 238.
The pass-only game reached turn 371 but used 92.3% of that policy budget. Those
charges were poorly calibrated to actual retention.

History2 shares each `VisiblePlayer` row independently, including its canonical
memo, after complete typed equality with the same viewer's immediate predecessor.
Changing one player's counts, life or mana retains only that player's new row.
Unchanged other players, including nested exile lists, are not retained again
or recharged. Graveyard objects already use individual shared object rows. New
player and object collection slots use fresh narrow allocations; unchanged whole
collections still reuse their allocation. Stack prefix sharing is unchanged.
No interning crosses viewers or skips the typed comparison. Public JSON fields,
array order and secrecy boundaries are unchanged.

The policy now counts the canonical encoded length of each newly retained row,
stack node, commander collection and view metadata, plus **256 fixed bytes per
view** and **8 fixed bytes per newly retained player/object reference slot**.
These are policy constants on every target, including wasm32 and x86_64; no
`size_of`, allocator capacity or pointer width enters the charge. Pointer identity
only recognizes sharing already established by typed equality. Decoding history
rebuilds that same sharing through `push`/`reuse_previous`, so restoring cold
memos reproduces the same charge. Saturating arithmetic exhausts the policy;
the 768 MiB boundary lies below either target's integer limit.

Accepted and external inputs use their exact encoded lengths. There is no
per-step retention allowance. Evidence reserves the exact encoded `TraceRecord`
and, for event records only, Detailed's duplicate `Event`. That same reservation
runs in Off, Compact and Detailed so tracing cannot change a rules result or
capacity boundary. The policy unit is encoded retained content, not an allocator
or RSS prediction; typed storage, canonical memos, staging and live tables need
separate measured headroom.

With the fixed card/live-token/retained-identity ceilings, per-view metadata and
collection slots are bounded. Unchanged rows are retained once; new content,
views, inputs and evidence accumulate linearly with game length. A changed
player row still owns its complete exile list; unchanged subfields inside that
changed row are not independently interned. Changed commander rows are likewise
stored as complete new values, and a changed object collection retains its
complete reference list. This does not provide persistent sharing of arbitrary growing subfields or make native export
strings linear in stack depth. Those limits are distinct from repeatedly storing
and charging every player's unchanged rows.

### Atomic terminal-step boundary

The history guard remains before the staged step and after visibility refresh.
**A step that would finish with a win or draw can exceed the budget during its
final visibility refresh. The entire step, including that provisional rules
outcome, then rolls back to the pre-step state with a typed history fault.**
The host must expose that resource fault, with no winner/draw; it must not report
the provisional result. Already committed terminal states are returned before
another step is attempted. This limitation applies to any finite atomic
retention cap. The ordinary decking regressions ensure these particular natural
games have ample headroom, rather than exempting terminal steps from the guard.

### Current release measurements

Raw logs and per-child `wait4` resource records are kept
under `generated/final-engine-verification/d1b3-history2/` (ignored artifacts).

The selected history budget is **768 MiB (805,306,368 policy bytes)**. The
512 MiB candidate completed the measured natural games but failed the unchanged
under-25% headroom assertion for foundations, combat-keywords and data-admission
land play (about 145–148 MB). Increasing the measured budget to 768 MiB leaves
these games at approximately 18.4%, without changing the guard, excluding evidence,
or exempting terminal steps. The old failed headroom assertions and interrupted
calibration jobs are retained separately and are not final passes.

Pinned Rust 1.99.0, release overflow checks enabled, tracing Off. Current engine
build: `b819f4b073e5de926376cd7283f6e14b11352070fcee834d71fe45b097a07d76`.
Probe executable SHA-256:
`eba883b986f9985b70c9b01beb35d1df008765617870d4887069f034a09f7869`.
The cascade and budget probes ran before the other final verification processes.
Natural-game probes ran four independent child processes at a time alongside
verification; elapsed times are diagnostic only. Kernel peak RSS comes from each
child's own `wait4` resource record, excluding compilation. Sampled VmRSS is a
separate Linux measurement.

| Current release workload | Final history charge (bytes) | Kernel child peak RSS (bytes) |
| --- | ---: | ---: |
| Foundations, pass-only to turn 371, Player 1 wins | 81,658,151 | 101,720,064 |
| Foundations, play a land whenever legal, turn 371, Player 1 wins | 148,016,519 | 103,153,664 |
| Hosted-attachments K=480 Blood Artist cascade | 16,223,891 | 61,063,168 |
| Warmed metadata/view retention at the 768 MiB boundary | 805,306,466 | 1,372,422,144 |

The K=480 cascade retains the same **24,972 views, 6,000,960 logical stack
entries and 3,840 distinct stack nodes**. Sampled VmRSS rises from 5,636,096 to
62,447,616 bytes, a **56,811,520-byte** increase. It completes without fault.
The boundary probe incrementally retains and warms **1,745,745** small views,
then proves the typed history-capacity fault at the actual selected limit. Small
views emphasize per-view allocation/memo overhead relative to payload. This is
synthetic retention, not an accepted-input reachable match or a snapshot export.
Its 1.37 GB peak is approximately **32% of the worker's 4 GiB `MEMORY_MAX`**, leaving
about 2.9 GB for live tables, staging and other worker costs. The budget is a
measured retention policy, not an absolute RSS guarantee for arbitrary native
export buffers, trusted injections or future IR layouts.

Both pass-only and land-play policies passed on **all twelve registered profiles**:
**24/24** natural games, turn 371, Player 1 wins, zero typed faults. Each uses
homogeneous copies of that profile's first admitted default deck, seed `[7; 32]`,
starting seat 2, keeps, empty combat and required discards. Land play adds each
legal land play while otherwise passing. The largest final charge is the
foundations land-play result above, **148,016,519 bytes (18.38% of budget)**.
Across profiles, pass-only charges range from 81,656,975 to 81,675,721 bytes;
land-play charges range from 124,020,649 to 148,016,519 bytes. The largest kernel
peak RSS among all 24 natural-game children is **104,853,504 bytes**.
The cheaper ordinary debug regression checks near-linear incremental charge over
16/32/48 turns and exact charge plus state-digest equality after snapshot restore.
The serialization-domain regression separately proves a large unchanged other
player's exile row is shared and charged once, including after incremental decode.

The current release's `baseline-byte-proof.json` verifies **173/173** complete
canonical snapshot hashes against their execution reports, then removes only
`retained_auxiliary_charge` and reproduces every frozen D1b-2 state hash. Only
after that proof was the intermediate `d1b3-history-scenario-digests.json` fixture
updated. The original `d1b2-scenario-digests.json` remains unchanged. Public
history fields, order, card facts and rules outcomes were not rebaselined.

All twelve final release `match-batch` suites passed **18/18** games each:
**216/216 complete games and independent replays**, with **zero faults and zero
truncations**. Every batch manifest binds the current engine build above. Exact
per-game evidence and reports are under `d1b3-history2/batches/` in the retained
output directory.

The final `npm run check:engine` passed formatting, Clippy with warnings denied,
**898 Rust tests** and **56 Node checks**, with **zero failures** and **18
intentionally ignored Rust probes**. The separately executed release probes
passed **26/26**: 24 natural games, K=480 and the warmed history boundary.
The final release `mtg-server` build and SDK verification also passed: **47 SDK
Node tests**, strict TypeScript and all **32 response variants**. SDK verification
logs/manifests are retained under `d1b3-history2/sdk/`; the SDK's own `generated/`
directory was removed afterward as requested. The final `summary.json` binds
these counts, probes, original-byte proof and batch results. Earlier calibration
failures/interrupted attempts remain separate and are excluded from final passes.
