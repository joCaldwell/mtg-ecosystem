# Engine structural capacity policy

D1b-3 corrects the impractical D1b-2 ceilings. These are executor resource
ceilings, not claims that every admitted game terminates within them. A ceiling
never determines a Magic winner. The historical limits and measurements remain
qualified by their original release in the progress ledger and slice documents.

| Resource | Limit | Accounting |
| --- | ---: | --- |
| Owned physical cards per player | 100 | Snapshot/wipe validation bound; deck size is enforced separately by admission |
| Live allocated tokens | 256 | Existing token objects |
| Retained token identities | 1,024 | Includes ceased tokens, whose history is retained |
| Damage occurrences per transaction | 2,048 | Entire simultaneous prepared damage group |
| Stack plus pending triggers | 1,024 | Combined entries, before any new entry |
| Event-group nesting depth | 4 | Checked before recursive collection |
| Event-group occurrences | 1,024 | Total leaves across ordered and simultaneous groups |
| Temporary effects | 256 | Current retained effects, before insertion |

`engine/capacities.rs` is the runtime authority. Catalog names, schema versions,
card identities and historical publication order never select a capacity.
Checked identity/cursor arithmetic remains an independent invariant.

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
review's workload measurements, not measurements of this new probe. D1b-2's
65,536-token and 4,194,304-stack ceilings could exhaust practical memory or
execution time long before reporting a typed resource fault.

D1b-3 uses the reproducible ignored `tokens_tests::capacity_release_probe` with
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

Measured release results (microseconds unless marked otherwise):

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
The raw probe output and separate child-process `getrusage` measurement are
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

The whole-state fork per step and per simultaneous occurrence remains a known
P7 performance limitation. This change makes structural exhaustion practical;
it does not optimize that copying or bound the duration of every game, arbitrary
exact-integer arithmetic, retained accepted-input journals, or every future
search workload. Any broader P7 performance claim requires fresh measurements
of its actual build and workload. Historical finite-pool hand proofs no longer
set runtime limits or establish current admission guarantees.
