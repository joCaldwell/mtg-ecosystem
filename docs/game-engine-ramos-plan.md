# Ramos deck engine implementation plan

Make **Jo's Cascade**, with Ramos, Dragon Engine as commander, executable end to
end using its unchanged saved list. Deliver complete mechanic families in bounded,
source-qualified increments. This plan does not admit the exact deck or consumers.

## Target and authority

Owner jcaldwell; deck 11; paper lifecycle; revision 17, edit version 0. The unchanged
export has **100 cards, 37 lands, 85 definitions**, SHA-256
`f9eb5bc8ea479e036fc24c3fba27383ee2f030191d1a689de80bdb30e2278ee1`.
Preserve quantities and commander designation. The maybe/acquisition lists do not
change membership; later app edits require an explicit new target.

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

Follow the [development workflow](game-engine-development.md),
[mechanics](game-engine-mechanics.md), [catalogs](game-engine-catalogs.md) and
[capacity policy](game-engine-capacity-policy.md). Whole-source compilation or
explicit rejection is mandatory. Card names/IDs identify evidence, never runtime
rules or policies. Keep the standalone deck-builder independent.

## Qualified checkpoints and limits

- **M0:** main `bbdba9f5`, planning base `46ef08fa`;85 records verified.
  Historical audit11 lowered/validated,74 Unsupported,zero Invalid and diagnostics
  remain preserved.
- **M1:** Ramos/Scales/Corpsejack support cast-color counters, removal/mana/undo
  and atomic Add/Multiply ordering with recovery. Entry/simultaneous/distributed
  placement producers remain unavailable.
- **M2 partial:** Cultivate/Reach retain basic split search. Farseek/Lore/Claim
  support typed nonbasic search, destination entry and shuffle. Cinder Glade and
  the separate Prairie Stream witness share conditional entry and intrinsic mana
  selection with paid, privacy, undo and recovery tests. Other sources remain open.
- **M3:** source-qualified basic/instant cascade, owned casting, optional kicker
  and paid prohibition have private paid/recovery witnesses.
- **M4 partial:** Apex/Wanderer, Imoti, Nexus/Sorcerer and Ethersworn are
  source-qualified. [Evidence](game-engine-evidence.md) owns producers, bridges,
  counts and exclusions; acceptance never transfers.

Coordinator receipts remain in `generated/ramos/*-integrated/integration.json`.
Source boundaries, failures and the unrepeated 833-second Apex witness remain in receipts and the [immutable prior plan](https://github.com/joCaldwell/mtg-ecosystem/blob/ad428b23ee5b3f5750f8c79b4a7823c8d93d5b7f/docs/game-engine-ramos-plan.md).
Research remains in Git `30ec86ee`. Exact-deck and consumer admission remain open.

## Ordered remaining milestones

### M2 Complete land and ramp infrastructure

Complete typed multicolor lands, shock/conditional/tapped entry, fetch life/tap/
sacrifice costs, typed nonbasic searches, mana choices/opponent production and hand
cycling. Cover Farseek, Nature's Lore, Skyshroud Claim and all triome modes; Forest
need not mean basic. Preserve reveal, optional quantities and destination entry rules.
Combined source `1fe0dfa1` closes typed-search and conditional-entry review findings;
[evidence](game-engine-evidence.md#ramos-land-and-library-choice-integration) records qualification. Five frozen shocks have SOURCE_ONLY_UNQUALIFIED
contracts/compiler groundwork; entry/payment producers remain blocked. Fetch costs,
triome cycling and opponent production remain open.

### M3 Build casting during resolution and basic cascade

Qualified above: owned free casts, optional additional costs, legal targets/current
prohibitions, nested completion and private-bottom recovery. Further casting
sources/costs require complete-source qualification under the shared
[mechanics](game-engine-mechanics.md).

### M4 Complete the cascade package and casting history

Apex/Wanderer/Imoti/Nexus/Sorcerer/Ethersworn are qualified above. Next:
Enigma's exact battlefield-to-own-graveyard successor and third-from-top library
movement/knowledge; Yidris needs complete AST/binding before grants. Remaining turn/zone scopes follow. Add Averna's action, Call Forth's other-spell mana
values and Vega's non-hand-cast draw. Preflight effective trigger batches and checked
IDs from common physical bounds; no card/profile ceilings, truncation or partial
publication. Copies require a revised capacity proof.

### M5 Add color and mana engines

Complete Leyline opening action/layers/hybrid costs, Prismatic Omen, Lantern grants,
Faeburrow dynamic quantities, Selvala allocation/draw and Omnath Phyrexian payment,
unspent-mana replacement and private choices. Keep permanent color, spell colors,
mana spent and commander identity distinct.

### M6 Complete card advantage and flexible choices

Complete Aragorn, Whirlwind, Temur Ascendancy, Rhystic payments, distributed Abzan
counters, Esper discard, Quandrix choose-two/graveyard targets, Conflux colored
search and Urban Evolution land permissions. Qualify partial target legality,
mandatory/optional order, private views and permission expiry.

### M7 Complete protection and board interaction

Complete Anguished Unmaking, Void Rend, Ruinous Ultimatum, Child, Avacyn, Escort,
Exterminatus, Farewell and Akroma's Will. Preserve actual recipient sets,
commander-dependent announcement permissions, ordered modes, keyword removal,
lookback triggers and protection's targeting/damage/blocking/attachment behavior.

### M8 Add spell and permanent copies

Complete Double Major, Creative Technique/demonstrate and Protege through generic
copyable characteristics, independent targets/choices, ownership/control and legendary
exceptions. Copies are not casts. Revise token/trigger capacity before copied tokens
can carry abilities; include token cessation and private recovery.

### M9 Add linked permissions, bestow and Sagas

Complete Mosswort's private linked hideaway permission, Chromanticore bestow/type/
attachment transitions and Thunder's lore/chapters/final sacrifice. Invalidate stale
source incarnations and preserve allowed viewers.

### M10 Complete phasing and remaining effects

Complete Teferi's Protection, Coalition's repeated modes, Progenitus replacement,
Archangel hand casting, Zacama cast-sensitive trigger/activations, Breya's tokens/
atomic sacrifice and Door's player loss. Phasing retains identity; it is not exile.
Never silently erase an unsupported target alternative.

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
Follow the [development workflow](game-engine-development.md) for freezing,
serialized native work, preserved failures/artifacts and stable main integration.
Exact-target admission requires complete semantics; coverage percentages cannot substitute.
