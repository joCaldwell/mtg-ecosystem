# Ramos deck engine implementation plan

Make unchanged **Jo's Cascade** executable through complete source-qualified
mechanic families. Exact-deck and consumer admission remain separate.

## Target and authority

Owner jcaldwell; deck11, paper, revision17/edit0: **100 cards,37 lands,85 definitions**, SHA-256
`f9eb5bc8ea479e036fc24c3fba27383ee2f030191d1a689de80bdb30e2278ee1`.
Preserve quantities and commander; later revisions require a new target.

The [export](../packages/game-engine/tests/fixtures/ramos/revision-17.txt),
[identity manifest](../packages/game-engine/tests/fixtures/ramos/revision-17.manifest.json),
[frozen sources](../packages/game-engine/tests/fixtures/ramos/provenance.json) and
[compiler baseline](../packages/game-engine/tests/fixtures/ramos/baseline-46ef08fa.json)
are authoritative. Archive SHA-256:
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`;
pinned rules SHA-256 is
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
App/frozen-text comparison remains OAuth-blocked.

Follow [development](game-engine-development.md); names identify witnesses, never mechanics.

## Qualified checkpoints and limits

[Evidence](game-engine-evidence.md) owns qualified M0–M4 producers, limits and receipts.
Exact-deck admission remains open. [Historical plan](https://github.com/joCaldwell/mtg-ecosystem/blob/ad428b23ee5b3f5750f8c79b4a7823c8d93d5b7f/docs/game-engine-ramos-plan.md).

## Ordered remaining milestones

### M2 Complete land and ramp infrastructure

Land/search/fetch/cycling and mana families have focused witnesses. Current source
`ec659db7` compiles **55/85 definitions;30 Unsupported**. [Evidence](game-engine-evidence.md#ramos-land-and-library-choice-integration)
retains exact producer boundaries; complete-deck admission remains open.

### M4 Complete the cascade package and casting history

Apex/Wanderer/Imoti/Nexus/Sorcerer/Ethersworn are qualified above. Next:
Yidris needs complete AST/binding before grants; Enigma successor tuck and Vega
cast-origin draw have focused witnesses. Remaining turn/zone scopes follow. Add Averna's action, Call Forth's other-spell mana
values.

### M5 Add color and mana engines

Complete Leyline opening action/layers/hybrid costs, Prismatic Omen, Lantern grants,
Selvala allocation/draw and Omnath Phyrexian payment,
unspent-mana replacement and private choices.

### M6 Complete card advantage and flexible choices

Complete Aragorn, Whirlwind, Temur Ascendancy, Rhystic payments, distributed Abzan
counters, Quandrix choose-two/graveyard targets, Conflux colored
search. Urban Evolution has a focused land-permission witness.

### M7 Complete protection and board interaction

Anguished Unmaking, Void Rend, Ruinous Ultimatum and Avacyn have focused witnesses.
Complete Child, Exterminatus, Farewell and Akroma's Will.

### M8 Add spell and permanent copies

Complete Double Major, Creative Technique/demonstrate and Protege through generic
copyable characteristics, independent targets/choices, ownership/control and legendary
exceptions. Revise copy/token/trigger capacity.

### M9 Add linked permissions, bestow and Sagas

Complete Mosswort's private linked hideaway permission, Chromanticore bestow/type/
attachment transitions and Thunder's lore/chapters/final sacrifice.

### M10 Complete phasing and remaining effects

Complete Teferi's Protection, Coalition's repeated modes, Progenitus replacement,
Archangel hand casting, Zacama cast-sensitive trigger/activations, Breya's tokens/
atomic sacrifice and Door's player loss.

### M11 Qualify the exact deck and host it

Qualify every complete target definition, capability/provenance and Commander/pod
admission. Use public-observation policies and fixed-seed four-player completion/replay
with Ramos in each seat, repeated Ramos and varied opponents. Distinguish outcomes,
faults and truncation. Then qualify hosting/SDK privacy, retries, reconnect, recovery
and four-client completion. Search remains separate.

## Execution and reporting

M0→M1→M3→M4 is primary; M2 is independent. Partial semantics never qualify.
