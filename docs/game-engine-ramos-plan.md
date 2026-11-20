# Ramos deck engine implementation plan

Make unchanged **Jo's Cascade** executable through complete source-qualified
mechanic families. Exact-deck and consumer admission remain separate.

## Target and authority

Owner jcaldwell; deck11, paper, revision17/edit0: **100 cards,37 lands,85 definitions**, SHA-256
`f9eb5bc8ea479e036fc24c3fba27383ee2f030191d1a689de80bdb30e2278ee1`.
Later revisions require a new target.

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

`02b35e35` compiles **70/85 original rows**, plus three source-identical authorized reparses. [Evidence](game-engine-evidence.md#ramos-land-and-library-choice-integration)
retains exact producer boundaries; complete-deck admission remains open.

### M4 Complete the cascade package and casting history

Averna and Call Forth compile through source-identical authorized reparses. Yidris awaits complete rules support.

### M5 Add color and mana engines

Prismatic Omen and Lantern have live mana witnesses. Complete Leyline, Selvala and Omnath.

### M6 Complete card advantage and flexible choices

Whirlwind, Rhystic, Abzan and Urban have focused witnesses. Complete Quandrix
choose-two/graveyard targets and Conflux colored search.

### M7 Complete protection and board interaction

Anguished Unmaking, Void Rend, Ruinous Ultimatum and Avacyn have focused witnesses.
Death-trigger destruction, Exterminatus, Farewell and Akroma's Will have paid witnesses.

### M8 Add spell and permanent copies

Double Major compiles from an authorized reparse. Complete Technique/demonstrate and Protege through generic
copyable characteristics, independent targets/choices, ownership/control and legendary
exceptions. Revise copy/token/trigger capacity.

### M9 Add linked permissions, bestow and Sagas

Complete Mosswort's private linked hideaway permission, Chromanticore bestow/type/
attachment transitions and Thunder's lore/chapters/final sacrifice.

### M10 Complete phasing and remaining effects

Teferi and Archangel have paid witnesses. Complete Coalition's repeated modes and Progenitus replacement.

### M11 Qualify the exact deck and host it

Qualify target definitions, provenance, capabilities and Commander/pod admission.
Run seeded four-seat completion and replay with public policies, rotating and
repeating Ramos among varied opponents. Report outcomes, faults and truncation.
Qualify host/SDK privacy, retries, reconnect, recovery and four-client completion.
Search remains separate.

## Execution and reporting

M0→M1→M3→M4 is primary; M2 is independent. Partial semantics never qualify.
