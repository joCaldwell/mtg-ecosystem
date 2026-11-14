# Engine structural capacity policy

D1b-3 corrects the impractical D1b-2 ceilings. These are executor resource
ceilings, not claims that every admitted game terminates within them. A ceiling
never determines a Magic winner. The historical limits and measurements remain
qualified by their original release in [evidence](game-engine-evidence.md).

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
The historical sizing catalog had 33 triggered abilities (132 source instances
per pod, 656 × 132 = 86,592 simultaneous triggers). Current build-time checks
recompute the bound from every selected catalog; these historical counts are not
current membership or a bypass for larger future catalogs.

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
The previous 1,024 ceiling excluded the [sacrifice pool](game-engine-mechanics.md#movement-sacrifice-and-board-wide-effects)'s
4,898-token lifetime bound; the new shared ceiling also exceeds the largest recorded finite-pool
[lifetime bound](game-engine-mechanics.md#movement-sacrifice-and-board-wide-effects) (33,997). Those historical bounds are sizing evidence, not
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

Capacity constants and build-time wave checks are runtime authority. Mutable game
staging, simultaneous-event preparation, exact quantities, canonical export strings,
query work and snapshot proof can cost substantially more than retained row charges.
Shared catalogs/history do not imply free forks or a universal memory bound.
Historical component timings are in [evidence](game-engine-evidence.md).

## Fault semantics and remaining limitation

Exhaustion produces a tagged, serializable `RuleFault` with capacity and
requested usage. Retained-token overflow has its own
`TokenHistoryCapacityExceeded` fault. Mutation runs on staged game state; an
overflowing rule step retains only the fault and fault frame, with no partial
game mutation. Event groups check their total before application and propagate
staged typed faults. Snapshot validation checks both bounds and fault metadata;
restoration still proves accepted-input replay. A fault is never a rules loss,
draw, or silent truncation.

Historical release probes and their exact measurements are linked from
[engine evidence](game-engine-evidence.md#search-hosting-and-performance).

The whole-state fork per step and per simultaneous occurrence remains a known
P7 performance limitation. This change makes structural exhaustion practical;
it does not optimize that copying or bound arbitrary exact-integer arithmetic
or every future search workload. The history policy below now bounds retained
accepted-input journals and accumulated public/evidence histories. Any broader P7 performance claim requires fresh measurements
of its actual build and workload. Historical finite-pool hand proofs no longer
set runtime limits or establish current admission guarantees.


## Retained history accounting

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


### D2c-a typed trigger evidence and retained-byte charge

D2c-a replaces movement/token optional fields and object-only trigger subjects
with typed movement/token/step witnesses. The retained trace payload therefore
changes even for old object triggers. Actual serialized bytes still determine
charge, equally across all trace modes; no capacity ceiling or guard changed.
The [original D2c-a report](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/game-engine-cards-as-data-d2c-a.md) records the source/rules/
representation review and the exact 173-snapshot comparison. Reversing only the
witness encoding and its doubled event-size charge delta reproduces all prior
D1b-3 hashes. Only then were 37 current state references regenerated. Original
D1b-3 bytes and failed full-gate output remain retained; historical D1b-2 fixtures
and the historical measurement evidence remain untouched.
