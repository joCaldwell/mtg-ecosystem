# Engine structural capacity policy

These are executor resource ceilings, not claims that every admitted game
terminates within them. A ceiling never determines a Magic winner.
`engine/capacities.rs` is the runtime authority; catalog names, schema versions,
card identities and publication order never select a capacity, and checked
identity/cursor arithmetic remains an independent invariant.

| Resource | Limit | Accounting |
| --- | ---: | --- |
| Owned physical cards per player | 100 | Snapshot/wipe validation; admission enforces deck size |
| Live allocated tokens | 256 | Existing token objects |
| Retained token identities | 65,536 | Includes ceased tokens, whose history is retained |
| Damage occurrences per transaction | 2,048 | Entire simultaneous prepared damage group |
| Stack plus pending triggers | 262,144 | Combined entries, before any new entry |
| Event-group nesting depth | 4 | Checked before recursive collection |
| Event-group occurrences | 1,024 | Total leaves across ordered and simultaneous groups |
| Temporary effects | 256 | Current retained effects, before insertion |
| Retained history charge | 768 MiB | Newly retained encoded content, fixed view/reference overhead, inputs and potential detailed evidence |

## Generic sizing rule

A four-seat pod has 400 physical cards and at most 256 live tokens, so at most 656
creatures can die in one simultaneous group. The trigger wave is bounded by 656 ×
four seats × the catalog's triggered abilities on nonbasic definitions (singleton
admission allows one physical source per seat; basic lands and generated creature
tokens have no triggers). `match_profile_validation_tests` recomputes this, and the
cast-trigger fanout below, for every profile catalog and fails unless each fits
below half the stack ceiling. Profile flags never bypass runtime checks; a catalog
with more trigger sources must revisit the shared ceiling.

Live-token and temporary-effect ceilings are reachable in legal games (repeated
token creation and sacrifice, repeated pumps); they are deliberate boundaries with
typed atomic faults, not complete legal-game coverage. Damage and event-member
ceilings exceed the 656-creature group plus four players; group depth is structural
IR nesting.

## Cast-trigger fanout preflight

Cast triggers and battlefield observers share the stack-plus-pending ceiling; no
family/profile flag bypasses `check_stack_capacity`. Each cast visits each observer
and its casting source once without deduplication, so fanout is at most
`400 * max(validated definition.triggered.len())` with checked arithmetic.
Copied/granted token abilities must extend this proof before admission. Before
publishing a batch, the engine preflights its entire emission against retained
stack/pending count, TriggerId arithmetic and StackId headroom; overflow rolls back
payment, events, IDs and all siblings atomically. Cascade has no executable R5 form
([mechanics](game-engine-mechanics.md)); a future lowering must bound its progress,
exile and permutation lists by the 100-card library and charge retained capacity.

## Fault semantics

Exhaustion produces a tagged, serializable `RuleFault` with capacity and requested
usage (`TokenHistoryCapacityExceeded` for retained tokens). Mutation runs on staged
state, so an overflowing step retains only the fault and fault frame. Event groups
check totals before application. Snapshot validation checks bounds and fault
metadata; restoration still proves accepted-input replay. A fault is never a rules
loss, draw or silent truncation.

Whole-state forking per step is a known performance limitation; mutable staging,
exact quantities, export strings, queries and snapshot proof can cost more than
retained charges. Any performance claim needs fresh measurement
([evidence](game-engine-evidence.md#search-hosting-and-performance)).

## Retained history accounting

History2 shares each viewer's `VisiblePlayer` row after typed equality with that
viewer's predecessor, so unchanged players, objects and stack prefixes are not
retained or charged again; nothing is interned across viewers, and public JSON and
secrecy are unchanged. The charge is the canonical encoded length of each newly
retained row, stack node, commander collection and view metadata, plus 256 fixed
bytes per view and 8 per new reference slot, identical on every target (allocator
and pointer sizes never enter). Decoding rebuilds the same sharing and charge.
Accepted inputs and evidence (`TraceRecord`, plus Detailed's duplicate `Event`)
reserve their exact encoded lengths in every trace mode, so tracing cannot change a
rules result or boundary. The unit is encoded content, not RSS; changed rows are
stored whole, so content grows linearly with game length.

### Atomic terminal-step boundary

The history guard runs before the staged step and after visibility refresh.
**A step that would finish with a win or draw can exceed the budget during its
final visibility refresh. The entire step, including that provisional outcome,
then rolls back to the pre-step state with a typed history fault.** The host must
expose that resource fault with no winner/draw, never the provisional result.
Already committed terminal states are returned before another step is attempted.
Decking regressions keep natural games well within headroom rather than exempting
terminal steps from the guard.
