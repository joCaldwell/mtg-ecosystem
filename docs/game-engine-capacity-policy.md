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
triggers and subsequent terminal token waves. The regression
`generic_simultaneous_trigger_bound_fits_shared_stack_policy` recomputes the same
rule for every catalog, independent of names, lineage and default deck allocation.
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
it does not optimize that copying or bound the duration of every game, arbitrary
exact-integer arithmetic, retained accepted-input journals, or every future
search workload. Any broader P7 performance claim requires fresh measurements
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

The passing private 96-death regression excludes visibility-history capture.
An exploratory run through full public `advance`/`respond` history capture grew
past 11 GiB observed RSS and was stopped before completion; it is **not a passed
public-path measurement**. The private rule-step and ceiling measurements above
do not hide or resolve that limitation. Existing natural-game tests retain
accepted-input, public-history and restore/replay coverage at their own sizes.
