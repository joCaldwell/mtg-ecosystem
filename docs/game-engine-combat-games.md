# Ordinary combat and complete scenario games

Contract frozen after independent rules and configuration review, October 13, 2026. Based on merged main `bb3f245` including the
turns/permanents implementation and preceding departure review fixes. This is
one bounded delivery, not full P3/P4/P5 or legal Commander match support.
Integration and simulator/evidence have one owner; separate agents own runtime,
scenario/compiler tests, and independent rules/contract/adversarial review.

## Source and admission freeze

Reuse the five exact source definitions and canonical definition digests in
`engine.rs::ALLOWED`, IR v3, and the schema-3 archive provenance. Gameplay uses
only Forest, Island, Grizzly Bears, and Unsummon. Isamaru remains confined to
its existing commander-foundation configuration. No new cards, parser changes,
keywords, or source-specific runtime dispatch. Complete-card compilation and
paired Cruel Edict/unknown semantic rejection remain required.

Rules are the official September 25, 2026 text:
https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Independent review redownloaded and verified the bytes. Relevant rules: 104.2a,
104.3b/c, 104.4a, 117.3–5, 302.6, 400.7, 504, 506–511, 514, 704.3/5a/b/f/g,
800.4, 802.4. Current 510.1c permits any split among multiple blockers; no
historical damage-assignment order or lethal-first restriction is imposed.

Admission is exact whole-configuration equality. Existing five configurations
retain their old bounded semantics. New configurations enable ordinary combat,
ordered draws, cleanup discard and life/failed-draw SBAs. No commanders. Each
player's reachable mana is monochromatic: P0/P2 own Forests only, P1/P3 Islands
only, with no land-control effects. Generic spending therefore has no strategic
color choice. A bounced Bears in a blue player's hand cannot be paid for;
existing keep/undo reversal applies. Mixed-mana configs reject. No owned pool
exceeds sixteen objects, preserving the reviewed atomic batch work bound.

All initial objects have incarnation 0, are untapped, owned/default-controlled
by their stated player, with no control effects. Initial battlefield creatures
have been continuously controlled since before the start of their controller's
most recent turn. New entrants/reentries and controller changes reset that fact;
start-of-turn processing updates it even if an object does not need untapping.
Start turn 1, P0 active/priority, precombat main, unused land play, empty stack,
mana, damage, combat and input history. Libraries below list TOP FIRST explicitly;
physical object iteration order must never supply library order.

### G: `combat_game()` / `four-seat-combat-game-v1`

Life P0=6, P1=P2=P3=2. Exact physical cards:

| IDs | Owner | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Forest | battlefield |
| 1 | P0 | Grizzly Bears | battlefield |
| 2 | P0 | Forest | hand |
| 3 | P0 | Grizzly Bears | hand |
| 4,5,6 | P0 | Forest | library ordered **6,4,5** |
| 7 | P1 | Island | battlefield |
| 8 | P1 | Grizzly Bears | battlefield |
| 9 | P1 | Unsummon | hand |
| 10 | P1 | Island | library |
| 11 | P2 | Forest | battlefield |
| 12 | P2 | Grizzly Bears | battlefield |
| 13 | P2 | Forest | hand |
| 14 | P2 | Grizzly Bears | hand |
| 15,16 | P2 | Forest | library ordered **16,15** |
| 17 | P3 | Island | battlefield |
| 18 | P3 | Unsummon | hand |
| 19–25 | P3 | Island | hand |
| 26 | P3 | Island | library |

Scripted witness, using visible handles/decisions only: turn1 P0 plays a Forest,
casts hand Bears with both Forests, attacks P1 with its older Bears. P1 blocks
with Bears and, in blocker priority, casts Unsummon targeting its own blocker.
P3 responds targeting the same blocker. Top response returns it; lower response
fails its sole target. The attacker remains blocked and deals no player damage.
Turn2 P1 draws and passes. Turn3 P2 draws, plays Forest, casts hand Bears and
attacks P0 with the older Bears. P0 blocks with its newer Bears; both die together.
Turn4 P3 draws and discards Island19 in cleanup (eight-card hand to seven), choosing the first
Island in its deterministic own-hand observation order.
Turn5 P0 draws library top 6, attacks P1 with its surviving Bears; P1 loses.
Turn6 P2 draws library top 15 and attacks P0 with its surviving Bears; P0 takes
2, to 4. Turn7 P3 attempts an empty-library draw and loses at stabilization.
Turn8 P0 draws library top 4, attacks P2, whose Bears is tapped; P2 loses and P0
wins at life4. P0 library retains 5. No concession or host horizon creates this
outcome. Blue players deliberately do not play lands; later green turns pass
main-phase actions once the scripted initial casts are complete.

### I: `combat_interactions()` / `four-seat-combat-interactions-v1`

Life P0=6, P1=P2=P3=2. Battlefield: P0 Bears cards0,1,2; P1 Bears3,4;
P2 Bears5; P3 Island6. P3 hand Unsummon7. All libraries empty. This exact state
supports branching interaction witnesses, with no claim each branch is a full
game. Declare 0 and 1 attacking P1, 2 attacking P2. P1 double-blocks 0 with 3/4;
P2 chooses no blockers. Assign 0's two damage 1/1 (legal current-rule split),
blockers deal 2 each to 0, and unblocked attackers deal 2 to each defender.
Creature0 dies; P1/P2 lose simultaneously; their objects depart; P0/P3 remain.
A second split 2/0 kills blocker3 in the same SBA wave before departures.
Branches remove one/both blockers with Unsummon or concessions, reject invalid
complete declarations, and test departures at every automatic/combat boundary.
Choosing no attackers skips blockers/damage. Declared attackers later all removed
does not invoke that skip rule. A departed attacked player receives no damage;
its surviving attacker remains attacking (800.4e).

No all-player draw or zero toughness is reachable from this positive-life,
positive-toughness admitted pool. Implement/test simultaneous all-player loss
and zero-toughness distinction using explicitly private kernel fixtures, without
expanding admission or claiming a production complete-game draw witness.

## Decisions, events, and continuation contracts

Extend the existing shared frames, typed batch pipeline, queries, observation,
snapshot and replay. Snapshot/bundle v8, projection v6, query sidecar v5. IR stays
v3; general combat rules belong to core, not per-card programs.

Attacker response is the complete set of distinct eligible creatures and live
opponent defenders, including empty. Eligibility requires untapped creature,
active control continuously since its latest turn began; declaration taps through
typed events. Blocker responses are complete distinct assignments by each relevant
defender in APNAP order; blockers must be untapped creatures controlled by that
defender and can block only attackers aimed at that player. Sickness does not
prevent blocking, and blocking does not tap. Multiple blockers are supported;
one blocker blocking multiple attackers is excluded by the pool. No priority
between defender declarations. Record the blocked bit permanently for this combat.

Priority windows: beginning combat; after attackers; after all blockers; after
simultaneous damage and stabilization; end combat. Instant responses use the
existing stack and payment path. No priority between assignment choices and damage. Zero or negative power
assigns no damage and emits no positive damage occurrence (CR 120.8/510.1a).
Assignment decisions expose full power and recipient domain, validating complete
nonnegative exact-quantity distributions summing to current assignable power.
Sole-recipient assignments may be automatic; genuinely multiple-recipient splits
must be chosen. Blocked attackers with no remaining blockers assign nothing.
Unblocked attackers assign to their defender unless that player has left.

Typed simultaneous damage aggregate captures all source incarnation/controller,
recipient and amount facts against one common pre-state, then actual player-life
and creature-marked-damage results. Multiple sources hitting one recipient sum
without breaking the existing independent-footprint atomic batch contract.
Marked damage is separate from toughness. Central Move/LeaveGame/control-change
paths remove appropriate combat membership and reset object-local damage/control
history. Removing blockers never unblocks an attacker.

At proper priority checkpoints, freeze all lethal-damage/zero-toughness/life-loss/
failed-draw candidates, apply one simultaneous SBA wave, then repeat. Lethal is
destruction; zero toughness is a distinct graveyard action. Freeze all losers
before departure cleanup or winner testing. Reuse existing departure machinery;
never declare an intermediate winner while simultaneous losers remain. All-loser
private fixtures yield rules-defined draw. Concessions remain ordered, drain
before ordinary work, and preserve all merged ordering/revalidation regressions.
Do not stabilize between resolving instructions. Repeated stabilization has
serializable/budgeted boundaries and no premature queries/priority.

Successful draws remove exactly the explicit top entry and use typed move/outcome
facts; empty attempts set a failed-draw fact until the next SBA checkpoint. Empty
library without attempt does not lose. Cleanup first requests exact excess-hand
discard with own-hand handles, commits simultaneous owner-graveyard moves, then
clears all damage, checks SBAs and advances. No triggers/temporary effects are in
pool. If cleanup SBAs occur in a private fixture, apply the 514.3 exception with
priority and another cleanup; no ordinary cleanup priority is offered.

Queued departures revalidate accepted declarations, splits and discards before
consumption. Skip departed choosers, preserve still-valid unrelated input, and
never apply ghost damage from removed combatants or to departed recipients.
Revalidate collected assignments after departures before the atomic damage event;
retain legal surviving choices and request revised choices when necessary.

Every attack, defender-block, assignment and discard request; accepted pending
response; damage/draw/discard/clear boundary; SBA wave; and departure continuation
must restore and replay. Stale/duplicate/foreign/wrong-player/wrong-timing and
incomplete declarations reject without semantic mutation. Budget exhaustion,
unsupported execution and faults remain separate from win/draw/loss.

## Verification and exclusions

Independently test legal/illegal declarations, splits 1/1 and 2/0, blocker removal,
responses before damage, simultaneous deaths/losses, continuous-control reset,
attack-multiple-opponents, no attackers, departed defenders, surviving priority/
turn order, draws/order/privacy, cleanup, untap/land/mana reset, and payment
reversal. Preserve every merged regression. Test hidden library-order and opponent
hand noninterference, pure budgeted queries and saved query recovery, stale receipts,
fork independence, forged snapshots, and complete replay. Compare off/compact/
detailed trace modes and budgets zero/one/larger/unlimited. Recover every meaningful
new boundary and retain complete-game journals, initial states and outcomes.

Runner policy lives outside core and receives only player-facing observations,
requests, responses and budgeted query results. Host verification may read snapshots
and raw IDs separately. Include distinct completed, suspended/truncated and fault
classifications; complete-game success must reach the frozen real outcome.

Excluded: full Commander setup/decks/casting/return/tax/damage; arbitrary admission;
combat keywords, planeswalker/battle attacks, attack/block costs/requirements,
triggers, prevention/replacement, continuous effects, temporary effects, general
control-changing spells, shuffling/randomness/mulligans, extra phases/turns.
Run package checks, debug/release tests and fresh offline retained-source rebuild/
replay. Resolve independent review findings, update maintained docs and evidence,
commit locally on `feat/engine-combat-games`; do not merge.

Review: independent rules reviewer verified rules bytes, G eight-turn outcome,
I current-rule splits, departure semantics and continuation requirements. Separate
configuration/compiler reviewer checked both exact setups, mono-color payment
closure and sixteen-object batch bounds. No unresolved contract objection.
