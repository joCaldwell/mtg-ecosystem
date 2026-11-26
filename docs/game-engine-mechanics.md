# Supported engine mechanics

Every executable card lowers completely from Archive R4 to schema-1 KIR.
`contracts/src/kir/tiers/executes.rs` validates the card shell and ability
context, `kir::check` the node and bindings, and `kir::tiers` derives the
[tiers](#derived-capability-tiers) the card earns. Match profiles independently
select decks and consumer features. Card admission requires a fresh
complete lowering and a matching catalog identity.

A spell can deal damage as its own source. Object operands that modify or put
counters on the source require a permanent ability. The controller can draw
a fixed count, one card at a time. Each-player draws finish one player's
attempts before moving to the next player in APNAP order (CR 121.2c).
An announced player draws one card per KIR instruction.

The rules baseline is the frozen [2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Use exact `tests/fixtures` records/provenance under `packages/game-engine`, never
card facts from memory or live refresh. The only executable schema is 1. No name/ID/profile dispatch:
card names below identify source witnesses, never mechanics.

<a id="source-identity-and-execution-boundaries"></a>
<a id="atomic-events-diagnostics-and-departures"></a>

## Runtime invariants

Compile complete exact archived source or reject; Rust contracts own executable
IR and [catalog/profile admission](game-engine-catalogs.md) is separate. Core
imports the IR and permitted-view contracts at runtime: no compiler/parser, I/O, clocks, OS randomness,
async inference or unsafe code. Keep physical IDs, zone incarnations, stack IDs,
owners, default control and explicit grants distinct; quantities are exact and
structural arithmetic checked. Frames/inputs/events/outcomes serialize as data;
clones are independent and restore proves accepted-input replay with exact identity.

Snapshot 85 stores RNG v2. Each random event starts at word zero of a
ChaCha20 stream keyed by the match seed. High bytes encode domain (1 library
shuffle, 2 affected-seat effect, 3 reserved table) and owning seat (zero for
table). The low 48 bits encode that domain and seat's event ordinal. Opening
shuffles, mulligans and resolving shuffles use the same library counter.
Opening rejection sampling consumes one word per rule step; a resolving
shuffle is one event drawing its Fisher-Yates words from its own stream. A
private board shuffles only with a scenario seed; otherwise a shuffle faults. A paired substitution
occupies the removed copy's physical slot while admission checks sorted rows.

The v2 trace chain hashes compact typed JSON in deterministic field order.
Stream tracing hashes the same committed evidence as Compact. Delivery is
local to a single executor and drains after each advance. Forks and restored
snapshots start with an empty delivery buffer. Every mode charges the same
retained payload. Pod analytics sample the choosing player's existing
observation and verify live seat facts against Detailed replay for proof groups.

Stage complete rule mutations atomically. Capture consumed pre-event facts and
actual typed outcomes. Simultaneous groups permit only independent read/write
footprints; unsupported composition faults. Freeze all loss candidates before
simultaneous departures/cleanup/adjudication. Restore must prove fault reachability.
Capacity/budget suspension never invents a win/draw, truncation or partial mutation.

An invocation has one owner: its stack entry, or `rules.detached` while a frame
still resolves it after the entry left the stack (CR 608.2h, 608.2m, 113.7a). That
owner holds the source's last-known facts. A frame names invocations by `StackId`
and holds progress only, so a rule step has no copy to reconcile.
`engine/frame.rs` traverses every frame, continuation, completion, owner and
origin variant with no wildcard arm; a new variant fails to compile there, in
`Frame::decider` (who decides) and in the publication classifier until it is
classified. A hand-cast permission names its issuer.

The committed `Game` lends its `RulesState` for reads. Each rule step or accepted
input runs on a copy-on-write stage whose `RulesTxn::rules_mut` grants exclusive
table access. `Game::transact` publishes each seat's knowledge before installing
the stage. Rule steps check history capacity at commit. Accepted inputs enter the
journal immediately and the next step enforces their capacity charge. Failed
transactions discard the stage. Internal `RuleError` carries invariant, resource
or illegal-input failures to the public error boundary.

Publication compares object chunks with the preceding revision to locate changed
cards. Within a layer epoch it reuses unaffected derived characteristics and
each viewer's eligible rows from that viewer's own history. Changes to shared
layer inputs invalidate the cache. Debug builds compare every publication and
its history charge against a full projection, check completed cached observations
against the budgeted builder, and validate each transaction from a valid state.

`Game::with_trace` selects Off/Compact/Stream/Detailed. Diagnostics enter neither semantic
hashes nor permitted views; `events()` populates only in Detailed, while `trace()`
is trusted evidence. Trace/budget choices cannot change rules behavior.

<a id="hosting-boundary"></a>

The host owns auth, HTTP, OS/SQLite, clocks and resources; Rust DTOs generate the
SDK. Derive viewers from authorization, retain bearer capabilities only in headers,
and never expose seeds/snapshots/receipts/raw IDs/sidecars/internal traces, another
seat's history or global private counters. Host faults remain distinct from rules
outcomes. [Hosting](game-engine-server.md) owns transport/recovery details.

## Execution, turns and queries

`advance(Budget)` suspends without changing rules meaning. Check a complete
response before accepting it; illegal responses are atomic.

Priority, announcements, payment and resolving frames are separate states. No
priority exists during announcement/resolution; retain the frozen invocation
through supported source departure. Land plays and ordinary creature timing require
active-player main priority and an empty stack; supported flash is explicit.
Cleanup discards to the active player’s current maximum hand size (CR 402.2,
514.1), then clears temporary effects and marked damage atomically. Concessions are
trusted recorded external inputs processed before work or queries. Land usage
resets each turn.

<a id="player-visibility-and-query-recovery"></a>

Under Protocol 2, the acting viewer’s `Observation` carries the `Decision` inline,
including priority actions and payment sources when offered. Its shape
(`Pick`, `Order`, `Distribute`, `Number` or `Pay`) defines legal answers.
Public clients submit an `Answer` using candidate indices local to that decision
and exact quantities where required. See the [decision controls](game-client.md#decision-controls).
The host checks the answer and submits the resulting opaque `ValidatedResponse`
receipt. Check receipts and recovery sidecars remain private and bound to the
executor and state. Raw IDs, `decision_for`, full-state getters and snapshots
are trusted host APIs. Public reads use existing viewer-permitted handles and
context. They do not publish history, mint handles, change rules or authorize
later submissions. Public revisions change only with permitted information.
Real restore rejects sampled origins. [Search](game-engine-search.md) separately
proves hypothetical origin and reconstruction.

## Modal double-faced cards

The layout allowlist admits Scryfall `normal` and `modal_dfc`. For an MDFC, Archive
R4 must supply two ordered source faces with matching parses and complete
resolution. Each face passes the ordinary shell and block checks independently
and the compiled card requires every capability of both faces. The card keeps
one physical identity and the frozen color identity across both faces
(CR 903.4d). Deck construction and commander eligibility use the front face's
characteristics (CR 712.8a, 903.3).

Each legal face is a separate priority action. Casting checks timing, targets
and costs for the selected spell face from hand or the command zone
(CR 712.11b–c, 903.8). A land selection uses the normal land-play allowance and
the chosen face's entry rules (CR 712.12). On the stack and battlefield the
selected face supplies all characteristics and abilities, including mana value
from that face's cost (CR 202.3, 712.8f). A resolving permanent spell keeps its
cast face (CR 712.13). A card otherwise entering the battlefield uses its front
face, and cards in hand, library, graveyard, exile or command use front
characteristics (CR 712.8a, 712.14). Zone changes still create fresh
incarnations. A stale ability source retains the face its incarnation had.

A viewer with permission to identify the card can see both face names and the
active face. Hidden hands and libraries expose neither face without existing
look or reveal permission (CR 712.6–7). A pending land-entry payment names the
chosen face and its life cost in the acting player's decision context, including
after restore. The card in hand keeps its front characteristics. Back-face
state is invalid for single-faced cards, outside stack or battlefield, on a
land spell, or on an instant or sorcery permanent.

Modal double-faced support is provisional at the `Executes` tier. Transform
and convert abilities, flashback on an MDFC, and other layouts including
transforming cards, Adventures, split cards, flip, meld and prototype remain
outside admission. The layout rejection identifies the archived layout.

<a id="commander-movement-and-hidden-libraries"></a>

## Commander and hidden information

Four-seat pod admission enforces 100 cards including commander, singleton name,
commander eligibility, color identity, basic-land mana and the selected frozen
ban policy, separately from executable support. Shuffles, mulligans and bottoming
are deterministic given trusted seed/input; seeds and library rows never enter
public streams. Open decklists are a chosen information policy.

Command-zone tax is physical and per caster, incremented only after a completed
command-origin cast; failed casts restore original zone/incarnation. Graveyard/
exile Commander return is an owner SBA choice after actual movement (CR 903.9a);
hand/library return replaces movement (CR 903.9b). Declining consumes that
incarnation's opportunity. Commander combat damage tracks physical source and
recipient across control and zone changes. A replaced draw of a tucked
commander completes once and preserves the next card. Commander-return decisions are
resumable continuations, never priority windows; drain concessions and
revalidate owner choices before exposing queries.

Library manipulation is private to its chooser and replay-proven. While a
search, scry or surveil choice waits, only its chooser sees the library cards
it looks at, a searcher its whole library (CR 701.23a); no other seat learns
an identity, count or order. Search candidates follow object order, and
view rows follow handle order (minted in object order). Scry picks the bottom
cards, then orders the top and bottom groups; surveil picks the graveyard cards,
then orders the rest (CR 701.22a, 701.25a). Each asks only for orders of two or
more cards and commits one atomic event; positions other seats remembered in
that library are forgotten. Scry, surveil and mill (CR 701.17) clamp short
libraries; mill emits nothing for zero, while a positive scry or surveil of an
empty library is still an empty event (CR 701.22d, 701.25d). Optional shuffles consume no randomness when declined. Counted
draws make sequential unclamped attempts; each-player draws finish each seat in
turn order. Discards clamp to hand size, group choices stay private until one
simultaneous discard, and random discard samples without replacement. Library-top
events preserve order and cannot compose with a conflicting order write. These
all finish before Commander SBAs; departure never redirects attempts.

Fixed, unqualified discard can follow other spell or ability instructions.
The controller, one announced player or opponent, each player, or each
opponent chooses from their own hand (CR 701.9a-b, 101.4). The damaged
player remains available in a damage trigger. A spell can make its
announced player discard at random, including an announced X count.
Choices and intermediate random samples stay private. Simultaneous
discard publishes the cards only when the rules expose their destinations.


A private "look at the top N, then put them back in any order" asks the
controller to order the clamped prefix (CR 401.2, 701.20e). The order
decision shows the current prefix to that player, including a lone card.
The engine saves only the order and identities that player learned. A
permutation can invalidate a public position in the prefix while positions
beyond the prefix remain known. This top-level instruction saves the
looked-at prefix and its resulting permutation. Snapshot 94 records
the private choice and the engine validates the completed outcome before
replay. Observations use the existing order decision.

Ordinary spell sequences preserve the source order of eligible fixed library,
life, removal and land-allowance instructions across printed blocks. A library
decision resumes the same instruction before later instructions execute. Fixed
library steps also compose in supported self-entry and declared-attack
triggers, and in paid source-sacrifice activations. The stack invocation's
controller receives each look decision during resolution.

Optional blocks keep their decision at resolution (CR 603.5, 608.2d). When an
exile spell offers its target's controller a basic-land search, that player is
bound before exile. Acceptance starts a private search and always shuffles that
library, including a search that finds nothing (CR 701.23b, 701.24a). Declining
skips the search and shuffle. The same decision can guard fixed life gain in
admitted creature-entry and upkeep triggers.

Departures preserve original seating and Commander history; all simultaneous
losers leave before cleanup/adjudication. Never resurrect their objects/mana.
Surviving activators retain paid abilities when a borrowed source's owner leaves.
Personal effects for a departed invocation controller skip rather than redirect.

<a id="announcement-priority-and-payment"></a>

Typed nonbasic lands derive intrinsic mana. A land play commits its entry and
land-play usage atomically. Paid land-search continuations retain ownership,
privacy and recovery. Fetch activations atomically tap, pay life and sacrifice the source
before typed search; paid roots retain LKI through source-owner departure.

### Mana abilities

A mana ability (CR 605.1a) is a tap cost, optionally with a mana cost paid
through a nested activation (Signets) or a life payment (CR 119.4), and one
production: fixed symbols, any color, commander color, N mana of one color
chosen from a printed list (duals, tri-lands, "any one color"), or one mana of
a type the controller's or opponents' lands could produce. CR 106.7 is the
least fixed point over battlefield lands, each read relative to its own
controller from intrinsic and printed abilities, ignoring costs; it does not
depend on object order. If a departure leaves no defined type while the color
is being chosen, the paid ability adds no mana. A source with an intrinsic
and a printed ability, or several printed ones, first asks which to activate
(CR 605.3a). After adding mana the ability performs its Kernel riders in
order: damage to its controller through the ordinary damage transaction, or
life gain (CR 605.3b, 608.2c). A nested activation's payment rechecks every
outer cost before the spend decision.

Reversal (CR 733.1) restores receipt-recorded mana, taps and counters. An
activation that paid life or performed riders is kept, and with it every
activation of that attempt. Spend-restricted mana has no producer and no spend
answer, so payment never counts or spends it (CR 106.6).

## Casting, costs, targets and stack

Fixed mana-producing spells and exact per-permanent mana multipliers use the
ordinary stack. Casting pays all costs before resolution. The resolving spell
counts matching permanents once and credits its controller through a staged
mana event. A zero count leaves the mana pool untouched.

Cast observers can match a spell subtype or a disjunction of subtypes against
its frozen paid cast event.

A library play window follows the face-up exile successors of one exact
resolution. The beneficiary uses ordinary priority, timing, costs, and land
allowance. Leaving exile ends that incarnation's permission. An undone cast
restores the original incarnation. A next-turn window expires in the cleanup
of the beneficiary's next actual turn (CR 400.7j, 514.2, 611.2a).

Fixed controller-relative stat modifiers reselect their recipients by live
controller and creature type in layer 7c. Signed modifiers and independently
verified intrinsic keyword blocks use the same layer from creature, artifact,
and enchantment sources (CR 611.3, 613.4c).

Stack projections run bottom to top. Saved announcement handles are historical
and never bind successor incarnations; reversal restores living players' passes.
Resolving frames retain frozen instances through supported source departure
(CR 608.2m). Only the caster answers targets and casting mana.

Targets precede costs. Casting always offers a mana window and cannot cancel a
payable spell; reversal refunds only receipt-recorded casting-time mana and taps.
Costs never guess colors. X is announced nonnegative and frozen. Base cost, X,
additional costs and increases compose in that order and freeze at CR 601.2f.
Additional costs never change mana value.

Activated abilities use a separate stack ID and historical source handle. Tap
costs enforce readiness and continuous control; mana, tap and sacrifice costs
retain separate guards, and failed payment reverses recorded mana activations.
Sacrifice costs use ordinary post-activation SBAs. Divided counters bind shares
before costs and commit surviving shares simultaneously.

Object target slots share one candidate set, so unequal per-slot filters reject.
Stack targets use public stack handles and exclude incomplete entries and the
targeting invocation. Spell filters check types, exclusions and exact stack mana
value, including announced X. An uncounterable spell remains a legal target
(CR 101.2, 701.6). Countered abilities disappear. Countered spells move to their
owners' graveyards, with flashback exile applied to stack departure (CR 702.34a).
Paid costs stay spent. A fixed generic counter tax offers the target spell's
controller payment at resolution (CR 118.12a).

Target legality is checked at proposal and resolution start. Loss of target
legality after a completed proposal during payment does not reverse casting
(CR 601.6), including departure before an accepted spending input is consumed. If
all targets are illegal at resolution, skip the entire program, including
compensation/draw. Target incarnation cannot rebind after bounce/reentry. Once
resolution begins, use its frozen legal set and refresh current/LKI facts only at
their defined boundaries.

Target-controller compensation reads the announced incarnation's controller
and supported attributes at those boundaries. A mandatory compensation step
still executes when a legal target survives destruction. Its recipient is
independent of the target's owner and any zone successor.

### Cost keywords and variable costs

A spell's printed cost may begin with one `{X}` and hold two-color hybrid or
single-color Phyrexian symbols. X is announced before targets and its value is
the spell's announced X (CR 107.3a, 601.2b). Each hybrid symbol is announced as
one listed alternative, and a Phyrexian symbol paid with life costs 2 life,
which a player with less than 2 life cannot pay (CR 107.4e-f, 119.4). Several X,
monocolored hybrid, colorless or hybrid Phyrexian symbols reject. Hybrid,
Phyrexian, X and special symbols in activation costs also reject.

Cycling is a hand activation on any card type: its fixed mana and the card's
own discard, then one draw (CR 702.29a). Typecycling pays the same cost, then
searches for one card of its type (basic land, or an allowlisted land or
creature subtype), reveals it, puts it into hand and always shuffles
(CR 702.29e-f). Kicker on an instant or sorcery is an
optional additional mana cost declared while casting. "If this spell was kicked"
is `Cond::CostPaid` on that declaration (CR 702.33a, d). Flashback on an instant
or sorcery without X or hybrid symbols casts the card from its owner's
graveyard at normal timing for the flashback cost instead of its mana cost
(CR 702.34a, 118.9). As it resolves, fails to resolve or is countered, the spell is
exiled instead. Abandoning the cast returns it to the graveyard (CR 733.1).
The public stack entry marks a flashback spell. Validation rejects a spell cast
from another player's hand or graveyard, or from a graveyard without flashback.
"As an additional cost to cast this spell, sacrifice a/an [quality]" is paid
after mana; a spell with nothing to sacrifice is not offered (CR 601.2b, 701.21a).
Other additional costs reject as `kind:additional-cost:<term>`. Ward waits for the
trigger wave.

An activation can also pay life and remove +1/+1 counters from its source, alone or with
tap, mana and source sacrifice. A player who cannot pay is not offered the
ability (CR 119.4, 122.1), and a cost without mana is paid as targets complete,
with no mana window (CR 601.2g). A chosen sacrifice can count several permanents,
any permanent, or "another" (never the source), and can also tap the source
after mana (CR 109.1, 602.2b, 701.21a). "[Mana], Exile this card from your
graveyard" functions only in its owner's graveyard: only that owner activates
it, targets precede mana, and the card is exiled after mana (CR 113.6j, 602.2a-b,
701.13a). The ability keeps its graveyard source through announcement and
exiles it on payment.

A battlefield activation can choose a fixed number of cards to discard from
its controller's hand, untapped permanents that player controls to tap, or
cards to exile from that player's graveyard (CR 601.2g–h, 602.2b, 701.9,
701.13, 701.26). The allowed qualities are any card or one card type for
discard, any permanent, one permanent type, one subtype or an artifact or
creature for tap, and any card, one type or one subtype for graveyard exile.
An explicit “another” excludes the source. These terms can combine with fixed
mana and the source's `{T}` symbol. A creature subject to summoning sickness
can pay a written tap cost, even for its own ability. Its own `{T}` symbol
still requires continuous control or haste, and one tap cannot pay both terms
(CR 107.5, 302.6).

Targets come first, followed by any mana window and then the activator's
`ChosenObjectCost` picks in ascending object order. A prefix offers only
objects that leave enough eligible choices to finish. The announcement commits
mana, the source tap and the entire chosen set in one rule step. Discard
candidates and the selected prefix remain private until the actual move to
the public graveyard (CR 400.2). Paid receipts bind the exact movements or
taps. A later untap preserves the proof that a tap paid the cost. The priority
menu requires enough eligible objects left after funding mana. It uses the
current pool and a bounded search over independent tap-mana abilities, reserving
the source for its own `{T}` cost. The proof excludes mana abilities that need
mana or life payments or have riders. The engine checks availability again
after the mana window. Random and whole-hand discard, variable counts,
other graveyards, self-exile from hand and chosen-object mana abilities remain
outside these cells.

Printed restrictions are permission gates. "As a sorcery" is main-phase
timing; your turn, your upkeep, your turn before attackers, combat, end of
combat and not your turn are typed windows. "Only once each turn" records a
use per source incarnation and ability when the activation completes; a
reversal uses nothing. It survives control changes, clearing on a zone change
or new turn (CR 400.7, 602.5b-d). Validation rejects a use off the
battlefield, on an unlimited ability, or disagreeing with whether its stack
entry is paid. "Activate only if" is read once, as activation begins (CR
602.5, 605.3a): an audited `kir::ActivationCondition` counts permanents you
control, your graveyard or hand, or the source's facts. Offer and start share
it; stack entries and mana receipts keep its counts, which restore checks.
Simulator mana plans need public proof. Others reject. Each of these surfaces is its own
`Executes`-only cell (`XManaCost`, `HybridManaCost`, `Cycling`, `Typecycling`,
`Kicker`, `Flashback`, `SpellAdditionalSacrifice`, `ActivationPayLife`,
`ActivationRemoveCounters`, `SacrificeCostChoice`, `SacrificeCostTap`,
`SacrificeCostAnother`, `GraveyardExileSource`, `ActivationRestriction`).

### Target status and tap/untap

A tapped-status filter requires current tapped status; absence means no
restriction, not untapped. Power bounds use derived power at announcement and
resolution. Tap/untap spells and activations, tap-source untap or destruction,
cast-entry land untap and owner-hand return each accept only their complete
validated shape. Already-matching status emits no second event, combat membership
is preserved and source tap is paid first. Paid activations survive source
departure.

### Target and selection operands

Battlefield targets check control and fixed power, toughness or mana value at
announcement and resolution (CR 109.5, 601.2c, 608.2b). A certified "it" or
"that" can reuse the block's sole announced incarnation (CR 400.7, 608.2b–c).

One optional battlefield or own-graveyard target can bind independent
destroy, tap, untap, move, counter and end-of-turn leaves. Tap and untap also
support one `Pick` of up to a fixed two through seven distinct targets.
They affect legal survivors simultaneously (CR 115.3, 601.2c, 608.2b, 701.26).
Zero targets skips the targeted instruction. Mixed required and optional
occurrences and optional permanent spells reject.

`ChooseObject` returns one controlled permanent chosen at resolution
(CR 109.5, 400.3, 608.2c–d). The public `Pick` includes the eligible source.
The engine derives current candidates and validates the full saved offer.
Departures that change the offer invalidate pending answers (CR 800.4a).
Empty sets skip the move, and commander owners may choose the command zone
(CR 903.9b). The cell earns `Executes`. Hidden hand choices and non-targeted
multi-object selections reject.

"This [permanent]" names the current source for tap, untap and owner-zone
moves (CR 400.7). "Put a +1/+1 counter on each [creature filter]" applies
simultaneously to the current set (CR 122.6, 608.2h).

<a id="combat-damage-and-state-based-actions"></a>

## Combat, damage and life

Noncombat damage can read a proven invocation source and damage each opponent
simultaneously. Resolve its count once from a matching battlefield or
graveyard set, or from the controller's hand size. Life effects can use those
same counts. Only the size of a hand is available for counting (CR 400.2,
402.3, 608.2h).

Attackers, blockers and damage commit atomically. Readiness stamps continuous
control at turn entry and resets on control/incarnation change. Blockers have no
damage assignment order. Blocked and blocking status survive counterpart
departure until combat ends (CR 509.1g, 511.3). Skip blocker/damage steps only
when none attacked. First and double strike have distinct participant sets;
trample and deathtouch assignments, dealt damage and prevention remain distinct.

Validate simultaneous damage before life/history writes. Combat source
validation reads the captured creature type, including animation, and
keywords from resolved and static grants (CR 510.2, 613.1d,f). Assignment
stays distinct from current or latest-departure source facts governing damage-time
deathtouch, lifelink and beneficiary, including Equipment departure. Noncombat
damage retains exact source and invocation controller. Protection from a color
checks derived or last-known source colors for targeting, attachments, blocking
and prevention. Commander combat history uses actual damage; drains credit
committed simultaneous losses before SBAs.

Replacement/prevention applies once per effect/member, under affected-recipient
choices and APNAP ordering. Freeze the consumed prefix; later source departure
cannot undo it. Remaining eligibility rechecks current facts. Damage allocation,
prevention budgets, zero damage and actual outcomes remain exact. Unsupported
general replacement/layer interactions reject instead of approximating.

## Movement, sacrifice and board-wide effects

Indestructible takes precedence over destruction. A regeneration shield protects
one exact permanent until its next permitted destruction or cleanup. It taps
that permanent, clears damage and removes it from combat (CR 701.19, 514.2).
The affected controller orders competing replacements. A zone change ends that
incarnation's shield. Exile, bounce, sacrifice, zero toughness, legend moves and
departure remain distinct. Removal filters use all card types (CR 205.2b).
Positive type lists are alternatives, exclusions reject any match and sacrifice
subtype lists are conjunctive.

Compensation uses the exact target's current or last relevant public appearance,
not its successor; tokens have zero mana value. Entry returns target at trigger
placement; optional returns choose at resolution; no legal target removes the
trigger.

A single announced card from the controller's graveyard can return to that
player's hand. The target can use exact printed types, supertypes, subtypes,
colors, fixed characteristic comparisons, and audited combinations of those
terms (CR 202.3, 205, 404.1, 601.2c, 608.2b). A printed "may" offers a separate
acceptance at resolution (CR 603.5, 608.2d). A creature card can enter the
battlefield under the controller's control, tapped when printed
(CR 110.2a, 614.1c). These moves use the exact announced incarnation and
earn `Executes`. A commander bound for the hand still offers its owner
the command zone (CR 903.9b).

When a removal's next instruction refers to the controller of its sole target,
the saved appearance supplies that player after the target leaves. That player
can create the printed token recipe or gain life. A life amount equal to the
target's power uses the same appearance and treats negative power as zero
(CR 107.1b, 111.2, 608.2h). Set tap and untap read the matching battlefield
permanents as the instruction is performed and change their statuses
simultaneously (CR 701.26).

Sacrifice moves a controller's own permanent to its owner's graveyard and is not
destruction. Choose/pay sacrifices after targets; “another” excludes the source.
Fling-like effects use paid source-backed LKI power. Forced opponent sacrifices
choose in APNAP order with earlier choices visible, then move simultaneously; a
player with no candidate sacrifices nothing.

Board-wide destruction, exile, bounce and damage commit atomically. Temporary
P/T sets capture recipients at resolution. Permanent filters for these moves
combine structural types and supertypes, current colors and controllers, derived
subtypes, stats and keywords, status, combat and token terms (CR 608.2h).
Permanent mana value is printed (CR 202.3).
Forbidding regeneration stays limited to the nonland-permanent wave. Counts use typed outcomes, including exile-replaced but
excluding prevented destruction. Turn-long exile replacement covers only
creatures actually dealt its damage, so exile causes no death. Indestructible
prevents destruction before replacement; generic departures never imply dies
triggers. A search shows the searcher every card of a non-empty library and
offers those with the printed qualities; with no match it still asks,
offering none. With a quality it may find none (CR 701.23b); "a card" must
find as many as it can (CR 701.23d). A printed reveal happens once, in the
library, for any destination; a found commander bound for its owner's hand
then offers the command zone (CR 903.9b), and the found cards move together, onto
the battlefield (optionally tapped, CR 614.1c) under the searcher's control or
into the owner's hand or graveyard. Found cards are saved in object order. A
found Aura entering the battlefield enchants an object its controller chooses
among those already there that it could enchant, without targeting (CR 303.4f);
with none, including when the last host leaves while the choice is open or
before entry, it stays in the library (CR 303.4g), as does a found instant or
sorcery (CR 304.4, 307.4), which never enters the battlefield by any move. Revealed hand cards are a public minimum
count per owner and Oracle identity (CR 701.23e), never an incarnation: each
physical departure is counted once, a public one consuming one copy of its
identity and a hidden one one of each; a reversed cast restores the copy its
announcement consumed (CR 601.2i).

A search of the controller's library can find at most one card, set it
aside within the library while the rest shuffles, and put it on top
(CR 701.24b). The library shuffles even if no card was found. The chooser
knows the resulting top. If the source instructs a reveal, all players
learn the found card and its new position (CR 701.20a, 701.23e). The
published position carries no library object handle and lasts while its
position is known. Searches that split found cards among destinations
or order several found cards after shuffling still reject.


## Battlefield entry replacements

A permanent's own entry replacement sets the tapped state or adds a fixed
number of +1/+1 counters to the new incarnation before entry triggers and
state-based checks (CR 122.6, 614.1c, 614.12). Conditional tapped entry can
count permanents controlled by the incoming controller, using their current
structural characteristics, or count that controller's live opponents. The
entering permanent cannot contribute to its own count. Simultaneous entry
uses the common prior battlefield and ordered entry sees earlier moves.
Conditional tapped entry and entry counters apply to every move onto the
battlefield. A land play's life-payment offer runs through
`begin_battlefield_entry`; a search onto the battlefield asks each payable
found entrant in object order, counting life already committed, then commits
the payments and one simultaneous entry batch with a per-entrant override. The
offer binds its entrant as the decision's subject. A forced tapped entry still
offers a payable payment and enters tapped either way, and its controller sees
the entering card while deciding, even from a hidden zone (CR 614.1c, 614.12; the
shockland ruling "If an effect puts this land onto the battlefield tapped, you
may pay 2 life, but it still enters tapped.", [Lorwyn Eclipsed release
notes](https://magic.wizards.com/en/news/feature/lorwyn-eclipsed-release-notes)).
Each continuation suspends before mutation and commits the chosen payment and
entry together. It skips the offer when the controller
cannot pay (CR 119.4). A land play
still requires a land in hand.

## Keywords, attachments and temporary effects

Battlefield static abilities use KIR scopes, predicates, conditions and
characteristic changes. The first applicable layer selects recipients for all
changes of that effect (CR 611.3, 613.6). Layer 4 adds artifact or enchantment
types, layer 5 sets colors, layer 6 grants supported keywords, layer 7b sets
base power and toughness, and layer 7c applies signed modifiers and counters.
Earlier layers supply facts to later layers. Admission rejects scopes that
could depend on another change in the same layer, including creature-subtype
filters for layer 4 and color filters for layer 5 (CR 613.8). Ordered setters
apply by source or creation timestamp (CR 613.7a–b,d–e). Simultaneous competing setters require a
timestamp choice and reject explicitly (CR 613.7m).

Static guards can read the active player, public battlefield or graveyard
counts, and devotion (CR 102.1, 700.5). Live rules apply combat prohibitions and
eligible attack requirements (CR 508.1c–d, 509.1b), countering prohibitions,
and flash timing for otherwise playable spells. Spell cost increases precede
reductions. Generic reductions preserve colored and colorless requirements,
and excess colored or colorless reduction reduces generic mana (CR 118.7a–d,
601.2f). The spell locks those inputs after announcement. Active static land
allowances add to existing plays per turn (CR 305.2), and hand-size effects
use source timestamps at cleanup (CR 514.1, 613.11).

This surface admits the source’s unconditional tapped entry. Conditional
entry and entry choices belong to W2. Static control changes, characteristic-defining
abilities, ability removal, and permissions to cast from additional zones
require further admission. Paid invocations and resolved effects keep independent lifetimes.

### Attachment entry and ordering

Resolving admitted Auras enter already attached to their exact legal target,
before entry observers capture the state (CR 608.3c); one typed entry occurrence
owns movement and attachment. Actual attachments receive a checked monotonic
timestamp (CR 613.7e, 701.3c); reattaching to the same object changes nothing
(CR 701.3b), and unattachment or zone change clears attachment metadata. Target
movement never redirects a grant to a successor incarnation. Ordering never comes
from object IDs, map iteration or trace retention. Simultaneous competing
attachments need the unimplemented ordering of CR 613.7m and reject explicitly.
Timestamps and their allocator are canonical state and replay inputs.

Equip announces one creature its controller controls at sorcery timing. Its
KIR Attach instruction moves the Equipment if both incarnations remain legal.
Enchant creature, artifact, land and permanent use the same predicate for spell
targeting and ongoing attachment legality. Enchant player remains unsupported.
Shroud and hexproof restrict new targets. Protection or an ineligible host
sends an Aura to its owner's graveyard and detaches an Equipment (CR 704.5m/n).
Attachment scopes apply keyword grants, combat restrictions and signed stats
to the current host in their respective layers. Attack and combat-damage
triggers match the host at the event. Dies triggers read the pre-event
attachment (CR 603.10a). These attachment cells earn Executes.
Fixed-color protection follows the current creature and affects targeting,
blocking, damage prevention and attachment legality. Applied prevention keeps
the protection facts captured when the damage was processed.

Predefined Treasure, Food and Clue use their standard artifact token templates.
Creation stages the new objects together. The normal mana and activation paths
pay the printed token costs. Restore reconstructs the template from its creating
source and token identity, including after the token ceases to exist.

### Keyword actions and counters

- Investigate creates a Clue with its two-mana sacrifice-and-draw ability
  (CR 701.16a, 111.10f).
- Populate copies a controlled creature token's copiable values. Animation
  can qualify the original. Restore reconstructs the copy from retained token
  data (CR 701.36, 707.2).
- Proliferate chooses any number of permanents and players with counters,
  adding one per existing kind simultaneously. Supported kinds are permanent
  +1/+1 and player poison and energy. At least ten poison counters cause
  a loss (CR 701.34a, 704.5c).
- Fight uses current power for simultaneous noncombat damage. Both participants
  must be legal battlefield creatures. Self-fight deals double power (CR 701.14).

### Continuous permanent control

The `GainControl` cell captures one battlefield target, the exact source, or a
matching battlefield set when it resolves (CR 611.2c). The recipient is the
invocation's controller or one announced player. The grant lasts until
cleanup, indefinitely, or while the exact source remains on the battlefield.
The source duration must hold at resolution and ends if that source leaves
or phases out (CR 611.2b, 702.26f). Durations that depend on who controls the
source require a separate admission row.

Layer 2 applies the active grants in creation order. A later grant can
override an earlier one, which can apply again when the later grant ends.
Each admitted grant assigns a fixed player to its captured incarnations,
so there is no dependence on another layer-2 assignment (CR 613.1b, 613.7b,
613.8). If no grant remains, the default controller applies.

Reconciliation finishes each occurrence or simultaneous group before observers,
keeping common-prior damage/death facts frozen. An actual controller change
removes combat membership and resets readiness (CR 302.6, 506.4); a
same-controller change does neither. Counters, damage, tapping, ownership and
attachments persist. Paid invocations keep their frozen controller, and queries
never repair the cache. With no living controller left, CR 800.4a/c exile is
immediate through typed movement. Reconciliation never grants priority, places
triggers or replaces a continuation, and never resurrects departed owners.

### Indestructible and deathtouch

Destruction prevention records a prevented event with no false movement.
Indestructible blocks lethal and deathtouch destruction SBAs but not damage,
lifelink, triggers, sacrifice, exile or zero toughness. Completed SBA checks
consume the current deathtouch window for protected survivors too, atomically
before another check or priority, so old nonlethal deathtouch damage cannot kill
after protection loss; fresh deathtouch reopens a window. Simultaneous
destruction consumes common-prior keyword facts, so departing Equipment cannot
remove protection mid-event.

### Temporary keyword and set families

Intrinsic keywords participate in combat, targeting and state-based actions. A
temporary effect requires a checked KIR operation and runtime support for its
exact target, duration and modifier; set modifications use the KIR selector and
the executor's captured incarnations.

`SetCharacteristics` captures a battlefield target, the exact source or a
matching battlefield set and applies its fixed values through cleanup
(CR 400.7, 514.2, 611.2c, 613.6). Animation adds creature and optionally
artifact in layer 4, keeping the permanent's previous card types and
supertypes. A fixed creature-subtype list can add subtypes, or replace just
the creature subtypes where the printed wording requires it (CR 205.1b,
205.3m). The same effect can set colors in layer 5, grant supported keywords
in layer 6, and set base power and toughness in layer 7b. Setters for either
stat compete independently at their timestamps. Counters and modifiers
apply afterward in layer 7c. A +1/+1 counter stays on its permanent when
animation expires and modifies its stats while it is a creature (CR 122.1a,
122.2). Settled effects own one captured set and one creation timestamp
across all their layers.

The signed pump cell accepts fixed deltas and signed announced X on one
announced creature, or the source of an activated ability. It reads and
stores X at resolution (CR 107.3, 611.2d, 613.4c). Fixed pumps, untapping
and keyword grants can read a certified singular target from a preceding
control or untap instruction in the same block. Copy effects, ability
removal, chosen characteristics and computed pump amounts have separate
admission boundaries.

Until-end-of-turn grants add supported keyword abilities to one announced
creature or a captured battlefield set. The existing activated-source form
uses the same layer 6 rules. Grants follow their creation order, expire at
cleanup, and affect only the selected incarnations (CR 400.7, 611.2c, 613.1f,
613.7b). The newly admitted operand cells qualify for `Executes`.

`RestrictCombat` is a rules effect, not a characteristic change (CR 611.2c,
613.11). "Can't attack", "can't block", "can't attack or block" and "can't be
blocked" this turn bind one announced creature's resolved incarnation. "Can't
block" can also name all creatures, or creatures without flying. That class is
read at each declaration, so a creature that later arrives or loses flying is
covered. The effect expires in cleanup (CR 508.1c, 509.1b, 514.2). Other
recipients, durations and "that creature" pronouns reject by name.

Printed protection from one or two fixed colors, or from each color, lowers to
separate per-color abilities on creatures (CR 702.16a, g, h). The printed
keyword line must match those qualities exactly. Other qualities such as types,
subtypes, players or everything reject by name. Static grants of hexproof,
shroud or indestructible may name noncreature recipients (CR 702.11, 702.12,
702.18). Other granted keywords still need a creature scope. Grant filters
cannot test keywords and no ability removal is admitted. Layer 6 grants
therefore have no dependencies and give the same result in any timestamp order
(CR 613.7, 613.8).

<a id="abilities-targets-and-triggers"></a>

## Trigger families and event witnesses

The shared trigger pipeline operates on the events admitted by composition.

Collect triggers from actual typed occurrences, wait through SBAs/Commander
choices, then place APNAP, selecting targets/order at the proper boundary. Freeze
controllers, actors, incarnations and witnesses. Intervening conditions recheck
at resolution. Source
status uses current incarnation or departure-refreshed LKI. Cast triggers collect
after successful payment; declared attacks use the real declaration batch.

A triggered ability's condition is `TriggerCondition::Event { pattern,
intervening_if }` (`contracts/src/kir/events.rs`). A `kir::EventPattern` names
the event and reads its object with a `kir::Pred` relative to the ability's
controller. `Atom::Is` and `Atom::IsNot` compare the object with the source or
its host (CR 201.5, 301.5, 303.4):

| Event | Object matched on |
| --- | --- |
| `Enters` | the resulting battlefield; a characteristic read retains the entrant as `Entered` evidence (CR 603.6a) |
| `Dies` | the battlefield just before the move, for any permanent, including "put into a graveyard from the battlefield" (CR 700.4, 603.10a) |
| `Attacks` | the attacker's facts before the declaration (CR 508.3a) |
| `Cast` | the spell's frozen stack characteristics, with the caster in a player set (CR 601.2i) |
| `DealsDamage` | the source's battlefield appearance as positive damage is dealt to a player (CR 120.3, 510.2) |
| `StepBegins` | upkeep, draw, beginning and end of combat, end, precombat and postcombat main (CR 503-513) |
| `BecomesTapped` | only the source itself (CR 701.26a) |

Same-kind alternatives ("this creature or another Elf you control") form one
`AnyOf` predicate, so an occurrence triggers the ability once (CR 603.2).
Two mixed alternatives can name only the source's own entry and its own
death or declared attack. They share one ability index, and collection and
recovery select the branch from the typed occurrence (CR 603.2c, 603.6a,
603.10a, 508.3a). Recovery rejects duplicate instances for one occurrence
or a second entry or death for the same source incarnation and ability.
The attacker in the event witness supplies the controller of the source's
own attack trigger (CR 603.3a). Mixed alternatives bind no event object or
player operand.
Player operands are "you", each opponent or each player. The reviewed narrow
lowerers run first; the general event table lowers the remaining enters, dies,
put-into-graveyard and attacks events term by term through the board filter.
`kir::check_event` admits only atoms evaluable on event facts. Cast and attack patterns read no tapped status (CR 110.5d, 508.1f), and no pattern excludes the host, which a snapshot cannot prove. Recovery reads frozen facts as of the event, including a controller who has since left (CR 800.4a). Only combat damage needs a creature source (CR 510.1).

`kir::Intervening::of` admits an intervening `kir::Cond` in exactly one audited
form: "if you cast it", an empty hand, the source's tapped status, a creature
with power N or greater, or a count of permanents "you" control matching a
filter, possibly excluding the source. Entry, attack and beginning-of-step
triggers can also compare a fixed amount with the controller's life, hand
size, or own graveyard card count. The graveyard filter admits all cards,
one card type, or the supported permanent card types. Tokens are excluded
(CR 111.6, 404). Core checks the clause when the event
occurs and again as the ability resolves, recording `InterveningCheck`
evidence (CR 603.4). Snapshots save the comparison's measured quantity and
check its kind, controller, count bound, and result. A graveyard count's
bound includes only owned physical cards that match the printed filter.
These earn `Executes`
(`InterveningIfV1`, `EventTriggerV1`). An
opponent's creature dying freezes that creature's controller
(`triggers::binds_death_controller`). Event-object pronouns ("it", "that
creature") remain unaudited.

Intrinsic landwalk and fixed-color protection can share a card with separately
proved entry effects and fixed paid creature activations. Entry triggers on
artifacts, Equipment and Auras use the entering incarnation. Supported entry
blocks can draw, change the controller's life, deal targeted damage or destroy
a target. A one-creature sacrifice for each player collects choices in APNAP
order and moves the chosen creatures together.

Event-player, source controller, invocation controller and recipient cannot be
interchanged. Optional “may” is an actual decision; unsupported source
explanation blocks a browser control. Compiler presence does not expand
host/search allowlists.

The per-turn cast ledger records completed casts. The retired cascade instruction
and its reveal-and-cast continuation have no executable R4 form.

A source-relative combat-damage trigger can place fixed +1/+1 counters on
the creature that dealt the damage. The instruction retains that battlefield
incarnation, including when the source also has supported intrinsic keywords
(CR 400.7, 603.3a, 608.2h). A returning card cannot receive the old trigger's
counters.

### Delayed and reflexive triggers

The final instruction can create one trigger with the invocation’s source
and controller (CR 603.7, 603.12).

| Actual occurrence | Admitted body |
| --- | --- |
| Next end step begins | Draw, create fixed tokens, or return the creature card exiled by the parent under its owner’s control. |
| Next turn’s upkeep begins | Controller draws fixed cards. |
| Controller’s next upkeep begins | Controller draws or randomly discards a fixed count. |
| Chosen creature leaves | Return the card the parent exiled after its own death, under its owner’s control. |
| Optional fixed mana payment completes | Announce one target for the separate reflexive ability. |

Step transitions govern timing (CR 513.2). Captures bind exact incarnations
(CR 400.7, 603.7c). Watchers see actual departures including SBAs, replacement
moves and concession exile (CR 603.6c, 614.6, 800.4a/c). Mana undo restores
watchers of reversed sacrifices (CR 733.1). Phased-out sources use the
parent’s last-known facts (CR 608.2h, 702.26b). Stabilization and APNAP
placement precede priority (CR 117.5, 603.3b–d).

Recovery checks creator, controller, capture and firing evidence. Step
watermarks and retired receipts prevent duplicate firings. These one-shot rows
earn `Executes` under `ResolutionTriggerV1`.
[Pinned Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)

### Printed choices, payments and conditions

Each printed "may", "if", "unless" and "up to" of a block must match its AST
carriers in number and kind. The check covers every certified sentence span of
the R4 resolution and the whole block. Carriers are: an optional step or group, a
permission, a modal repeat, a condition of that kind, a countered or created
replacement, an "if able" requirement, or a zero-minimum count. In a static
block a kindless condition prints as "if", "as long as" or "during". A block
whose counts differ, including an "if" read as "unless", rejects as
`printed_choice_limit_or_condition_disagrees_with_the_ast`. So does a condition
moved to another sentence, or an "If you do" sentence read as "if you don't"
(CR 118.12). A trigger whose certified header span is followed by ", if" must
carry that clause as its intervening condition (CR 603.4). A header that
certifies `/trigger` with `/condition` already printed it as one. Composition peels control off ordinary steps and lowers
the core through the verb lowerers. It then nests each wrapped step range
under one kernel control node:

| Printed | Kernel IR |
| --- | --- |
| "you may [step]", "you may [steps]" | `Optional` (CR 608.2d) |
| "[step] unless [player] pays {N} / N life" | `UnlessPays` (CR 118.12a) |
| "[player] may pay [cost]. If they do / don't, [steps]" | `IfPays` / `UnlessPays` (CR 118.12) |
| "if/unless you control a [filter]", "N or more [filter]", not/all of these | `If` over a battlefield count read on resolution (CR 608.2c) |

An optional chooser is the controller; a chooser named by `decidedBy` would
need CR 800.4g reassignment and rejects. A filtered target's captured controller
may choose only to search and shuffle their own library, in the compiler and
the Kernel IR check alike: it leaves with them (CR 800.4a), so a reassigned
choice could act on nothing. A payer binds to the controller or the
trigger's kernel event player, and a cost's implicit payer must be the same
player. An optional body is a draw, life change, mill, search, shuffle, scry or
surveil, or a tap, untap, move or counter verb on a target or the source. The continuation offers no impossible
choice (CR 608.2d):
- no object that can still change (CR 701.26a) or move (CR 400.7)
- no live player or a zero amount
- a library smaller than the mill amount (CR 701.17b)

A pending choice that becomes impossible while it waits is withdrawn (CR
800.4a). Nested equal-range wrappers keep
printed ancestry. `IfPays` performs its node only after payment. A payer who has
left the game pays nothing. State validation checks a suspended chooser, payer, life
amount and generic mana amount against the admitted node. A destroy step keeps its own optional
row. These cards earn `Executes` only (`Cell::Control`, `kernel_control_v1`).
Everything else rejects under `compose.control:<detail>`:
- "if you do" after an action other than a payment
- another condition, a nested leaf that needs persistent provenance
- `forEach` amounts (CR 121.2a: a counted draw is one replaceable event)

### Modal spells and abilities

A printed "Choose one", "two", "one or both" or "one or more" over two to five
bullets lowers to a `ModalSpell` declaration (CR 700.2): an instant or
sorcery's only block, or a trigger's or activation's text after its header.
Counts and bullets must match the AST and certified spans. Every mode lowers
(an ability's as a whole ability of its block, agreeing on all but its program),
or the card rejects under `verb:modal:<detail>`. The base program keeps timing
only; each legal set of distinct modes is its own program in printed order (CR
608.2c, 700.2d), yielded by `spell_programs` and `programs`.

A spell or activation answers `Reason::ChooseModes` in `Frame::ChooseModes`
before costs, X and targets (CR 601.2b, 602.2b), and with nothing offerable
cannot be proposed. A trigger chooses in `Frame::TriggerTargets` and with
nothing offerable leaves the stack (CR 603.3c). Only selections with legal
targets are offered; an empty decision faults. The public `mode_mask` selects
the program. A selection targets at most one battlefield, own-graveyard,
any-damage or player object. State validation rejects a stray or unbound mask.
These earn `Executes` (`modal_choice_v1`).

"Up to one", "any number", repeats, entwine, escalate, spree, costed modes
(CR 700.2h), upgrades, other text in the block, two-target selections and modes
that add mana (CR 605.1a) reject.

"[One] for each [filter]" on a draw or life step is `Amount::Count`, counted
once (CR 608.2h), over objects you or opponents control, graveyard cards, or
(life only) your hand. A counted draw (`DrawControllerPerObject`) reads a
battlefield or graveyard set, which drawing cannot change. Other amounts,
step-result filters and another player's referents reject.

## Derived capability tiers

Every compiled card is lowered one printed block at a time. Its definition
carries `composed: true` and `composed_ability_v1`. Rust checks the complete
capability set and rejects a surface with no executor. The schema describes the
structural form of the single `Kernel` instruction.

A definition consists of cells: the audited kernel cells `kir::check` names
(`kir::Cell`, one `(verb, operand kinds, context)` row each) and the reviewed
non-kernel surfaces named by their capabilities. Each cell carries the tiers it
has earned; a definition earns a tier only when every cell has (the meet). The
tables are `kir::cell_tiers` and `kir::surface_tiers`; no card name, Oracle ID
or profile appears in them, and a newly opened cell starts at `Executes`.

| Tier | Earned when | Admits to |
| --- | --- | --- |
| `Executes` | the cell has an executor and a rules test | dynamic headless pods; a runtime fault is the pod's `RulesFault` |
| `HeadlessQualified` | it ran in the pod parity corpus with state validation and snapshot round trips | `--analytics-out` and paired runs |
| `Search` | its hidden-information behaviour is reviewed for the sampler | profiles requesting search |
| `Hosting` | every decision it raises has source labels and a client control | profiles requesting hosting |

The three reviews above `Executes` are independent: a hosting review says
nothing about hidden information. A rejected hosting or search request names
the capabilities (`UnreviewedCapabilities`) or kernel cells (`UnqualifiedCells`)
below its tier. On all 2,370 Archive R4 definitions and every fixture catalog
the derived decisions equal the capability lists they replaced.

`Executes` is decided per surface, each row independent (CR 113.3, 602.1, 603.1,
608.2); a surface without a row rejects under `cell:<surface>:<detail>`:

| Surface | Executes |
| --- | --- |
| Instruction × stack object | A checked KIR node with operands bound in its spell, activated or triggered context. Unsupported operands and verbs reject. |
| Target route × stack object | Battlefield permanents/creatures and any-damage targets everywhere; players and own-graveyard cards for spells and triggers; stack targets for spells and activations. |
| Cost × route | Tap, mana, source sacrifice, chosen sacrifices of a type, type union, subtype or any permanent (optionally several, "another", or with a source tap), tap/mana/sacrifice/life/+1/+1-counter combinations, untargeted tap, pay life and sacrifice the source, chosen discard/tap/graveyard exile, the cycling and typecycling hand discard, and a graveyard source exiled after mana, only where that announcement route pays the complete cost (CR 602.2b). |
| Event | An admitted `kir::EventPattern` on a battlefield permanent, with at most one audited intervening condition. Reviewed patterns keep their capabilities; any other uses `EventTriggerV1`. |
| Static, mana, keyword | Own entry tapped, scoped continuous changes and rule statics as above; the [mana abilities](#mana-abilities) above, whose chosen, could-produce, mana-cost, life-cost, multi-ability and rider shapes are `Executes` cells (`kir::mana_ability_cells`); evergreen keywords without parameters. |
| Kernel node | A [kernel node](game-engine-catalogs.md#kernel-ir) with checked children, bindings and an executor for its stack context. |
| Card shell | Normal cards and [modal double-faced cards](#modal-double-faced-cards) whose faces have complete bound source and supported spell costs and keywords. |

Sentences resolve in printed order (CR 608.2c). A pronoun must be bound by the
archive to a target of its own block or to an earlier step of that block;
`its controller` and `its power` require the sole announced target (CR 608.2h).
Sorcery-speed activations use main-phase announcement timing (CR 602.5d).
Tokens recover from the creating recipe of a spell, activation or trigger.
The R4 source and binding checks are described under
[compositional lowering](game-engine-catalogs.md#compositional-lowering).

## Admission and policy boundaries

The Foundations profile supplies the retained hosted and search decklists.
Other R4 fixture catalogs exercise private rules and compiler tests.

Develop policy uses permitted observations and public IR features. It may
estimate value but never determines legality or inspects hidden state; new
behaviors need their own public-policy witnesses. Core and policy share the
`mtg_core::query` predicates for permanent, sacrifice and stack-spell filters and
board-wide sets: core reads derived state through a private adapter, a policy
reads only `VisibleObject` and `PermittedView`, and an unpublished fact answers
`Unknown`. The module also states each target domain's inhabitants for policies;
core's own target legality, with hexproof, shroud, protection and per-ability
restrictions, remains the authority. Paid accepted-pod prefixes,
private board setups, synthetic IR controls and fixed-index completion games stay
separately labeled in [evidence](game-engine-evidence.md), which owns producers,
proof locations and omissions; [catalogs](game-engine-catalogs.md) own
reproduction.
