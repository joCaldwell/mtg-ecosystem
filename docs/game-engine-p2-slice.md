# Budgeted execution and private observations

Scope frozen October 13 2026. This P2 slice hardens the existing reviewed
three-card, four-seat scenario. It adds resumable execution/query work,
viewer-specific observations and handles, the permitted SearchContext boundary,
recovery at every supported execution boundary, and retained dependency closure.
It does not add playable matches or new card mechanics.
Status: **implemented and verified**; checks and retained identities are in
[implementation progress](game-engine-progress.md).

## Execution and query budgets

An execution unit runs one atomic rules step of the admitted small interpreter.
Budget zero suspends runnable work without mutation; minimum quantum is one.
Reaching a decision returns that decision even when the last unit was consumed.
Budgets count deterministic work, not wall-clock time. The fixed scenario bounds
rules/object work within a step. Clone, snapshot, and history copying still grow
with run length; this is not a latency or process-memory guarantee. Broader
profiles must bound or subdivide larger steps and measure representation costs.

Queries execute one player/object/history row or one validation stage per unit.
An incomplete query returns an opaque native work object, never an empty complete
domain. Resuming work checks both executor and exact state identity. A state
transition, fork, or restore invalidates live work and receipts. Query work does
not allocate semantic IDs, advance rules, change histories, or consume RNG.

Saved query work is a trusted recovery sidecar. Recovery verifies its snapshot
identity and recomputes the saved prefix before rebinding it to the restored
executor. Clients never receive private scratch or validation receipts.

## Observations and SearchContext

Use distinct serializable player-facing types, never a redacted State. Public
zones expose known faces; each player sees their own hand, while other hands and
all library contents expose counts only. Zone policy follows CR 400.2, 401.2,
and 402.3 in the pinned September 25 rules. Libraries/command/exile zone types
may be represented, but their gameplay remains outside the admitted scenario.

Opaque handles are scoped to a viewer and allocated only for objects that viewer
can currently see. They expire on zone changes or loss of visibility. Allocation
occurs during deterministic game transitions, never during queries. Previously
revealed information stays in permitted history without granting current access
to a hidden card. Player responses resolve only that viewer's live handles.
Decision tokens are viewer-specific and never contain an authoritative hash.

SearchContext is supported only at ordinary priority boundaries with no accepted
input awaiting consumption. It contains the root player's observation/history,
open deck compositions, public definition/profile identities, and a separate
caller-supplied sampling seed. It has no authoritative snapshot, physical or
incarnation IDs, private frame, hidden library order, game seed, or state hash.
Equal permitted histories must yield equal contexts despite different hidden
authoritative state. The sampler/reconstruction builder remains P7 work.

## Recovery and evidence

Snapshot version 2 records the deterministic rule-step boundary and persisted
visibility knowledge. Recovery replays the accepted journal only to that exact
boundary, including suspended payment/resolution frames and pending inputs.
Rehashed impossible state or altered private bookkeeping rejects.

Keep the P1 input/event fixture unchanged. The additional boundary/knowledge
state changes the version-2 semantic hash; review and record that new hash rather
than treating it as interchangeable with P1. Test budget-partition equivalence,
restore at every boundary, invalid/stale/foreign query work and receipts,
query-order purity, hidden-zone redaction, handle expiry, context equivalence,
and tampered recovery sidecars. No random operation is introduced.

Retain the native source closure, Cargo.lock, toolchain/build settings, executable,
replay bundle, and vendored dependency checksums. Prove a clean Cargo dependency
store can rebuild from retained files without registry/network access, then verify
the bundle with that rebuilt executable. Measure the tiny scenario's branching
and retained memory without claiming representative Commander performance.

The subsequent [departure slice](game-engine-departure-slice.md) implements
departure history, Commander identity/history tables, and boundary-ordered
concessions. It advances snapshots to version 3 and observations/contexts to
version 2; the version-2 snapshot/hash above remains historical evidence.
The next [event slice](game-engine-event-slice.md) adds atomic compositions and
three nonsemantic trace modes, advancing snapshots/bundles to version 4.
The subsequent [instant-response slice](game-engine-stack-slice.md) exercises
multiple entries and source departure, with snapshot/bundle v5 and projection
v3. Broader recovery and interaction invariants remain work. This delivery is
not full P2 signoff. No user decision blocks it.
