# Event-time appearance for filtered damage triggers

A creature-filter damage trigger must match the exact battlefield permanent at
the actual event, rather than an assignment-time source that has since left.
The IR30 predicate and its card programs are unchanged; this is a rules correction
with no Card IR, capability-minimum, catalog, profile or public DTO change.

The source-backed regression uses complete Research Thief and Alpha Myr with
real Furnace of Rath and Samite Healer effects. Alpha's owner concedes while
replacement ordering is pending after damage assignment. Its already assigned
positive damage still commits to the surviving player under the existing
qualified CR 510.2 contract. Research remains on the battlefield, but Alpha no
longer exists there, so it cannot satisfy “an artifact creature you control.”
The retained red test executed once and failed against the previous matcher:
one Research trigger was collected where zero was required.

`TriggerEventWitness::DamageDealt` now has an optional `match_source` appearance.
Actual assigned damage, source incarnation, controller and frozen damage keywords
remain intact. Ordinary collection captures the current exact battlefield
appearance separately for both transactional and legacy damage leaves. Filtered
abilities retain that appearance; exact self-source abilities omit it and keep
their old witness bytes. Missing, nonbattlefield or different-incarnation sources
cannot satisfy the creature filter.

The collected appearance freezes for the trigger's lifetime. Later source,
damager or owner departure does not undo a matched trigger. Restore validates
its exact object/physical owner, immutable characteristics including Snow,
controller domains, zone and complete predicate separately from assigned damage
and observer LKI. It does not require the damager to remain present or under the
same current controller after collection. Accepted-input replay rejects coherent
appearance lies beyond those structural guards. Existing serialized-payload
charging automatically includes the extra fact; no resource ceilings change.

`match_source` is private evidence, not a player-facing DTO. Snapshot19 retains
its exact engine build fence; old filtered witnesses without the new evidence
cannot bypass that fence or mandatory validation. Historical source records,
compiled catalog digests, expected IR and evidence captures remain unchanged and
retain their original build qualifications.

Rules authority remains the pinned 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 109.2 defines the battlefield permanent domain, distinct from 109.2c sources;
603.10 uses ordinary event-time appearances; 510.2/120.2/120.7/506.4 underlie the
separate retained assigned-damage interpretation; 113.7a/800.4 preserve supported
collected invocations after departures. Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

Focused validation passed formatting, dependency/structure guards, workspace
all-target compilation and 52 selected core tests: four new appearance cases,
32 existing self/opponent/filtered damage cases, six Ohran cases, six exact
quantity-draw cases and four v2 admission cases. Tests execute actual source-backed
replacement-ordering/concession inputs, positive retained damage, absent-source
rejection, observer departure, postcollection continuation and restore/replay.
Six malformed appearance variants reject structurally; a coherent derived-power
lie passes local shape validation but rejects during accepted-input replay.

Logs are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/` as
`damage-appearance-red-01.log` (one expected failing regression) and
`damage-appearance-green-01.log` (passed). Historical predicate probes now pass
explicit appearance facts. The older coherent-damager forgery changes both
prepared and matching identities, preserving its local-pass/replay-reject claim.
Compiler/contracts tests, executable Card IR schema/catalog checks, SDK, full
runtime suites, new complete matches, release/profile matrices and retention
rebuilds were not repeated for this private core correction. The genuine legacy
appearance path was inspected in the common collector; no separate exact-source
filtered noncombat positive was added. Previous evidence keeps its own source
identity; this is focused validation rather than combined acceptance.
