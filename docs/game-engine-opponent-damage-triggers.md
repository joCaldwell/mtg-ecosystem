# Source damage to an opponent

IR26 adds `SourceDamageTriggerV2`, `TriggerDamageKind::Any` and
`DamageTriggerRecipient::OpponentPlayer`. The existing combat/player predicate
still requires `DamageTriggerV1` at IR25. Either expanded dimension requires
IR26 independently; removing a capability label cannot make an old catalog
accept the new predicate. Historical catalogs and source fixtures retain their
original versions and bytes.

The complete new source witness is **Thieving Otter**, Oracle ID
`5dee663f-9e2e-4503-8086-fc51664fdf29`, retained in
`packages/game-engine/tests/fixtures/cards-data-d2ce/`. Its exact text is
“Whenever this creature deals damage to an opponent, draw a card.” Provenance
records archive line 14102 and source SHA-256
`92a218752ae56044c6e5e19a1b99213c5ddd69e379aa93eaa9be6da94434fc74`.
This is a newly supported complete source, superseding its unsupported
disposition in the historical D2c-e report without changing that report's scope.

The compiler admits the exact self-creature subject, omitted combat qualifier,
and unqualified `an-opponent` recipient. It preserves the mandatory controller
draw program. Explicit noncombat-only predicates, qualified recipients, filtered
sources, event quantities and optional programs remain unsupported. The parser's
`parser/triggers.ts` distinguishes omitted combat (either kind) from explicit
`false` (noncombat only); lowering must preserve that distinction.

Runtime matching consumes positive actual player damage, including transactional
combat/noncombat members and genuine legacy damage events. Prevention and life
changes cannot manufacture damage occurrences. Source identity is the exact
incarnation. Opponent relation uses the trigger source's controller at collection,
then retains that controller with the invocation; it does not use the damage
invocation's controller. Source departure or later control changes do not rebind
the collected trigger. Existing occurrence/member validation, atomicity and
accepted-input restoration guards remain in force.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 109.5 and 603.3a define the trigger controller; 120.2a-b distinguish damage
kinds; 120.7 identifies the source; 603.2c defines multiplicity; 603.10 checks
ordinary triggers after the event; 615.6 excludes prevented damage. Official
source: <https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

This increment adds no player request, effect instruction, profile, hosting or
search admission. Snapshot/bundle/public format versions do not change; exact
catalog and engine identities still bind recovery. Focused development validation
is separate from combined acceptance of the later IR25/IR26 development line.
