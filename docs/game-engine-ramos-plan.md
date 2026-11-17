# Ramos deck engine implementation plan

Make **Jo's Cascade**, with Ramos, Dragon Engine as commander, executable end to
end using its unchanged saved list. Deliver complete mechanic families in bounded,
source-qualified increments. This plan does not admit the exact deck or consumers.

## Target and authority

Owner jcaldwell; deck 11; paper lifecycle; revision 17, edit version 0. The unchanged
export has **100 cards, 37 lands, 85 definitions**, SHA-256
`f9eb5bc8ea479e036fc24c3fba27383ee2f030191d1a689de80bdb30e2278ee1`.
Preserve quantities and commander designation. Later app edits require an explicit new target.

The [export](../packages/game-engine/tests/fixtures/ramos/revision-17.txt),
[identity manifest](../packages/game-engine/tests/fixtures/ramos/revision-17.manifest.json),
[frozen sources](../packages/game-engine/tests/fixtures/ramos/provenance.json) and
[compiler baseline](../packages/game-engine/tests/fixtures/ramos/baseline-46ef08fa.json)
are authoritative. Archive SHA-256 is
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`;
pinned rules SHA-256 is
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
The app/frozen Oracle-text comparison remains unverified because OAuth needs
reauthentication; no new revision was fetched.

Use the [development workflow](game-engine-development.md) and complete-source semantics; names identify witnesses, never runtime rules.

## Qualified checkpoints and limits

[Evidence](game-engine-evidence.md) owns exact producers, counts, bridges and
exclusions. M0 preserved all85 records and historical11/74/0 compiler diagnostics.
M1 covers Ramos/Scales/Corpsejack; entry/simultaneous/distributed placements remain
unavailable. M2 covers basic split/typed search, Cinder conditional entry and intrinsic
mana. M3 covers basic/instant cascade, kicker and paid prohibition. M4 covers
Apex/Wanderer/Imoti/Nexus/Sorcerer/Ethersworn. None admits the exact deck or consumers.
Receipts: `generated/ramos/*-integrated/integration.json`; research: Git `30ec86ee`.
The [prior plan](https://github.com/joCaldwell/mtg-ecosystem/blob/ad428b23ee5b3f5750f8c79b4a7823c8d93d5b7f/docs/game-engine-ramos-plan.md) retains historical boundaries and failures.

## Ordered remaining milestones

### M2 Complete land and ramp infrastructure

Complete shock entry, fetch life/tap/sacrifice costs, opponent production and triome cycling.
Combined source `1fe0dfa1` closes typed-search and conditional-entry review findings;
[evidence](game-engine-evidence.md#ramos-land-and-library-choice-integration) records qualification. Five frozen shocks have SOURCE_ONLY_UNQUALIFIED
entry transactions and test sources; native proof remains pending. Fetch costs,
triome cycling and opponent production remain open.

### M3 Build casting during resolution and basic cascade

Qualified above: owned free casts, optional additional costs, legal targets/current
prohibitions, nested completion and private-bottom recovery. Further sources require complete-source qualification.

### M4 Complete the cascade package and casting history

Apex/Wanderer/Imoti/Nexus/Sorcerer/Ethersworn are qualified above. Next:
Enigma's exact battlefield-to-own-graveyard successor and third-from-top library
movement/knowledge; Yidris needs complete AST/binding before grants. Remaining turn/zone scopes follow. Add Averna's action, Call Forth's other-spell mana
values and Vega's non-hand-cast draw.

### M5 Add color and mana engines

Complete Leyline opening action/layers/hybrid costs, Prismatic Omen, Lantern grants,
Faeburrow dynamic quantities, Selvala allocation/draw and Omnath Phyrexian payment,
unspent-mana replacement and private choices.

### M6 Complete card advantage and flexible choices

Complete Aragorn, Whirlwind, Temur Ascendancy, Rhystic payments, distributed Abzan
counters, Esper discard, Quandrix choose-two/graveyard targets, Conflux colored
search and Urban Evolution land permissions.

### M7 Complete protection and board interaction

Complete Anguished Unmaking, Void Rend, Ruinous Ultimatum, Child, Avacyn, Escort,
Exterminatus, Farewell and Akroma's Will.

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

Every target definition must compile completely and pass capability/provenance and
Commander/pod admission. Add useful public-observation policies without hidden-state
access or name dispatch. Run fixed-seed four-player completion/replay with Ramos in
every seat, multiple Ramos seats and varied reviewed opponents; distinguish natural
outcomes, faults and truncations. Then adapt Rust hosting/SDK decisions and verify
privacy, retries, reconnect, recovery and four-client completion. Search is separate.

## Execution and reporting

Primary dependency path is M0→M1→M3→M4; M2 can advance independently. M5–M10 reuse
those foundations. Call Forth, Selvala, Averna, Omnath, Yidris, Creative Technique
and Double Major still need complete parser/archive work under that package manual.
Exact-target admission requires complete semantics.
