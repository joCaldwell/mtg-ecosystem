# Ramos deck engine implementation plan

Make Jo's Cascade, with Ramos, Dragon Engine as commander, executable end to end
using its unchanged saved list. Development follows the dependencies below and
delivers complete mechanic families in small verified checkpoints.

M0 is frozen against `origin/main` at
`bbdba9f5add0bfd0c75c0ec18eb2986581c3e534`, with the planning-only commit
`46ef08faedb8c5fa9c1c1fa9a8afefd1f85e1f6c` directly above it.
The prior keyword/compiler round is integrated. The
[M0 support matrix and dependency backlog](game-engine-ramos-support.md) records
the production-compiler baseline and later bounded source witnesses. M1a Ramos
and the M2 Cultivate/Reach slice now execute; exact-target qualification remains
incomplete. The M1 counter-placement family is also source/runtime-qualified.
M3b implements the basic cascade/resolution-casting family. The M3c bounded
optional-mana/instant-cascade exit cohort is separate from exact-target admission;
M3’s focused source/runtime exit is now qualified on the combined source below;
exact-target and consumer admission remain separate.
This plan is not exact-deck engine acceptance.

## Target and source authority

- Deck-builder owner: jcaldwell; deck 11, **Jo's Cascade**, paper lifecycle.
- Frozen list target: revision 17, edit version 0; the app reports 100/100 cards
  and 37 lands. The export is reproduced at the end of this plan.
- Normalized export SHA-256 (LF lines, final newline):
  `f9eb5bc8ea479e036fc24c3fba27383ee2f030191d1a689de80bdb30e2278ee1`.
- Preserve every listed card, quantity and commander designation. The maybe
  list and acquisition tags do not change engine membership.
- The product goal is complete four-player games with the exact deck, first
  headless and then through the Rust host and TypeScript SDK.
- Use existing reviewed decks as opponents at first. Later test multiple Ramos
  seats and interactions with the full admitted pool.
- Intermediate reduced decks are separately labelled development profiles;
  they never qualify the exact target.
- New list revisions are explicit new targets. Never silently chase live app
  edits or modify the owner's paper deck to reduce implementation work.
- Keep the deck-builder independent. Export and source resolution happen
  outside deterministic core; core receives a validated immutable catalog.

The connected deck-builder was read successfully on 2026-11-13 in the owner's
America/Chicago timezone. Its full Oracle text is the product discovery source.
Executable fixtures must come through the frozen archive/provenance workflow
in [catalogs](game-engine-catalogs.md), with differences reconciled explicitly.

## Existing foundations and concrete gaps

The [current map](game-engine-current.md) records complete finite matches,
priority, payments, combat, Commander lifecycle, ordinary triggers, creature
tokens, +1/+1 counters, modal programs, removal, wipes, attachments, replay,
local hosting, and a separately bounded sampled-search implementation.
These foundations should be reused where their exact contracts fit.

The inspected contracts expose ordinary mana costs, fixed token definitions,
selected static effects, and narrow search/target/instruction families.
Cascade, general casting during resolution, flexible mana costs, phasing,
copy semantics and Saga rules need new support. Existing flying, counters or
modal choices do not by themselves support the complete cards below.

The frozen archive contains source records for every name in the exported
target. It currently marks these records **unsupported at the parser stage**:
Call Forth the Tempest; Selvala, Heart of the Wilds; Averna, the Chaos Bloom;
Omnath, Locus of All; Yidris, Maelstrom Wielder; Creative Technique; Double Major.
Their milestone includes narrowly scoped parser/archive work before executable
lowering. Read the oracle-parser manual before changing that package.
A parsed archive record does not establish complete binding, compilation,
runtime support or profile admission.

## Completion criteria

The exact target is complete when all of these gates pass on identified source:

1. Every exported card has an exact frozen source, complete AST/binding and
   executable lowering, capability validation and catalog provenance.
2. The unchanged target passes Commander deck and pod admission with the pinned
   rules/legality policy. Every mode and optional branch is represented;
   unsupported branches cannot be omitted.
3. Accepted-input scenarios exercise every card's distinct behavior and the
   important compositions. Synthetic unit states supplement these witnesses.
4. The exact list completes four-player games under policies that receive only
   permitted observations. A preregistered final batch rotates Ramos through
   all seats and includes multiple Ramos seats, varied opponent decks and seeds.
   Record natural outcomes, faults and truncations separately; replay every run.
5. Snapshot/restore and accepted-input replay cover the new pending choices,
   private information, nested casts, copied objects, replacements and durations.
6. The host and SDK admit the catalog, expose every new decision and projection,
   and preserve privacy, retries, recovery and reconnect. Run a four-client
   completion with the exact deck.

Sampled search for this pool is a separate follow-on. A browser gameplay UI,
public deployment and support for arbitrary user decks need separate scope.
The existing deck-builder website does not become a game client implicitly.

## Ordered milestones

Each milestone below may require several implementation checkpoints. Advance
the observable deck behavior; do not count source admission and a policy tweak
as unrelated product features. Add useful headless witnesses as mechanics land,
rather than postponing execution to the final milestone.

### M0 Freeze inputs and produce the executable support audit

Record the current main/build identity, exact deck export digest, card Oracle
IDs, frozen source/AST/binding hashes and the rules/legality identities.
Compare app text against the frozen sources before choosing fixtures.

Run the production compiler once across the target under the shared native
lock. Produce one row per definition with separate source, parse, binding,
lowering, capability, scenario and consumer status; retain exact diagnostics.
Map each rejection to a milestone below. Preserve the failing baseline.

Exit: a reproducible target manifest, support matrix and dependency backlog.
This audit is bounded setup, not a new coverage percentage or a tooling project.

Completed input/compiler audit: [support matrix](game-engine-ramos-support.md),
[revision-17 manifest](../packages/game-engine/tests/fixtures/ramos/revision-17.manifest.json)
and [preserved baseline](../packages/game-engine/tests/fixtures/ramos/baseline-46ef08fa.json).
All 85 definitions resolve to verified archive lines; the export has 100 cards
and 37 lands with the stated digest. Eleven definitions lower and validate;
74 reject, including the seven parser-stage failures listed above.
The connected app requires reauthentication, so the app/frozen Oracle-text
comparison remains explicitly unverified. No new deck revision was read.
This source audit does not admit a Ramos catalog, headless profile, host or search.

### M1 Complete Ramos and counter replacement

The [M1 family contract](game-engine-ramos-m1-contract.md) records the frozen
sources, proposed interfaces and focused witnesses. Preserve cast-event identity;
use current or last-known spell information at trigger resolution under CR 608.2h.

Support Ramos's clause "for each of that spell's colors", including frozen
cast-event facts, colorless and multicolor spells. Keep spell colors, mana
spent and commander color identity separate.

Implement removal of five +1/+1 counters as a mana-ability cost, exact ten-mana
production, atomic payment and the "once each turn" restriction. Track use by
the correct ability/object incarnation; resets and failed payments need tests.

Then add Hardened Scales and Corpsejack Menace as counter-placement
replacements, with affected-player ordering and one application per event.

Exit: a legal paid Ramos cast/counter/mana/spell sequence with snapshot/replay;
zero-color and copied-but-not-cast controls; alternate replacement orders;
ability-use tests across opponents' turns and source departure/reentry.

### M2 Complete land and ramp infrastructure

The [M2 family contract](game-engine-ramos-m2-contract.md) records the complete
source boundaries and initial search/reveal/partition checkpoint.

Reuse basic lands, Command Tower, Arcane Signet and Golgari Signet only after
their complete exact sources pass M0.

Implement typed multicolor lands, shock-land entry life choices, Cinder Glade's
basic-land condition, enters-tapped lands, fetch-land life/tap/sacrifice costs,
and typed library searches. A Forest card need not be basic; entry rules still
apply after putting a found land onto the battlefield.

Cover Cultivate, Kodama's Reach, Farseek, Nature's Lore and Skyshroud Claim with
correct reveal, split destinations, optional quantities and shuffle behavior.
Include cycling from hand for Ketria Triome, Spara's Headquarters and Ziatora's
Proving Ground; their mana abilities alone do not complete those sources.

Implement Sulfurous Springs, Yavimaya Coast and the three Talismans' production
and damage, City of Brass's separate becomes-tapped trigger, and opponent-land
production queries for Exotic Orchard and Fellwar Stone.

Exit: legal opening development with genuine payment, life/damage, search,
entry and cycling witnesses; private search projections and replay; no hidden
library inspection by policy. Lands support each printed basic subtype.

### M3 Build casting during resolution and basic cascade

The [M3 casting contract](game-engine-ramos-casting-contract.md) records the
frozen basic sources, shared continuation/payment interfaces and remaining exit
obligations. M3b now qualifies complete basic Agent/Bloodbraid source/runtime and
recovery on the checked M3a foundation. The separate M3c cohort covers source-bound fixed-mana kicker and a naturally
developed paid prohibition context using two records outside the frozen target.
Parent source `2b83a482` integrates main `33dcbe9d` and closes the focused M3
source/runtime exit after the single review’s fixes, 528 distinct Rust tests and
2,160 schema probes. This does not admit a profile/pod or host/search consumer.

Introduce a reusable casting-permission and continuation path supporting
casting from the indicated zone while another invocation resolves, choosing
not to cast, target/mode selection, optional costs and complete payment rollback.
"Without paying its mana cost" is an alternative payment, not mana value zero.

Start cascade with Shardless Agent and Bloodbraid Elf. Exile sequentially,
identify the eligible nonland using mana value, offer the permitted cast,
then randomize the remaining exiled cards to the library bottom at the correct
time. A nested cast must not resume the parent continuation too early.

Exit: cascade into a creature, targeted spell, modal spell and another cascade;
decline, empty library/no hit, uncastable hit, prohibitions and countered
original-spell cases; deterministic randomization; suspended choice restore,
visibility and accepted-input replay. Never simulate cascade as a free draw.

### M4 Complete the cascade package and casting history

The bounded [M4a Apex checkpoint](game-engine-ramos-casting-contract.md#m4a-direct-multiple-self-cascade-on-permanent-spells)
qualifies complete frozen Apex Devastator through the existing cascade family,
with independent indexed self triggers and whole-batch capacity preflight.
Wanderer's complete controller-creature haste grant still rejects; its intrinsic
haste is not a substitute. M4 remains open for Wanderer and the full package below.

Add separate multiple cascade triggers for Maelstrom Wanderer, Apex Devastator
and Call Forth the Tempest. Complete Ethersworn Sphinx's affinity and Enigma
Sphinx's graveyard trigger and third-from-top insertion as part of their source
admission.

Add Imoti, Maelstrom Nexus, Wild-Magic Sorcerer and Yidris through generic
ability grants and turn/zone-qualified casting history. Include spells cast
before a grant source enters and distinguish hand, exile and command-zone
casts. Multiple cascade grants must compose correctly.

Add Averna's choice during cascade at its specified point before the cast
decision. Complete Call Forth the Tempest's damage from other spells' total
mana value and Vega's non-hand-cast draw trigger.

Exit: a genuine Ramos-powered nested cascade turn including granted and
multiple cascades, history resets and source changes, affinity without mana
value changes, and replay through each nested decision.

### M5 Add color and mana engines

Implement hybrid costs for Leyline of the Guildpact and Phyrexian payment for
Omnath. Reuse the payment model rather than enumerating special card cases.

Add Prismatic Omen's basic-land-type addition, Chromatic Lantern's granted
mana abilities, and Leyline's opening-hand permission, permanent color changes
and land types. Evaluate the relevant layers, duration, source removal and
ability/production queries consistently. Leyline changes nonland permanents,
not spells on the stack.

Complete Faeburrow Elder's dynamic characteristics and per-color mana output,
Selvala's greatest-power/color-allocation mana ability and optional enters draw,
and Omnath's unspent-mana replacement and private look/reveal/draw/mana choice.

Exit: land-type and granted-mana interactions with typed searches and opponent
production, color changes without spell-color confusion, replacement timing
across phase boundaries, and private-choice recovery.

### M6 Complete card advantage and flexible choices

Complete Aragorn's independent color-qualified triggers including scry,
Whirlwind of Thought, Temur Ascendancy, and Rhystic Study's opponent payment
and optional draw sequence. Trigger choices, payments and ordering remain
distinct from ordinary spell casting.

Complete Abzan Charm's distributed counters, Esper Charm's player discard,
Quandrix Command's choose-two and graveyard/library targets, Conflux's
color-qualified multi-card search/reveal, and Urban Evolution's additional
land permission. Reuse the target engine while supporting each required
target count, partition and mode combination.

Exit: mandatory/optional choice ordering, duplicate and overlapping targets,
partially illegal resolution targets, failed payment, source departure,
private scry/search/discard views and exact replay. Extra land permissions
compose and expire correctly.

### M7 Complete protection and board interaction

Complete Anguished Unmaking, Void Rend, Ruinous Ultimatum, Child of Alara,
Avacyn, Dauntless Escort, Exterminatus, Farewell and Akroma's Will.

Extend keyword grants/removal and protection to their actual recipient sets.
Avacyn protects other permanents, not just creatures. Exterminatus removes
opponents' indestructible before destruction. Akroma's Will's mode permission
depends on commander control as cast. Farewell's selected modes execute in
their proper order.

Exit: protection's targeting/damage/blocking/attachment interactions,
simultaneous wipes and lookback triggers, commander movement, keyword removal
against intrinsic/static/temporary grants, and full snapshot/replay witnesses.

### M8 Add spell and permanent copies

Build copyable characteristics, copied invocation metadata, ownership/control,
token creation from resolving permanent-spell copies, and explicit legendary
exceptions. A spell copy is not a cast unless an effect instructs casting it.

Complete Double Major, Creative Technique including demonstrate/opponent
copies, and Sakashima's Protege including its optional enters-as-copy choice
and eligible permanents that entered this turn.

Exit: independent targets and choices for copies, source disappearance,
legend-rule outcomes, copied cascade versus actual cast triggers, copied
permanent histories and token cessation, private information and recovery.
Before admitting copies, revise token representation and the generic trigger
capacity proof: copied tokens can carry abilities, invalidating the current
assumption that generated creature tokens have no triggered abilities.

### M9 Add linked permissions, bestow and Sagas

Complete Mosswort Bridge's private hideaway selection, face-down exile,
object-linked permission and total-power condition. Preserve allowed viewers
and invalidate stale links correctly across source incarnations.

Complete Chromanticore's bestow alternative, stack types/targeting, enchanted
grants, and transition to a creature when unattached, including a bestow
target becoming illegal.

Complete Thunder of Unity's lore counters, chapter triggers, final-chapter
sacrifice timing and distinct turn-long triggered abilities from chapters II/III.

Exit: accepted-input witnesses for every transition; hidden exile projection,
source reentry, copied objects, leave/reenter, chapter timing and continuation
recovery. Do not admit only the ordinary-creature half of Chromanticore.

### M10 Complete phasing and remaining effects

Implement phasing as its own status with retained identities and correct
indirect phasing, untap timing, attachment handling and multiplayer control.
It must not masquerade as exile/reentry.

Complete Teferi's Protection's life-total restriction, player protection,
phase-out and duration; Unite the Coalition's repeated mode selection and
independently bound target occurrences; Progenitus's protection and
graveyard replacement from every source zone.

Complete Maelstrom Archangel's optional hand cast during resolution,
Zacama's cast-sensitive enters trigger and activated abilities, Breya's
artifact creature tokens and two-artifact modal activation, and Door to
Nothingness's targeted player-loss effect.

Some of these primitives can land earlier with their natural dependencies.
Keep them here as explicit completion obligations until qualified. For
planeswalker references in target domains, either support the full domain in
the closed admitted pool with explicit exclusions of unavailable inhabitants,
or implement the missing type before broadening admission; never silently
erase a source target alternative.

Exit: protection/phasing/wipe interactions, next-turn durations when players
leave, replacement/Commander choices, cast-versus-put/copy controls, Breya
payment atomicity and target semantics, and player loss during nested resolution
with remaining-player cleanup/replay.

### M11 Qualify the exact deck and host it

Build a separate immutable Ramos catalog/profile covering the exact target and
selected opponent definitions. Verify capability closure and generic resource
bounds, particularly copied trigger sources, nested cascade continuations,
token histories and private decision payloads.

Extend public-observation policies to make legal useful choices without
card-name dispatch or access to authoritative hidden state. Run focused
accepted-pod witnesses as each family lands, then the final fixed-seed
completion/replay batch described in the completion criteria.

Read the game-server and SDK manuals before adapting host capability admission,
generated contracts, new decisions and private projection. Verify recovery
during a cascade cast, hideaway selection, replacement ordering and copy choice;
test retries, reconnect, backpressure and a four-client completed match.

Exit: exact-list headless and host acceptance with source-qualified evidence,
visible coverage omissions and no unsupported reachable source behavior.
Performance work follows a measured bottleneck in this workload, rather than
a new blanket benchmark matrix.

## Dependencies and parallel work

The initial path is M0 -> M1 -> M3 -> M4. M2 can proceed alongside M1 with
coordinated payment/target interfaces. M5 follows the mana/payment foundations;
M6 and M7 can proceed independently after the shared trigger/choice contracts
stabilize. M8 depends on M3 and the casting-history model; M9 depends on the
relevant payment, linking and continuous-effect primitives. M10 composes these
foundations; M11 integrates the qualified whole.

Use separate orchestrator-owned worktrees and explicit file ownership.
Contracts, compiler dispatch and core payment/continuation modules need a
single integration owner when multiple families touch them. Source/rules
research and dedicated scenarios can run in parallel. Native builds/tests
remain serialized through the documented shared Cargo lock.

The prior temporary-keyword and compiler work is integrated at this M0 baseline.
Reuse its source-qualified foundations; it does not by itself qualify this deck.

## Verification and progress reporting

For each checkpoint:

- Name the exact intended deck behavior, source boundary and explicit exclusions.
- Review the frozen card text and pinned rules; cite rules in semantic code.
- Add capability validation, a positive execution and an unsupported near-miss.
- Run the smallest relevant compiler/contracts/core/simulator checks, confirm
  actual selected test counts, and preserve failures.
- Exercise accepted-input snapshot/replay and privacy where the edit changes
  choices, hidden zones, history, payments or continuation state.
- Commit and push coherent checked increments, integrate without discarding
  concurrent work, and keep one concise family contract current.
- Report newly executable target cards/branches, witnessed interactions,
  remaining dependency blockers and consumer admission separately.
- Keep generated source dumps, build outputs and raw receipts ignored.
  Preserve useful evidence and existing shared caches.

Maintain the [single card support matrix](game-engine-ramos-support.md).
Each row needs the complete card source status, owning milestone, current
diagnostic, tests and exact evidence. Update that matrix instead of adding
repetitive implementation transcripts. A parser-only pass, compilation,
headless admission and hosted acceptance remain different states.

Do not run full release/profile/search/SDK matrices after every small change.
Broader integration runs belong at the relevant milestone gate and final
target acceptance. No runtime guard, capacity fault or completion policy may
be weakened to make this deck appear supported.

## Exact target export

Commander: Ramos, Dragon Engine. The following is the app export of deck 11
at revision 17; the first entry is designated commander.

```text
1 Ramos, Dragon Engine
1 Abzan Charm
1 Akroma's Will
1 Anguished Unmaking
1 Apex Devastator
1 Aragorn, the Uniter
1 Arcane Signet
1 Avacyn, Angel of Hope
1 Averna, the Chaos Bloom
1 Bloodbraid Elf
1 Breeding Pool
1 Breya, Etherium Shaper
1 Call Forth the Tempest
1 Child of Alara
1 Chromanticore
1 Chromatic Lantern
1 Cinder Glade
1 City of Brass
1 Command Tower
1 Conflux
1 Corpsejack Menace
1 Creative Technique
1 Cultivate
1 Dauntless Escort
1 Door to Nothingness
1 Double Major
1 Enigma Sphinx
1 Esper Charm
1 Ethersworn Sphinx
1 Exotic Orchard
1 Exterminatus
1 Faeburrow Elder
1 Farewell
1 Farseek
1 Fellwar Stone
6 Forest
1 Golgari Signet
1 Hallowed Fountain
1 Hardened Scales
1 Imoti, Celebrant of Bounty
2 Island
1 Ketria Triome
1 Kodama's Reach
1 Leyline of the Guildpact
1 Maelstrom Archangel
1 Maelstrom Nexus
1 Maelstrom Wanderer
1 Mosswort Bridge
4 Mountain
1 Nature's Lore
1 Omnath, Locus of All
1 Overgrown Tomb
4 Plains
1 Polluted Delta
1 Prismatic Omen
1 Progenitus
1 Quandrix Command
1 Rhystic Study
1 Ruinous Ultimatum
1 Sakashima's Protege
1 Selvala, Heart of the Wilds
1 Shardless Agent
1 Skyshroud Claim
1 Spara's Headquarters
1 Stomping Ground
1 Sulfurous Springs
4 Swamp
1 Talisman of Creativity
1 Talisman of Curiosity
1 Talisman of Unity
1 Teferi's Protection
1 Temple Garden
1 Temur Ascendancy
1 Thunder of Unity
1 Unite the Coalition
1 Urban Evolution
1 Vega, the Watcher
1 Void Rend
1 Whirlwind of Thought
1 Wild-Magic Sorcerer
1 Windswept Heath
1 Yavimaya Coast
1 Yidris, Maelstrom Wielder
1 Zacama, Primal Calamity
1 Ziatora's Proving Ground
```

