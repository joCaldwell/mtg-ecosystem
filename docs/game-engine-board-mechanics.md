# Engine mechanics: permanents, combat and triggers

This file owns combat, damage and life; movement and board-wide effects;
battlefield entry; keywords, attachments and temporary effects; and triggers,
choices, modes and resolution-time counts. [Core mechanics](game-engine-mechanics.md)
owns the rules baseline, [runtime invariants](game-engine-mechanics.md#runtime-invariants),
casting, costs, mana, targets, [tiers](game-engine-mechanics.md#derived-capability-tiers)
and admission. Every shape outside these contracts rejects with a named detail.

## Combat, damage and life

Attackers, blockers and damage commit atomically. Readiness stamps continuous
control at turn entry and resets on control or incarnation change. Blockers have
no damage assignment order; blocked and blocking status survive counterpart
departure until combat ends (CR 509.1g, 511.3). Blocker and damage steps are
skipped only when none attacked. First and double strike have distinct
participant sets; trample and deathtouch assignment, dealt damage and
prevention stay distinct.

Declarations offer exactly the legal set and validate the whole declaration
(CR 508.1c–d, 509.1b). Flying, reach, menace, printed basic landwalk,
horsemanship, shadow, fear, intimidate and skulk restrict blocks per pair
(CR 702.13b, 702.14c, 702.28b, 702.31b, 702.36b, 702.118b); other landwalk
qualities reject. Static block restrictions ("can't be blocked by/except by",
"can't block", "can block only") and blocker counts ("by more than N", "except
by N or more", with menace) read keywords, colors, types and fixed power of
both creatures at the declaration; counts are joint pick rules. "Can't attack
you" and "or planeswalkers you control" restrict those defenders. A creature
that attacks each combat if able must attack only while a defender remains
(CR 508.1d). Relative powers and ambiguous "X and/or Y with Z" filters reject.
Toughness-based combat damage assignment uses current toughness for attackers
and blockers (CR 101.1, 510.1a). Additional blocking capacity stacks, and
unlimited capacity lets a blocker select every legal attacker once (CR 509.1a).
A blocker divides one damage total across its attackers (CR 510.1d); when a
shared blocker connects variable assignments and trample, the attacker
controller assigns the source totals together and each trample allocation
checks combined lethal damage (CR 510.1e, 702.19b).

Simultaneous damage is validated before life/history writes. Combat source
validation reads the captured creature type and keywords from resolved and
static grants (CR 510.2); current or latest-departure source facts govern
damage-time deathtouch, lifelink and beneficiary, including after Equipment
departure. Noncombat damage keeps exact source and invocation controller and can
hit each opponent simultaneously; its count, like life effects' counts, resolves
once from a matching battlefield or graveyard set or the controller's hand size
(CR 402.3, 608.2h). Protection from a color checks derived or last-known source
colors for targeting, attachment, blocking and prevention. Commander combat
history uses actual damage; drains credit simultaneous losses before SBAs.

Replacement/prevention applies once per effect and member under
affected-recipient choices and APNAP order (CR 616.1); the consumed prefix
survives later source departure. Static abilities prevent all, N or half
(rounded as printed) of each matching event (CR 615.10), or double, triple or
add to it (CR 614.1a); sources match on damage-time facts (CR 609.7a),
recipients on the current battlefield, and "to and dealt by" on either.
Resolved preventions last this turn: all or combat damage from any, an
announced or a filtered source, or "the next N damage" to an announced
recipient, a shield spent across events whose simultaneous members its
recipient orders (CR 615.7). Damage that can't be prevented takes no
prevention, protection included, and spends no shield (CR 615.12). Source
choices, "next time" shields, redirection, non-damage replacements and other
replacement/layer interactions reject.

### Planeswalkers

Single-faced planeswalkers with fixed printed loyalty enter with those counters
(CR 306.5b). A fixed loyalty cost is paid after targets, at sorcery timing, and
uses that permanent's single loyalty activation for the turn (CR 606.3–4); the
public object shows current loyalty and that use. Opponents can attack the
planeswalker, and damage removes loyalty counters (CR 508.1b, 120.3c). Zero
loyalty and the legend rule put it into its owner's graveyard (CR 704.5i–j).
“Any target,” “target player or planeswalker,” and “target creature or
planeswalker” include eligible planeswalkers and enforce printed qualifiers.
Emblems and variable loyalty costs require separate lowering.

## Movement, sacrifice and board-wide effects

**Removal.** Indestructible takes precedence over destruction. A regeneration
shield protects one permanent incarnation until its next permitted destruction,
zone change or cleanup; it taps the permanent, clears damage and removes it from
combat (CR 701.19, 514.2), and the affected controller orders competing
replacements. Exile, bounce, sacrifice, zero toughness, legend moves and
departure stay distinct. Removal filters use all types (CR 205.2b), controllers
and statuses; positive type lists are alternatives, exclusions reject any
match, and sacrifice subtype lists are conjunctive. Compensation uses the exact
target's last public appearance (tokens have mana value zero): its controller
can create the printed token recipe or gain life equal to its power, negative
as zero (CR 107.1b, 608.2h).

**Returns.** Entry returns target at trigger placement; no legal target
removes the trigger. One announced card from the controller's graveyard,
filtered by printed types, subtypes, colors and fixed characteristic
comparisons, can return to hand or, if a creature card, to the battlefield under
the controller's control, tapped when printed (CR 110.2a); a printed "may" asks
at resolution. A commander bound for hand still offers the command zone. A
graveyard card can also go to either end of its owner's library (CR 401.4).
Every zone change is one [evaluated move](game-engine-catalogs.md#verb-signatures):
a target that left or became illegal is skipped, a stolen permanent goes to its
owner's zone, and "exiled/returned this way" reads the cards actually moved.

**Sacrifice** is not destruction. Fling-like effects use paid LKI power. Forced
sacrifices choose in APNAP order with earlier choices visible, then move
simultaneously through the replacement and leaves-the-battlefield pipeline
(CR 101.4, 603.10a, 701.21a). Each player, each opponent, or an announced
player chooses qualifying permanents they currently control, read from derived
battlefield characteristics and status (type alternatives, nontoken, combat);
phased-out permanents are excluded (CR 702.26b). A mandatory instruction can
require a fixed number per player, taking all eligible when fewer (CR 609.3).
“Another” excludes the exact source (CR 400.7). A spell can ask each opponent
for a creature with the greatest current power among those they control, each
seeing every tied creature (CR 107.1b, 608.2h). In a step-begin trigger, “that
player” is the active player from the original event (CR 500.1, 603.2).

**Board-wide sets** include granted keywords at execution. Destruction, exile,
bounce and all-recipient damage commit atomically; temporary P/T sets capture
recipients at resolution; set tap and untap change matching permanents
simultaneously. A printed "all non-[subtype] creatures" P/T change reads each
creature's current subtypes as it captures recipients (CR 205.3m, 611.2c).
Destroy, exile and return-to-hand of "all"/"each" permanents
read a general filter as performed (CR 608.2h): conjunctions of card types,
type alternatives, excluded types, basic/legendary, subtypes, colors, controller
(you or your opponents), tapped, untapped, attacking, token, flying and fixed
power, toughness or mana-value comparisons. Types and supertypes are current
structural ones, colors current, subtypes, power, toughness and keywords fully
derived, and mana value printed. Forbidding regeneration is limited to nonland
permanents. Counts use typed outcomes, excluding prevented destruction.
Turn-long exile replacement covers only creatures actually dealt its damage.

**Library search** shows the searcher a non-empty library and offers cards
with the printed qualities. With a quality it may find none (CR 701.23b); "a
card" must find as many as it can (CR 701.23d). Found cards move together onto
the battlefield (optionally tapped) under the searcher's control, or into the
owner's hand or graveyard. A found Aura entering the battlefield enchants an
object its controller chooses without targeting (CR 303.4f), otherwise stays in
the library, as does a found instant or sorcery (CR 303.4g, 304.4). Revealed
hand cards are a public minimum count per owner and Oracle identity, never an
incarnation (CR 701.23e); a reversed cast restores what its announcement
consumed. A grouped search keeps its found cards private and offers one
legal addition at a time until the searcher stops or reaches the maximum.
Every found card in a different-name group has a name and shares none
with the others (CR 201.2b, 709.4a). A search with several printed quality
slots can find a card for each slot, assigning flexible matches to any
eligible slot and using each card at most once (CR 701.23a–b, h).
A search can put one found card on top after shuffling the rest
(CR 701.24b); a printed reveal publishes it without a library handle. A
two-basic split search reveals up to two basic lands, puts one onto the
battlefield tapped and the other into hand (one found land enters tapped); the
split choice offers only the found cards and shows their identities to everyone
without library handles or positions (CR 608.2c–f, 701.20a, 701.23b–e). Other
search partitions and ordering several cards after shuffling are unsupported.

**Hand and graveyard choices** are untargeted (CR 115.10, 608.2c–d): the
controller chooses up to a fixed number of cards with printed qualities from
their own hand (onto the battlefield, optionally tapped) or graveyard (onto the
battlefield or into hand), sharing search's entry questions and commander
returns; a chosen Aura enchants a chosen host or stays put (CR 303.4f–g). "May"
or "up to" allows none in that one choice; otherwise as many as possible are
chosen. A hand choice is private and asked whenever the hand could hide a
match, even with none (CR 402.3). "At random" from the graveyard draws from the
seat's seeded stream at top level only. Random hand moves, counts of X or "any
number", computed qualities and other players' zones reject.

**Library placement and shuffle** can put a single announced permanent on the
bottom of its owner's library, on top (single creature), or shuffle it in; the
mandatory target may be declared in a separate sentence, and the shuffle and its
owner use that occurrence (CR 115.1, 601.2c, 608.2b). The shuffle still happens
if the owner's commander goes to the command zone instead, and an audited
follow-up lets the target's owner draw two. A spell can shuffle itself into its
owner's library as its final instruction, after its stack-departure
replacements, with the owner's normal shuffle stream (CR 400.3, 401.4, 608.2n,
701.24c, 903.9b). Tucked tokens and spell copies remain in the zone until SBAs
and are absent from card counts, the shuffle permutation and draws (CR 111.6–8,
121.1, 704.5d–e, 707.10).

A **private top-card selection** looks at a fixed positive prefix of the
controller's library, chooses one card for hand, and puts the remainder on the
bottom in a private order (CR 401.2, 401.4, 608.2d, 701.20e); the commander hand
replacement still applies. Filtered or optional selection, public top-card
reveals, random bottom ordering and other destinations are unsupported.

## Battlefield entry replacements

A permanent's own entry replacement sets the tapped state, chooses a linked
value or places counters before entry triggers and SBAs, on every move onto the
battlefield (CR 122.6, 614.1c, 614.12). Conditional tapped entry can count
permanents the incoming controller controls (current structural characteristics, excluding the
entrant) or that player's live opponents; simultaneous entry uses the common
prior battlefield and ordered entry sees earlier moves. The private
`ProspectiveEntry` query applies the ordinary layers to one entrant's incoming
controller, face, status and applied modifications, including the entrant's own
statics on itself, and uses prior occupancy for counts such as devotion. Cards
and unallocated tokens share it; it consumes no handles, history or timestamps
(CR 614.12, 700.5).

The entering controller chooses from the five colors or pinned CR 205.3m creature
types. A choice suspends the unwritten entry in the replacement continuation.
Only the incoming controller sees the request and the visible subject, and the
committed choice is public on that battlefield incarnation (CR 607.2d, 614.12).
A zone change starts a new choice. Linked continuous scopes can match the
chosen color or creature type, and a mana ability can produce the chosen color
or bind its spend permission to the chosen creature type.

Life-payment entry offers run through `begin_battlefield_entry`; a search asks
each payable entrant in order, counting life already committed, then commits one
simultaneous entry. The controller sees the entering card even from a hidden
zone. A forced tapped entry still offers a payable payment and enters tapped
either way (CR 614.12; the shockland ruling "If an effect puts this land onto
the battlefield tapped, you may pay 2 life, but it still enters tapped.", [Lorwyn Eclipsed release notes](https://magic.wizards.com/en/news/feature/lorwyn-eclipsed-release-notes)).
Payment and entry commit together; an unpayable offer is skipped.

An entry reveal ("you may reveal a [filter] card from your hand. If you don't,
this land enters tapped.", CR 701.20a) lowers to `RevealOrEnterTapped` over the
hidden-card filter vocabulary with the same continuations. The offer lists
Decline, the entrant as subject, then matching cards in the incoming
controller's hand other than the entrant. A live controller is always asked,
with Decline alone when nothing matches, so the wait discloses nothing (CR
400.2). A search asks reveals after all payments. A reveal publishes the card's
identity in that hand (`known_searched_hand`) without exceeding the revealed
copies. Other entry costs (behold, multiple cards) reject.

**Quantity and entry replacements (E2 contract)** follow the shared
[pre-commit protocol](game-engine-mechanics.md#runtime-invariants). "If an
effect would create one or more tokens under your control" and "If [an effect
would put] one or more [kind] counters [would be put] on a [permanent/creature]
you control" lower to `StaticRule::ModifyQuantity { event, change }`: event
`CreateTokens` or `PutCounters` (one kind or any, permanents or creatures) with
an effect-only or any origin, and an add or multiply `QuantityModification`
(Hardened Scales, Doubling Season). Tokens only multiply. Any other event,
recipient or change, such as a subtraction, rejects as
`static:replacement:<verb>:<field>`.

An entering permanent is tested as it would exist under its incoming
controller against the battlefield before any simultaneous entrant (CR 614.12),
so a co-entering doubler never applies and an entrant's own quantity statics do
not see its own entry. Its "enters with" counters, intrinsic loyalty and
ordinary Saga lore are contained counter members produced by a replacement
(CR 122.6, 306.5b, 714.3a), as is the megamorph counter (CR 614.1e). Printed
placement uses one positive quantity per counter kind, optionally multiplied by
a checked count of battlefield permanents or graveyard cards read from prior
occupancy for the incoming controller. The count reads structural (layer-4)
characteristics, so color filters reject; a card entering from the counted
graveyard counts itself, and "other" over a graveyard rejects because that card
is not the entrant (CR 400.7). Kicker-count and mana-spent multipliers need cast
facts and reject until supported. For a quantity-order question, the
entering card is the order subject unless it comes from a hidden zone or is a
token. Precombat-main lore stays a turn-based action (CR 714.3c) and is not
doubled. A loyalty cost under Doubling Season stays at its paid amount, while a
resolving proliferate of it doubles; a planeswalker enters with twice its
loyalty and a Saga with two lore counters, so chapters I and II each trigger.
From one +1/+1 counter placed by an effect, Scales then Season gives four and
Season then Scales three. A token creation multiplies each recipe, defender or
opponent (populate, myriad, encore) and stays one creation per invocation.
Counters on each of several targets are one placement whose members each ask
their own order.

## Keywords, attachments and temporary effects

The state-owned timestamp allocator records rules time independently of batch
and trace IDs (CR 613.7a–g, 613.7m–n). Ordered occurrences get separate times;
simultaneous sources share a time and keep the APNAP ranks fixed when assigned,
in every layer. A same-controller tie that would change the result rejects
(CR 613.7m). Becoming attached, transforming and turning face up refresh the
object's timestamp; resolved effects, entry haste grants and counter stacks keep
their own creation or latest placement time. Dependencies override timestamp
order (CR 613.8).

[Battlefield layers](../packages/game-engine/crates/core/src/engine/ability_layers.rs)
own static applicability and the interaction with resolved changes
(CR 611.3, 613.6–8). Granted triggers require the observer work in
[Stage 4](game-engine-rework.md#carried-forward-from-the-first-audits).

Vehicles retain printed power and toughness off the battlefield (CR 208.3) and
have them on the battlefield while creatures (CR 301.7). Crew adds artifact and
creature in layer 4 until cleanup, leaving printed stats subject to layer 7.
Attack eligibility uses the Vehicle's continuous control even if it began the
turn as an artifact.

An Aura, Equipment or self static can also set card types, creature subtypes
or basic land types, and a God's devotion condition can remove Creature
(CR 205.1a, 700.5); a filtered scope can set basic land types ("nonbasic
lands are Mountains"). Removing a card type removes its subtypes and P/T; a
permanent that stops being a creature leaves combat (CR 506.4), and one no
longer an Aura or Equipment becomes unattached (CR 704.5p). Layer 4 is ordered
across the battlefield: the earliest effect that depends on nothing outside its
own loop applies, then dependencies are re-evaluated (CR 613.8); an effect
depends on another that changes its existence, its recipients or whether each
of its changes takes effect on them. A basic land type set on a land removes
its rules-text abilities and their effects, the setting effect excepted, and
intrinsic mana follows the new types (CR 305.6-7). Layer 6 removes all, all
other or listed keywords, and "can't have" applies last. Printed abilities
precede any removal; grants and removals then apply by timestamp, so a later
grant survives, while an effect whose source loses its abilities never applies
unless it started in an earlier layer (CR 613.6, 613.8a). Without its card's
abilities a permanent has no triggered, activated, mana, static or
characteristic-defining ability, and an entering one lacks its own entry
replacements (CR 604.3a, 614.12). Resolving loss, color addition, conditional
removals and other type changes reject.

Changeling and a self "is every creature type" CDA are a printed characteristic
in every zone (CR 702.73a, 604.3), also on a token; every subtype check counts
it only while the object is a creature (CR 205.3d, 205.3m). An Equipment, Aura
or source static can make its recipient every creature type as an unfiltered
layer-4 change, applied in timestamp order with resolved animations, whose
creature-subtype replacement removes it (CR 613.3, 613.7). Scopes reaching
other zones, conditional grants and resolving grants or removals reject.

A printed `*`, `N+*` or `*+N` power or toughness needs one self CDA per starred
stat (`StaticEffect::DefinePowerToughness`, functioning zone `Any`; CR 208.2a,
604.3). Layer 7a sets the stat in every zone, before 7b setters and 7c anthems
and counters (CR 613.4a), whenever it is read; nothing locks it in, so SBAs see
a live zero toughness (CR 704.5f). Values: counts of permanents, graveyard cards
or whole hands; `Times`, `Sum` (the printed `N` is its one fixed term), greatest
mana value, devotion and `Distinct` card types (exhaustive over
`CardKind::CARD_TYPES`, CR 205.2a), basic land types or colors. They read
layers 1–5 of other objects, never P/T or abilities. Chosen players, life
totals, counters, event objects, exile and other values reject under
`static:cda-pt:*`; a star without a CDA rejects under `source.power` or
`source.toughness`. A static "+N/+M for each [filter]" is a layer-7c change over
the same counts, recounted whenever the stat is read (CR 608.2h, 613.4c); "other"
or "other than this" excludes the source, and a penalty ("-1/-1 for each")
rejects as `static:modifier:for-each:negative`. A lord scope may be a union whose
every alternative names a creature type ("Skeletons you control and other
Zombies you control").

Printed triggered keywords lower to the triggers their rules define, each
instance triggering separately; the keyword line must print each N, the source
keyword list must name each keyword, and X amounts reject. Prowess (CR 702.108)
and bushido N (702.45) need a creature card, afterlife N (702.135) and soulshift
N (702.46) a permanent card. Echo [fixed mana] is an upkeep sacrifice-unless-paid
trigger keyed on upkeeps begun since the permanent came under its controller's
control, also in last-known information (CR 603.4, 702.30a); other echo costs
reject. Combat keywords (CR 702.83, 702.91, 702.23, 702.25, 702.105, 702.116,
702.121, 702.130, 702.149) read the declaration: the attack witness records
whether its creature attacked alone, attacked with a greater-power creature and
attacked the player with the most life (read once attackers have tapped,
CR 508.1m), and the combat records its declared defenders. Exalted, battle cry,
rampage, melee, flanking (marker and trigger only together), dethrone,
training and afflict follow their rules; rampage and melee count on resolution.
Ward [cost] (CR 702.21a) on a permanent card is a trigger on becoming the target
of an opponent's spell or ability that counters that stack object, named by the
event rather than targeted (CR 115.1, 603.2), unless its controller pays one
fixed mana cost or "Pay N life"; other ward costs reject as `keyword:ward:cost`.
Annihilator N makes the defending player sacrifice N permanents together, or all
if fewer (CR 702.86). Mentor compares both powers at placement and resolution,
using the mentor's last known power after it leaves (CR 702.134a–b, 608.2b);
"whenever ... mentors" events (CR 702.134c) reject. Myriad (a single-faced creature card
without entry replacements) offers a copy per other live opponent; each
accepted copy attacks that opponent or a planeswalker they control, entering
together tapped and attacking (CR 508.4, 608.2f) without having attacked, and
the invocation's delayed trigger exiles them at end of combat. Undying/persist,
extort and other triggered keywords reject.

Static guards can read the active player, public battlefield or graveyard
counts, and devotion (CR 700.5). Rule statics apply combat prohibitions and
attack requirements (CR 508.1c–d), countering prohibitions and flash timing.
Cost increases precede reductions, and excess colored or colorless reduction
reduces generic (CR 118.7a–d). Land allowances add plays per turn (CR 305.2);
hand-size effects use source timestamps at cleanup. Static control changes,
other characteristic-defining abilities and casting from additional zones
reject.

### Attachment entry and ordering

Resolving Auras enter attached to their exact legal target before entry
observers capture state (CR 608.3c), as one typed entry occurrence. Actual
attachments receive a checked monotonic timestamp that is canonical replay
state (CR 613.7e); reattaching to the same object changes nothing (CR 701.3b).
Grants never follow a successor incarnation; ordering never comes from object
IDs or map iteration; simultaneous competing attachments reject (CR 613.7m).

Equip announces one controlled creature at sorcery timing and moves the
Equipment if both incarnations remain legal. Enchant creature, artifact, land
and permanent use one predicate for targeting and ongoing legality; enchant
player rejects. Protection or an ineligible host sends an Aura to its owner's
graveyard and detaches Equipment (CR 704.5m–n). Attachment scopes apply keyword
grants, combat restrictions and signed stats to the current host. Attack and
combat-damage triggers match the host at the event; dies triggers read the
pre-event attachment (CR 603.10a).

Token creation supports fixed quantities, announced X and supported resolution
counts, including a fixed quantity per matched object; each instruction creates
one simultaneous batch (CR 111.1–4, 608.2h). Creature recipes preserve supported
printed keywords, secondary types and tapped entry. Treasure, Gold, Food and
Clue use their CR 111.10 templates through the normal mana and activation paths.
Restore rebuilds the template from the creating source and token identity.
Printed abilities, other predefined recipes, copy recipes in this path and
event-result counts reject.

### Keyword actions and counters

- Investigate creates a Clue (CR 701.16a). Populate copies a controlled
  creature token's copiable values; animation can qualify the original
  (CR 701.36, 707.2).
- Proliferate adds one counter of each existing kind to chosen permanents and
  players simultaneously, including loyalty on permanents and poison and
  energy on players (CR 701.34a). Ten poison counters lose (CR 704.5c).
- Permanent counters besides +1/+1 go on the same recipients (source, target
  creature, matching creature set) as one placement event, one atomic change
  per permanent. -1/-1 counters take fixed or announced X amounts and modify
  P/T in layer 7c; matching +1/+1 and -1/-1 counters annihilate in the
  state-based wave, skipping permanents that leave in it (CR 122.1a, 122.3,
  704.5q). Keyword counters for the reviewed temporary-grant keywords apply in
  layer 6 at the latest placement's timestamp (CR 122.1b, 613.7c). Stun
  counters and counters the pinned rules give no meaning may go on any targeted
  permanent; every untap event removes one stun counter instead (CR 122.1,
  122.1d, 614.1a). "[N for each] / X is the number of [kind] counters on this"
  reads the source's current or last-known counters. Printed placement of
  shield, loyalty, lore, level, time, fade, hone, other ±X/±Y and other
  rules-bearing kinds rejects as `verb:put-counters:counter_kind`.
- Fight deals current power as simultaneous noncombat damage between two legal
  battlefield creatures; self-fight deals double power (CR 701.14).

### Saga enchantments

Single-faced Saga enchantments earn `Executes` (`SagaV1`). Entry includes one
lore counter as a replacement (CR 122.6, 614.12, 714.3a). Before priority in the
precombat main phase, the active player simultaneously adds one lore counter to
each Saga they control with chapter abilities (CR 505.4, 714.3c). Lore is a
visible counter kind in placements, counts and proliferate; removing one lore
counter from an announced permanent lets that chapter trigger again later.

Each printed numeral is a separate triggered ability. A placement triggers every
crossed chapter, using its prior and resulting lore totals (CR 714.2b–c), with
ordinary APNAP placement, targets and last-known source facts. A Saga at or past
its highest current chapter number is sacrificed as an SBA once every triggered
chapter from that incarnation has left the stack, including chapters waiting for
placement; one with no current chapter abilities stays (CR 704.5s, 714.2d,
714.4). Read Ahead, Saga creatures, multiface Sagas, final-chapter trigger
events and unlowered chapter effects reject.

### Continuous permanent control

`GainControl` captures one battlefield target, the exact source or a matching
battlefield set at resolution (CR 611.2c) for the invocation's controller or an
announced player, until cleanup, indefinitely, or while the exact source
remains on the battlefield; that duration must hold at resolution and ends if
the source leaves or phases out (CR 611.2b, 702.26f). Durations depending on who
controls the source reject. Layer 2 applies active grants in creation order; a
later grant overrides an earlier one, which applies again when the later ends.
Each grant assigns a fixed player to its captured incarnations, so grants never
depend on each other (CR 613.8); with none left, the default controller applies.

Reconciliation finishes each occurrence or simultaneous group before
observers. An actual controller change removes combat membership and resets
readiness (CR 302.6, 506.4); counters, damage, tapping and attachments persist,
and paid invocations keep their frozen controller. With no living controller
left, CR 800.4a/c exile is immediate. Reconciliation never grants priority or
places triggers.

### Indestructible and deathtouch

Destruction prevention records a prevented event with no false movement.
Indestructible blocks lethal and deathtouch destruction SBAs but not damage,
lifelink, triggers, sacrifice, exile or zero toughness. Each completed SBA check
consumes the current deathtouch window, protected survivors included, so old
nonlethal deathtouch damage cannot kill after protection loss; fresh deathtouch
reopens a window. Simultaneous destruction consumes common-prior keyword facts,
so departing Equipment cannot remove protection mid-event.

### Face-down permanents

`TurnFaceUp` is a special action at priority (CR 116.2b, 702.37e). It pays the
morph cost as a resolution payment does and turns the same object face up
without entering; megamorph adds a +1/+1 counter (CR 702.37b, 708.8). Its
`TurnedFaceUp` source trigger fires; a noncreature leaves combat (CR 506.4).
An unpaid cost reverses its mana (CR 733.1). Counters may outlast face-down
creature status (CR 122.2).

Only the controller sees the identity (CR 708.5). In a decision offering a
face-down object, object candidates and subject slots follow zone, entry
timestamp and stack position, never card. A face-down hand departure consumes
known copies as a hidden one. Other zone changes and game end reveal the card
(CR 708.9); a departing owner's cards are revealed under each viewer's handle
(`PermittedView::revealed_face_down`). Snapshots reject face-down objects off
the stack and battlefield or without morph.

### Temporary keyword and set families

The [resolving effects evaluator](../packages/game-engine/crates/core/src/engine/kir/exec/continuous.rs)
owns checked captures, ordered characteristic changes and effect expiry.

- `RestrictCombat` is a rules effect, not a characteristic change (CR 613.11).
  "Can't attack", "can't block", "can't attack or block" and "can't be blocked"
  this turn bind one announced creature; "can't block" can also name all
  creatures or creatures without flying, read at each declaration. "Attacks
  this turn if able" and "can attack this turn as though it didn't have
  defender" bind one announced creature or the ability's source incarnation
  (CR 508.1d, 609.4); the latter is also a static rule.
- Printed protection from one or two fixed colors, or each color, lowers to
  separate per-color abilities on creatures matching the keyword line exactly
  (CR 702.16a, g, h); other qualities reject. Static grants of hexproof, shroud
  or indestructible may name noncreature recipients; other grants need a
  creature scope. Grant filters cannot test keywords, so layer 6 is
  order-independent.

## Trigger families and event witnesses

Triggers collect from actual typed occurrences, wait through SBAs and Commander
choices, then go on the stack in APNAP order with targets and order chosen at the
proper boundary. Controllers, actors, incarnations and witnesses freeze;
intervening conditions recheck at resolution; source status uses the current
incarnation or departure-refreshed LKI. Cast triggers collect after payment;
attack triggers use the real declaration batch. Generic departures never imply
dies triggers. Event player, source controller, invocation controller and
recipient never interchange.

A trigger condition is `TriggerCondition::Event { pattern, intervening_if }`
(`crates/contracts/src/kir/events.rs`). A `kir::EventPattern` names the event
and reads its object with a `kir::Pred` relative to the ability's controller;
`Atom::Is`/`Atom::IsNot` compare it with the source or its host (CR 303.4):

| Event | Object matched on |
| --- | --- |
| `Enters` | the resulting battlefield; a characteristic read retains the entrant as `Entered` evidence (CR 603.6a) |
| `Dies` | the battlefield just before the move, for any permanent, including "put into a graveyard from the battlefield" (CR 700.4, 603.10a) |
| `Attacks` | the attacker's facts before the declaration (CR 508.3a) |
| `AttacksUnblocked` | an attacker left unblocked at its defender's declaration (CR 509.3g, 802.4) |
| `Draws` | one successful draw and its player (CR 121.2, 603.2g) |
| `BecomesTarget` | the targeted permanent and announcing spell or ability, captured at choice and committed after costs. Trigger-generated matches wait for the next APNAP wave (CR 601.2c/i, 602.2b, 603.3d, 733.1) |
| `Cast` | the spell's frozen stack characteristics, with the caster in a player set and type or subtype predicates (CR 601.2i); `targets_source` requires the source among its targets as it became cast (heroic, CR 601.2c), and `target` some targeted permanent matching a predicate, frozen for recovery |
| `DealsDamage` | the source's battlefield appearance as positive damage is dealt to a player (CR 120.3, 510.2) |
| `StepBegins` | upkeep, draw, beginning and end of combat, end, precombat and postcombat main (CR 503-513) |
| `BecomesTapped` | only the source itself (CR 701.26a) |
| `TurnedFaceUp` | only the source itself, as it is turned face up (CR 702.37e, 708.8) |
| `GainsLife`, `Discards` | the gaining or discarding player; each source's positive gain is one event (CR 119.9, 701.9a). `Discards` reads the card where it was put (CR 603.10); `cycled` requires a discard paying its own cycling or typecycling cost, and "cycles or discards" is one discard (CR 702.29a, d, f) |
| `Sacrifices` | the sacrificing player and the permanent on the battlefield just before the move (CR 701.21a, 603.10a) |
| `LeavesBattlefield` | a move off the battlefield or a phased-in permanent leaving the game with its owner, read as for `Dies` (CR 603.6c, 603.10a); `without_dying` excludes a move to a graveyard (CR 700.4) |
| `Blocks`, `BecomesBlocked` | each declared blocker once, and each attacker unblocked until that declaration, before it (CR 509.3a, 509.3c) |
| `BlocksCreature`, `BlockedByCreature` | each declared blocking pair once from each side, both creatures as declared; an Aura or Equipment host can be the attacker (CR 509.3b, 509.3d, 509.3f) |
| `IsDealtDamage` | a creature dealt positive damage, once per simultaneous damage event (CR 120.3, 603.2c) |

Intervening comparisons can read this turn’s declared attacks, creature deaths
(any seat or under your control), departures under your control, spells cast
(all, noncreature, or from your hand), draws and total life gained. Departures
use the pre-move controller and types; artifact, creature and nonland entries
use the resulting battlefield types and controller, including token entries.
Creature entry history can also ask whether any opponent had an entry. Each
check reads the current turn from its start, including the triggering event
(CR 603.4, 603.10a).

Saga chapters match the positive lore-counter facet against its prior and
settled balance, including lore placed on entry; each chapter whose threshold
the change crosses triggers once, retaining that member's identity and facts
(CR 122.6, 714.2b). Saga chapters are the only admitted counter triggers.

A per-turn player ledger (spells as frozen stack facts, draws, life-gain
events, cycles) counts from the turn's start, whether or not the source was
present, and resets with the turn. A `TurnOrdinal` on cast, draw, life gain or
an unfiltered cycle triggers only on the Nth matching event of the event player,
or of every player for "the Nth spell of a turn"; a filtered ordinal counts only
spells that matched as cast. "This ability triggers only once each turn" records
a use per source incarnation, face and ability once it actually triggers (after
its intervening check), read live so simultaneous occurrences see it; "one or
more" under that limit lowers as the per-object form when nothing reads the
event. Witnesses carry the matched count; other ordinals, periods and limits
reject as `trigger:occurrence:*` or `trigger:limit:*` (`TriggerTurnHistoryV1`).

Same-kind alternatives (including one player's sacrifices) use one predicate.
Mixed entry/death, entry/attack, entry/departure, entry/cast, entry/life-gain,
attack/block, block/becomes-blocked and blocking-pair pairs use one ability
index without common event operands (CR 603.2c). "When you discard (or cycle)
this card" triggers from the graveyard the card was put into, under its owner's
control (CR 702.29c); it is the only non-battlefield observer, and no supported
replacement diverts a discard. "During your turn" requires the controller to be
active when the event occurs (CR 500.1); recovery rechecks it. Event filters
read frozen facts; cast and attack filters omit tapped status (CR 508.1f); host
exclusions reject. A bound entrant or attacker can take fixed counters or deal
directed damage, using its power when the effect resolves or from its last
battlefield appearance (CR 400.7, 608.2h). A source-relative combat-damage
trigger can put fixed +1/+1 counters on the incarnation that dealt the damage
(CR 400.7). Cascade has no executable R5 form.

`kir::Intervening::of` admits cast provenance, empty hand, source tapped
status, own commander control, a creature power threshold, signed total power of
your phased-in creatures and filtered battlefield counts including zero
matching permanents. Relative counts compare lands or creatures against each
opponent separately; “that player” instead binds the entering permanent’s
controller or the opponent whose step began, using the same opponent as the
event witness and counting phased-in permanents. Entry, attack and step triggers
can compare the controller's life, hand or own graveyard card count. Both stages
record `InterveningCheck` evidence (CR 603.4). These rows earn `Executes`
(`InterveningIfV1`, `EventTriggerV1`).

**Canonical event witnesses (E1–E3 contract).** The matcher reads the
`Created`, `Entered` and `Moved` facets, with at most one witness in those
families per member; delayed departure watchers read movement. Cast provenance
of a resolved permanent spell cross-checks the completion player against the
spell controller. The `AnyOf` branch audit ensures admitted alternatives
describe distinct occurrences. Restore rejects duplicate ordinary triggers by
source incarnation and face, ability, occurrence and subject (with a damage
member key where applicable) and repeated entry or death of one incarnation.
Collection deduplication by ability instance and `EventId` is E3 work.

Created and entered stay distinct: a resolving copy of a permanent spell yields
a move and a token entry with no creation (CR 111.13), so token-creation
replacements and created-token history ignore it while "token enters" triggers
see it. A cycled card is one discard member carrying both meanings; one "cycles
or discards" ability triggers once, and distinct abilities may each trigger
(CR 702.29d). A tap transition needs an untapped battlefield permanent and is
recorded with or without a listener. Entering tapped is entry status, never a
tap (CR 603.2e); vigilance, tapping a tapped object and a regeneration tap of a
tapped object produce no transition. A stun-replaced untap yields a stun counter
removal with lineage and no untap (CR 122.1d). Undo never creates a tap, untap,
life, counter or movement trigger.

### Delayed and reflexive triggers

The final instruction can create one trigger with the invocation's source and
controller (CR 603.7, 603.12).

| Actual occurrence | Admitted body |
| --- | --- |
| Next end step begins | Draw, create fixed tokens, or return the creature card exiled by the parent under its owner's control. |
| Next turn's upkeep begins | Controller draws fixed cards. |
| Controller's next upkeep begins | Controller draws or randomly discards a fixed count. |
| Chosen creature leaves | Return the card the parent exiled after its own death, under its owner's control. |
| Optional fixed mana payment completes | Announce one target for the separate reflexive ability. |

Captures bind exact incarnations (CR 603.7c). Watchers see actual departures,
including SBAs, replacement moves and concession exile (CR 603.6c, 800.4a);
mana undo restores watchers of reversed sacrifices. Phased-out sources use the
parent's last-known facts. Step watermarks and retired receipts prevent
duplicate firings. These one-shot rows earn `Executes` (`ResolutionTriggerV1`).

Two keyword rows extend this. Encore creates at once one copy of the card's
copiable values per living opponent; each has haste and, that turn, may attack
only that opponent and must if able, unless that player left or now controls it
(CR 508.1d); at the next end step its controller sacrifices the copies they
still control. A card whose own entry replacement would apply rejects (CR
614.1c). A resolved dash spell's permanent (or its copy's) has haste and returns
to its owner's hand at the next end step (CR 603.7, 702.109a, 702.141a).
Phased-out permanents are skipped (CR 702.26b).

### Printed choices, payments and conditions

Each printed "may", "if", "unless" and "up to" must match its AST carriers in
number and kind across the block and each certified sentence span, or the card
rejects as `printed_choice_limit_or_condition_disagrees_with_the_ast` (an "if"
read as "unless", a moved condition, "If you do" read as "if you don't"). A
trigger header followed by ", if" must carry that clause as its intervening
condition (CR 603.4).

Composition lowers the core steps through the verb lowerers and nests each
wrapped step range under one kernel control node:

| Printed | Kernel IR |
| --- | --- |
| "you may [step]", "you may [steps]" | `Optional` (CR 608.2d) |
| "[step] unless [player] pays fixed mana or life, or discards an artifact card" | `UnlessPays` (CR 118.12a) |
| "[player] may pay [cost]. If they do / don't, [steps]" | `IfPays` / `UnlessPays` (CR 118.12) |
| "if/unless you control a [filter]", "N or more [filter]", not/all of these | `If` over a battlefield count read on resolution (CR 608.2c) |

The optional chooser is the controller; a `decidedBy` chooser would need CR
800.4g reassignment and rejects, except a filtered target's controller choosing
to search their own library (CR 800.4a). A payer is the controller or the
trigger's event player; object payments are controller-only. An optional
sacrifice of one creature followed by a dependent clause shares the payment
continuation: the controller selects an eligible object or declines. A private
discard always offers decline, even with no eligible card (CR 400.2). An
optional body is a draw, life change, mill, search, shuffle, scry or surveil, a
tap, untap, move or counter verb on a target or the source, or a +1/+1 counter
verb on a matching battlefield creature set; a direct controller choice can
also create a fixed number of tokens under that player’s control (CR 111.2).
Impossible choices are not offered and are withdrawn if they become impossible
while pending (CR 608.2d): no object can still change or move, no live player, a
zero amount, or a library smaller than the mill (CR 701.17b). `IfPays` acts only
after payment; a departed payer pays nothing. A self-replacement that only
changes an action's amount selects one admitted branch before the action,
retaining its original announcement (CR 614.15). These earn `Executes`
(`Cell::Control`, `kernel_control_v1`). Everything else rejects under
`compose.control:<detail>`, including "if you do" after unsupported actions,
other conditions, nested leaves needing persistent provenance, and `forEach`
amounts (CR 121.2a).

### Modal spells and abilities

A printed "Choose one", "two", "one or both", "one or more", "up to one" or "any
number" over two to five bullets lowers to a `ModalSpell` declaration (CR
700.2): an instant or sorcery's only block, or a trigger's or activation's text
after its header. Counts and bullets must match the AST and certified spans and
every mode must lower, or the card rejects under `verb:modal:<detail>`. Each
legal set of distinct modes is its own program in printed order (CR 700.2d),
yielded by `spell_programs` and `programs`; the base program keeps timing only.

Spells and activations choose modes before costs, X and targets (CR 601.2b); a
trigger chooses with its targets and is removed if no mode can be chosen or it
chooses zero modes (CR 603.3c, 700.2b). Only selections with legal targets are
offered; a zero `mode_mask` is a chosen empty set. Spell selections can announce
independent object occurrences or a single counted target group. Illegal
targets suppress their own effects if another target remains legal (CR 608.2b).
These earn `Executes` (`modal_choice_v1`). Repeats, entwine, escalate, spree,
costed modes, upgrades, mixed target announcement routes and mana modes are
outside this subset.

### Resolution-time counts

"[N] for each [filter]" and a sentence's "where X is [number]" are read as the
step that prints them is performed (CR 107.3, 608.2h), after earlier steps of
the same program: `Amount::Count` over objects you or opponents control,
graveyard cards or (not for draws) your hand, `Times` for N above one,
`Greatest` power, toughness or mana value among permanents (never below zero,
CR 107.1b) and your devotion (CR 700.5). Draws, life changes, set or player
damage, token creation and P/T changes read them; token counts also read a life
total, an object's stat with last-known information (a destroyed target, a
cost's sacrificed object), "that many" and half of a number rounded as printed
(CR 107.1a), and an X binding never names a token its own reading step creates.
A P/T change keeps its value in the saved effect (CR 611.2c) and a counted draw
its recorded attempts. Several steps may re-read one X only across draws, life
changes and damage. A step that cannot read X, a hand count re-read, announced
or minimum quantities, event results ("this way"), another player's referents
and per-object iteration reject under `bindings:*` or `operand:bindings:*`.
