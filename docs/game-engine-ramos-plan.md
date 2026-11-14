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

- **M0:** frozen against main `bbdba9f5`, planning base `46ef08fa`; all 85 archive
  records verified. Production audit: 11 lowered/validated, 74 Unsupported, zero
  Invalid. The baseline retains exact diagnostics; later checkpoints do not rewrite it.
- **M1:** complete frozen Ramos, Hardened Scales and Corpsejack Menace have private
  paid mechanics/recovery witnesses. Ramos uses current/LKI spell colors, exact
  incarnations, positive counter-removal costs, ten-mana production, once-each-turn
  tracking and nested keep/undo. Prospective Add/Multiply replacements use atomic
  transactions and affected-controller ordering. Entry/simultaneous/distributed
  placement producers remain unavailable.
- **M2 partial:** Cultivate/Reach support optional basic search, temporary public
  reveal, identity-bound battlefield/hand partition, tapped entry and shuffle.
  The other land/ramp sources below remain obligations.
- **M3:** source `2b83a482` combined main `33dcbe9d`; published feature `1377fae8`.
  Complete Agent/Bloodbraid cascade and separate non-target Dismantling Blow/Natural
  Reclamation witnesses close the focused source/runtime exit: owned exile casting,
  mode/target/payment routing, source-bound optional kicker, conditional mandatory
  draw, private random bottom and genuine paid Aura/Kutzil prohibition pair.
  Passed 528 distinct Rust tests (538 executions), 2,160 schema probes and 925
  compiled-source rows, all-target compilation and formatting. The single review's
  empty-source-block and stale-format findings are fixed; actual RED/GREEN retained.
  Test producer `7bd4f7e4`, CLI `5aba2e46`; formats 24/19, IR1.
- **M4a pending:** reviewed source `0e26ccd1` on `2b83a482` lowers complete Apex
  through independent self-cascade entries and atomic whole-batch trigger preflight.
  Three compiler/13 core tests passed; one source/test review found no concrete
  defect. Private counter roots have off-color cards and establish no legal-pod
  admission. Adjacent checks, own CLI/schema export and fresh-main combination
  remain pending; Wanderer stays wholly Unsupported. No new format is claimed.

Authoritative coordinator receipts are ignored
`generated/ramos/m3b-integrated/` and `generated/ramos/m3c-integrated/`;
M3c's `integration.json` links exact copied executables, source/settings, review,
failures and timings. Apex producer evidence is in its isolated checkout's
`generated/ramos/m4a/focused-04/`; the single review manifest is retained separately.
Old evidence keeps its original source and optimization identity. Full contract
research and historical matrices remain in Git at `30ec86ee` and preserved ignored
copies; they are not duplicated operating manuals.

The current coordinator imports set-family main `93afb8fa` after its published
`7a3d4b6e` producer. Combined 25/20 headers and optional-cost/set exclusion checks
are **unqualified** until affected tests run. The separate scalar family reserves
26/21; neither producer's acceptance transfers. Client qualification has the next
native slot; no coordinator Cargo is queued.

## Ordered remaining milestones

### M2 Complete land and ramp infrastructure

Complete typed multicolor lands, shock/conditional/tapped entry, fetch life/tap/
sacrifice costs, typed nonbasic searches, mana choices/opponent production and hand
cycling. Cover Farseek, Nature's Lore, Skyshroud Claim and all triome modes; Forest
need not mean basic. Preserve reveal, optional quantities and destination entry rules.

### M3 Build casting during resolution and basic cascade

Retain completed casts distinct from copies/failed proposals. Free alternative
cost leaves additional costs payable and mana value unchanged. Preserve exact
owner-below-child completion, no premature priority/SBA, current prohibitions,
second mana-value guard, decline/no-hit, countered originals and legal modal targets.
Recovery must bind cost selections, payments and private bottom membership.
The focused exit above does not cover every possible casting source or cost.

### M4 Complete the cascade package and casting history

Finish Apex qualification, then complete Wanderer's live controller-creature haste
and independent cascade entries. Add Ethersworn affinity, Enigma graveyard return,
Imoti/Nexus/Sorcerer/Yidris grants and actual turn/zone casting history, including
casts before grant-source entry. Add Averna's action, Call Forth's other-spell mana
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
Freeze source through native passes, serialize the shared lock, preserve failed
results and exact copied artifacts, and reconcile current main only at stable
boundaries. State executed selectors, overlap, omissions and source identities.
No coverage percentage substitutes for complete semantics or exact-target admission.
