# Engine structural capacity policy

These are executor resource ceilings, not claims that every admitted game
terminates within them. A ceiling never determines a Magic winner. Historical
limits and measurements remain qualified by their original release in
[evidence](game-engine-evidence.md).

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

A four-seat pod has 400 physical cards and at most 256 live tokens, so at most
656 creatures can die in one simultaneous group. The simultaneous trigger wave
is bounded by 656 times four seats times the catalog's triggered abilities on
nonbasic definitions (singleton admission allows one physical source per seat;
basic lands and generated creature tokens have no triggers). A core test
(`match_profile_validation_tests`) recomputes this for every profile catalog and
fails unless the wave fits below half the shared stack ceiling. A regression checks the same rule
without names, lineage or default deck allocation. Profile flags never bypass
runtime capacity checks; a catalog with more trigger sources must revisit the
shared ceiling.

The live-token and temporary-effect ceilings remain reachable in legal admitted
games (repeated token creation and sacrifice, repeated pump activations). They
are deliberate resource boundaries with typed atomic faults, not claims of
complete legal-game coverage. Damage and event-member ceilings exceed the
656-creature group plus four players; group depth is structural IR nesting.

## Cascade preflight

Self-cast cascade triggers and battlefield observers share the stack-plus-pending
ceiling; no family/profile flag bypasses `check_stack_capacity`. Each cast visits
each observer and its casting source once without deduplication, so fanout is at
most `400 * max(validated definition.triggered.len())` with checked arithmetic.
Copied/granted token abilities must extend this proof before admission. Before
publishing a batch, the engine preflights its entire effective emission against
retained stack/pending count, TriggerId arithmetic and StackId headroom. Overflow
rolls back payment, events, IDs and all siblings atomically. One resolving owner
and one announcing child remain the execution shape. Cascade progress, exile and
permutation lists use the 100-card bound and charge retained capacity; random
bottom preserves the library prefix and never counts as a library shuffle. Exact
source qualification lives in
[evidence](game-engine-evidence.md#ramos-land-and-library-choice-integration).

## Fault semantics

Exhaustion produces a tagged, serializable `RuleFault` with capacity and
requested usage; retained-token overflow has `TokenHistoryCapacityExceeded`.
Mutation runs on staged state: an overflowing step retains only the fault and
fault frame, with no partial game mutation. Event groups check totals before
application. Snapshot validation checks bounds and fault metadata; restoration
still proves accepted-input replay. A fault is never a rules loss, draw or
silent truncation.

Whole-state forking per step remains a known performance limitation; any broader
performance claim requires fresh measurement. Mutable staging, exact quantities,
export strings, queries and snapshot proof can cost more than retained charges.
Release probes are linked from
[evidence](game-engine-evidence.md#search-hosting-and-performance).

## Retained history accounting

History2 shares each viewer's `VisiblePlayer` row after typed equality with that
viewer's predecessor, so unchanged players, objects and stack prefixes are not
retained or charged again. No interning crosses viewers; public JSON and secrecy
are unchanged. The charge is the canonical encoded length of each newly retained
row, stack node, commander collection and view metadata, plus 256 fixed bytes per
view and 8 per new reference slot. These policy constants are identical on every
target; allocator and pointer sizes never enter. Decoding rebuilds the same
sharing and charge. Accepted inputs and evidence (`TraceRecord`, plus Detailed's
duplicate `Event`) reserve their exact encoded lengths in every trace mode, so
tracing cannot change a rules result or boundary. The unit is encoded content,
not an RSS prediction. Changed rows are stored whole, so content grows linearly
with game length.

### Atomic terminal-step boundary

The history guard runs before the staged step and after visibility refresh.
**A step that would finish with a win or draw can exceed the budget during its
final visibility refresh. The entire step, including that provisional rules
outcome, then rolls back to the pre-step state with a typed history fault.**
The host must expose that resource fault, with no winner/draw; it must not report
the provisional result. Already committed terminal states are returned before
another step is attempted. Decking regressions keep natural games well within
headroom rather than exempting terminal steps from the guard.
