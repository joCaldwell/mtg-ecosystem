# Supported engine mechanics

Every executable card lowers completely from Archive R5 to schema-1 KIR.
`crates/contracts/src/kir/tiers/executes.rs` validates the card shell and
ability context, `kir::check` the nodes and bindings, and `kir::tiers` derives
the [tiers](#derived-capability-tiers) the card earns. Admission requires a
fresh complete lowering and a matching catalog identity; match profiles
independently select decks and consumer features. Every shape outside the
contracts below rejects explicitly with a named detail.

The rules baseline is the frozen [2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Use exact `tests/fixtures` records/provenance under `packages/game-engine`,
never card facts from memory or live refresh. No name/ID/profile dispatch: card
names identify source witnesses, never mechanics.

## Runtime invariants

Compile complete exact archived source or reject; Rust contracts own executable
IR and [catalog/profile admission](game-engine-catalogs.md) is separate. Core
imports only the IR and permitted-view contracts: no compiler/parser, I/O,
clocks, OS randomness, async inference or unsafe code. Keep physical IDs, zone
incarnations, stack IDs, owners, default control and explicit grants distinct;
quantities are exact and structural arithmetic checked. Frames, inputs, events
and outcomes serialize as data; clones are independent and restore proves
accepted-input replay with exact identity.

- **Randomness.** Each shuffle or seat-local random event draws from its own
  ChaCha20 stream keyed by the match seed, domain, seat and ordinal
  (`engine/setup.rs`). A private board shuffles only with a scenario seed. A
  paired substitution occupies the removed copy's physical slot.
- **Tracing.** `Game::with_trace` selects Off/Compact/Stream/Detailed; every
  mode charges the same retained payload and none changes rules behavior.
  Diagnostics enter neither semantic hashes nor permitted views; `events()`
  populates only in Detailed and `trace()` is trusted evidence.
- **Atomicity.** Stage complete rule mutations atomically, capturing consumed
  pre-event facts and actual typed outcomes. Simultaneous groups permit only
  independent read/write footprints; unsupported composition faults. Freeze all
  loss candidates before simultaneous departures, cleanup and adjudication.
  Restore proves fault reachability. Capacity/budget suspension never invents a
  win/draw, truncation or partial mutation.
- **Invocations.** An invocation has one owner holding the source's last-known
  facts: its stack entry, or `rules.detached` while a frame still resolves it
  (CR 608.2m, 113.7a). Frames name invocations by `StackId` and hold progress
  only; `engine/frame.rs` matches every variant without a wildcard.
- **Transactions.** Each rule step or accepted input runs on a copy-on-write
  `RulesTxn` stage; `Game::transact` publishes each seat's knowledge before
  installing it, and failure discards it. Debug builds check every cached
  publication, history charge and transaction against a full projection.
- **Hosting boundary.** The host owns auth, HTTP, OS/SQLite, clocks and
  resources; Rust DTOs generate the SDK. Viewers derive from authorization;
  seeds, snapshots, receipts, raw IDs, sidecars, internal traces, another seat's
  history and global private counters are never exposed. Host faults stay
  distinct from rules outcomes; [hosting](game-engine-server.md) owns transport.

## Execution, turns and queries

`advance(Budget)` suspends without changing rules meaning. A complete response
is checked before acceptance; illegal responses are atomic. Priority,
announcement, payment and resolving frames are separate states, with no priority
during announcement or resolution. Land plays and ordinary creature timing need
active-player main-phase priority and an empty stack; supported flash is
explicit. Land usage resets each turn. Cleanup discards to the active player's
current maximum hand size, then clears temporary effects and marked damage
atomically (CR 402.2, 514.1). Concessions are trusted recorded inputs processed
before work or queries.

Under Protocol 2 the acting viewer's `Observation` carries the `Decision`
inline; its shape (`Pick`, `Order`, `Distribute`, `Number` or `Pay`) defines
legal answers ([decision controls](game-client.md#decision-controls)). Clients
submit an `Answer` with decision-local candidate indices; the host checks it and
submits the opaque `ValidatedResponse`, whose receipts stay private and bound to
executor and state. Raw IDs, `decision_for`, full-state getters and snapshots
are trusted host APIs. Public reads mint no handles and publish no history, and
public revisions change only with permitted information. Real restore rejects
sampled origins ([search](game-engine-search.md) owns hypothetical origins).

<a id="modal-double-faced-cards"></a>

## Double-faced cards and adventurers

The layout allowlist admits Scryfall `normal`, `modal_dfc`, `transform` and
`adventure`. Both Archive R5 parts must lower completely; the card requires
both parts' capabilities and keeps one frozen color identity (CR 903.4d).
Deck construction, commander eligibility and zones other than the stack or
battlefield use the front or normal characteristics. Every zone change
creates a fresh front-face incarnation (CR 400.7, 712.8a, 712.14, 715.4).

A modal card offers each legal face as its own cast or land play, with that
face's timing, targets, costs and entry rules; the cast face stays up
(CR 712.11b–c, 712.12–13, 202.3).

A transforming card is cast and enters front face up. Its own activated or
triggered ability, plain, optional or conditional, turns the same object
over: effects, counters, damage and status stay (CR 701.27a, 712.18). It
transforms only if it has not transformed since the ability was stacked or
the delayed trigger created (CR 701.27f). Abilities, delayed triggers and
activation limits keep the face they came from (CR 113.7a). The back face's
mana value uses the front cost (CR 202.3b). Both faces must be permanents,
both creatures or neither, with no transform-event trigger.

An adventurer may be cast as its Adventure, which has only the inset spell's
characteristics on the stack (CR 715.3a–b). A resolving Adventure is exiled;
its controller may cast or play the card normally from exile until it leaves
(CR 715.3d). A countered or fizzled Adventure goes to the graveyard.

Identifying viewers see the layout, part names, active face and adventure
exile; hidden zones show neither part (CR 712.6–7). Decision context names
the chosen part after restore. Snapshots reject a face, transform count,
permission, ability face or baseline the card or zone cannot have.

Support is provisional at `Executes`. Split, flip, meld, prototype, Omen and
Room layouts, battle and planeswalker faces, daybound, disturb, craft,
casting or entering transformed and two-part flashback stay rejected.


## Commander and hidden information

Four-seat pod admission enforces 100 cards including commander, singleton names,
commander eligibility, color identity, basic-land mana and the selected frozen
ban policy, independently of executable support. Shuffles, mulligans and
bottoming are deterministic from trusted seed and input; seeds and library rows
never enter public streams. Open decklists are a chosen information policy.

Command tax is physical and per caster, incremented only after a completed
command-origin cast; failed casts restore zone and incarnation. Graveyard/exile
return is an owner SBA choice after actual movement (CR 903.9a); hand/library
return replaces movement (CR 903.9b); declining consumes that incarnation's
opportunity. These are resumable continuations, never priority windows;
concessions drain and owner choices revalidate before queries. Commander combat
damage tracks physical source and recipient across control and zone changes. A
replaced draw of a tucked commander completes once and preserves the next card.

**Library choices** are private to their chooser and replay-proven: only the
chooser sees the cards looked at (a searcher its whole library, CR 701.23a).
Other seats see only the identities and counts that a printed reveal makes
public, without the library order. Scry picks bottom cards, then
orders both groups; surveil picks graveyard cards, then orders the rest (CR
701.22a, 701.25a); orders are asked only for two or more cards. Each
manipulation is one atomic event, and other seats forget remembered positions in
that library. Scry, surveil and mill clamp short libraries; mill of zero emits
nothing, while positive scry or surveil of an empty library emits an empty event
(CR 701.17, 701.22d, 701.25d). Declined optional shuffles consume no randomness.
Library-top events preserve order and cannot compose with a conflicting order
write. "Look at the top N, then put them back in any order" orders the clamped
prefix, even one card (CR 701.20e); positions beyond it stay public. All of these
finish before Commander SBAs; departure never redirects attempts.

**Draws and discards.** Counted draws are sequential unclamped attempts, one
card per KIR instruction for an announced player; each-player draws finish one
player before the next in APNAP order (CR 121.2c). Fixed, unqualified discard
can follow other instructions: the controller, an announced player or opponent,
each player or each opponent chooses from their own hand (CR 701.9a–b, 101.4),
and a damage trigger can name the damaged player. A spell can make its announced
player discard at random, including announced X, sampling without replacement.
Discards clamp to hand size; choices and samples stay private until one
simultaneous discard.

Spells keep the printed order of fixed library, life, removal and
land-allowance instructions across blocks; fixed library steps also compose in
self-entry and attack triggers and paid source-sacrifice activations. Optional
blocks decide at resolution (CR 603.5, 608.2d). An exile spell's basic-land
search for its target's controller binds that player before exile; acceptance
searches privately and always shuffles (CR 701.24a); declining skips both.

Departures preserve seating and Commander history; simultaneous losers all leave
before cleanup and adjudication, and their objects and mana never return.
Surviving activators keep paid abilities when a borrowed source's owner leaves;
personal effects for a departed invocation controller skip rather than redirect.

Typed nonbasic lands derive intrinsic mana. Fetch activations atomically tap,
pay life and sacrifice the source before the typed search, which keeps LKI
through source-owner departure.

### Mana abilities

A mana ability (CR 605.1a) is a tap cost, optionally with a mana cost paid
through a nested activation or a life payment (CR 119.4), and one production:
fixed symbols, any color, commander color, N mana of one color chosen from a
printed list, one of the printed fixed bundles, up to five mana in any
combination of colors, or one mana of a type the controller's or opponents'
lands could produce. A bundle is chosen in full as the ability resolves.
CR 106.7 is the least fixed point over battlefield lands, each read
relative to its own controller from intrinsic, printed and granted abilities,
ignoring costs and object order. If a departure leaves no defined type while
the color is chosen, the ability adds nothing. A source with several mana abilities first
asks which to activate (CR 605.3a). Riders follow in order: damage to the
controller or life gain (CR 605.3b). A nested payment rechecks outer costs.

Statics may grant quoted mana abilities in layer 6 (CR 113.10a, 613.1f). The
recipient taps for its activator (CR 109.5, 302.6); printed then granted
abilities drive activation, views, planning and saves. Grants admit only tap and
life costs, without riders, restrictions, conditions or tapped-status filters.

Reversal (CR 733.1) restores receipt-recorded unrestricted and restricted
mana, taps and counters. An activation that paid life or performed riders is
kept with every activation of that attempt.

CR 106.6 spending predicates distinguish spells, activated abilities and
resolution payments. The spend decision offers only eligible buckets and
validates the exact unrestricted and bucket allocation. The finite rows
cover spell types, colorless spells, artifact spells and battlefield artifact
abilities, colorless Eldrazi spells and battlefield colorless Eldrazi
abilities, and activated abilities generally. Mana with an uncounterable
rider protects the paid spell only when that allocation spends the mana.
The legendary row restricts the payment itself. The instant-or-sorcery row
can pay other costs without protecting them. Mana still empties at step and
phase boundaries (CR 106.4, 500.4).

## Casting, costs, targets and stack

Targets precede costs. Casting always offers a mana window and cannot cancel a
payable spell; reversal refunds only receipt-recorded mana and taps and restores
living players' passes. Costs never guess colors. Base cost, X, additional costs
and increases compose in that order and freeze at CR 601.2f; additional costs
never change mana value. Only the caster answers targets and casting mana. Stack
projections run bottom to top; saved announcement handles never bind successor
incarnations.

Activated abilities use a separate stack ID and historical source handle. Tap
costs enforce readiness and continuous control; failed payment reverses
recorded mana activations; sacrifice costs use ordinary post-activation SBAs.
Divided counters bind shares before costs and commit surviving shares together.

Object target slots share one candidate set, so unequal per-slot filters reject.
Stack targets use public handles and exclude incomplete entries and the
targeting invocation. Spell filters read types, exclusions and stack mana
value, including announced X. Uncounterable spells remain legal targets.
Countered abilities leave the stack. Countered spells go to their owners'
graveyards, with flashback exile applied on departure. An audited fixed
generic tax offers the target spell's controller payment (CR 118.12a, 701.6,
702.34a). Paid costs stay spent. Spell copies are counterable nonlegendary
token spells.

Target legality is checked at proposal and resolution start. Losing it during
payment does not reverse casting (CR 601.6). If every target is illegal at
resolution, the whole program is skipped, including compensation and draws. A
target incarnation never rebinds after bounce or reentry. Target-controller
compensation reads the announced incarnation's controller, still executes when
a legal target survives destruction, and ignores owner and zone successors.

A spell can deal damage as its own source; operands that modify or counter the
source need a permanent ability. Mana-producing spells, including per-permanent
multipliers, count once on resolution and credit a staged mana event. A library
play window follows the face-up exile successors of one exact resolution with
ordinary priority, timing, costs and land allowance; leaving exile ends it, an
undone cast restores the original incarnation, and a next-turn window expires
in cleanup of the beneficiary's next actual turn (CR 400.7j, 514.2, 611.2a).

### Cost keywords and variable costs

Cost keywords with dedicated announcement or payment cells are `Executes`-only.

- **Spell mana symbols**: one leading `{X}`, announced nonnegative before targets
  and frozen (CR 107.3a, 601.2b); two-color hybrid, announced as one alternative;
  single-color Phyrexian, payable with 2 life unless below 2 life (CR 107.4e–f,
  119.4). Several X, monocolored hybrid, colorless or hybrid Phyrexian symbols
  reject, as do hybrid, Phyrexian, X and special symbols in activation costs.
- **Affinity** counts the caster's matching phased-in permanents at CR 601.2f
  and reduces generic mana by that count (CR 702.41). Each instance uses the
  stack-static cost lock. Admitted parameters cover artifact, creature,
  artifact creature, enchantment, token, supported subtypes, historic
  permanents and outlaws.
- **Cycling**: a hand activation on any card type paying fixed mana and
  discarding the card, then drawing one (CR 702.29a). **Typecycling** pays the
  same, then searches for one card of its type (basic land, or an allowlisted
  land or creature subtype), reveals it, puts it into hand and shuffles (CR
  702.29e–f).
- **Kicker** (instant/sorcery): an optional additional mana cost; "if this spell
  was kicked" is `Cond::CostPaid` (CR 702.33a, d).
- **Buyback** (instant/sorcery) uses an optional fixed additional mana cost.
  On successful resolution the paid spell goes to its owner’s hand. A copy
  ceases to exist and flashback still exiles the original on departure (CR
  702.27a, 702.34a, 707.10a).
- **Flashback** (instant/sorcery without X or hybrid): cast from its owner's
  graveyard at normal timing (CR 702.34a, 118.9), exiled instead of leaving the
  stack any other way; an abandoned cast returns it to the graveyard (CR 733.1).
  The public stack entry marks it.
- **Additional sacrifice**: "As an additional cost to cast this spell, sacrifice
  a/an [quality]" is paid after mana and not offered with nothing to sacrifice
  (CR 601.2b, 701.21a). Other additional costs reject as
  `kind:additional-cost:<term>`; ward rejects.
- **Activation costs** combine tap, mana, source sacrifice, life and removing
  +1/+1 counters from the source; unpayable abilities are not offered (CR
  119.4), and one without mana skips the mana window. A chosen sacrifice can
  take several permanents, any permanent or "another", optionally also tapping
  the source (CR 701.21a).
- **Graveyard source**: "[Mana], Exile this card from your graveyard" works only
  in its owner's graveyard and for that owner; the card stays the source through
  announcement and is exiled after mana (CR 113.6j, 602.2a–b, 701.13a).
- **Chosen objects** (`ChosenObjectCost`, CR 601.2g–h, 701.9, 701.13, 701.26): a
  fixed number of cards to discard from the controller's hand (any or one card
  type), untapped controlled permanents to tap (any, one type, one subtype, or
  artifact or creature), or own graveyard cards to exile (any, one type or one
  subtype), combinable with mana and `{T}`. A summoning-sick creature can pay a
  written tap cost, but its own `{T}` needs control since turn start or haste,
  and one tap cannot pay both (CR 302.6). Picks follow targets and mana in
  object order and commit in one rule step; discard picks stay private until the
  move (CR 400.2). The menu offers the ability only if enough objects remain
  after funding mana. Random or whole-hand discard, variable counts, other
  graveyards, self-exile from hand and chosen-object mana abilities reject.
- **Restrictions** are permission gates. "As a sorcery" is main-phase timing;
  your turn, your upkeep, your turn before attackers, combat, end of combat and
  not your turn are typed windows. "Only once each turn" records a use per
  source incarnation and ability, surviving control changes (CR 602.5b–d).
  "Activate only if" is read once as activation begins (CR 602.5): an audited
  `kir::ActivationCondition` counts permanents you control, your graveyard or
  hand, or the source's facts, kept on the stack entry for restore.

### Targets, selection and tap/untap

Battlefield targets use one legality check for castability, announcement and
resolution (CR 601.2c, 608.2b). Criteria can combine card types, subtypes
(including changeling), supertypes, colors, supported keywords, token status,
control, tapped/untapped and attacking/blocking status, and fixed stat bounds.
An ability with one target occurrence can exclude its source with “another.”
Graveyard targets check the announced incarnation, its owner's graveyard and
its printed criteria, including secondary card types and fixed mana-value bounds
(CR 400.7). Exile and return to the owner's hand can target a card in any
graveyard or an opponent's graveyard. A permanent returned from your graveyard
uses the battlefield entry procedure for Aura attachment and land entry choices.
An exiled target's printed creature type can govern a later “if it was a
creature card” instruction. A certified "it" or "that" can reuse the block's sole announced incarnation, and
"this [permanent]" names the current source for tap, untap and owner-zone moves
(CR 400.7).

One optional battlefield or own-graveyard target can bind independent destroy,
tap, untap, move, counter and end-of-turn leaves; zero targets skips the
instruction. A counted occurrence ("two target", "up to three target", two
through seven) is the program's only occurrence: one `Pick` of exactly or up to
N distinct objects, needing N legal objects to cast or place (CR 115.3, 601.2c).
Battlefield destroy, exile, bounce, tap and untap affect legal survivors in one
simultaneous event; P/T, keyword, counter and combat-restriction leaves, a
spell's own damage and graveyard returns to hand or exile run per member.
An unchosen or illegal member skips its leaves; all illegal counters the spell
(CR 608.2b). Per-member damage is separate events; recheck that once batched or
creature-damage triggers are admitted. Victimize declares its creature-card
targets before the sacrifice. A successful sacrifice returns the legal survivors
tapped simultaneously. Other graveyard group entries, "any number" and X
counts, mixed required and optional occurrences
and optional permanent spells reject. "Target player or planeswalker" announces
players for spells and triggers: planeswalker faces are not admitted (CR 115.1,
120.3). Already-matching tap status emits no event and combat membership is
preserved. "Put a +1/+1 counter on each [creature filter]" applies
simultaneously to the current set. “Tap all creatures target player controls”
reads that player’s creatures at resolution.

`ChooseObject` returns one controlled permanent chosen at resolution (CR
608.2d); departures that change the offer invalidate pending answers, empty sets
skip the move, and commander owners may choose the command zone (CR 903.9b).
Hidden hand choices and untargeted multi-object selections reject.

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
you" removes that defender; with no attackable planeswalker or battle kinds,
"or planeswalkers you control" adds nothing. A creature that attacks each
combat if able must attack only while a defender remains (CR 508.1d). Relative
powers and ambiguous "X and/or Y with Z" filters reject.

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
recipient orders (CR 615.7). Damage that can't be prevented takes no prevention,
protection included, and spends no shield (CR 615.12). Source choices, "next time" shields,
redirection, non-damage replacements and other replacement/layer interactions
reject.

## Movement, sacrifice and board-wide effects

**Removal.** Indestructible takes precedence over destruction. A regeneration
shield protects one permanent incarnation until its next permitted destruction,
zone change or cleanup. It taps the protected permanent, clears damage and
removes it from combat (CR 701.19, 514.2). The affected controller orders
competing replacements.
Exile, bounce, sacrifice, zero toughness, legend moves and departure stay
distinct. Removal filters use all types (CR 205.2b), controllers and statuses;
positive type lists are alternatives, exclusions reject any match, and
sacrifice subtype lists are conjunctive. Compensation uses the exact target's
last public appearance (tokens have mana value zero): its controller can create
the printed token recipe or gain life equal to its power, negative as zero (CR
107.1b, 608.2h).

**Returns.** Entry returns target at trigger placement; no legal target
removes the trigger. One announced card from the controller's graveyard,
filtered by printed types, subtypes, colors and fixed characteristic
comparisons, can return to hand or, if a creature card, to the battlefield under
the controller's control, tapped when printed (CR 110.2a); a printed "may" asks
at resolution. A commander bound for hand still offers the command zone.

**Sacrifice** is not destruction. Fling-like effects use paid LKI power. Forced
sacrifices choose in APNAP order with earlier choices visible, then move
simultaneously.

**Board-wide sets** include granted keywords at execution. Destruction, exile,
bounce and all-recipient damage commit atomically; temporary P/T sets capture
recipients at resolution; set tap and untap change matching permanents
simultaneously. Destroy, exile and return-to-hand of "all"/"each" permanents
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
consumed. A search can also put one found card on top after shuffling the rest
(CR 701.24b); a printed reveal publishes it without a library handle.
A spell or activated ability can let its controller search for up to two basic
lands, reveal them, choose one for tapped battlefield entry, put the other into
hand, and shuffle. If the search
finds only one land, that land enters tapped. The split choice offers only the
found cards to the searcher and shows revealed identities to everyone without
library handles or positions (CR 608.2c–f, 701.20a, 701.23b–e). Other search
partitions and ordering several cards after shuffling remain unsupported.

**Library placement and shuffle** can put a single announced permanent on the
bottom of its owner's library, retain the existing single-creature top
placement, or shuffle a single announced permanent into its owner's library.
The shuffle still happens if the owner's commander goes to the command zone
instead. An audited follow-up lets the target's owner draw two cards. A spell
can shuffle itself into its owner's library as its final instruction. It
applies its stack-departure replacements first and uses the owner's normal
shuffle stream (CR 400.3, 401.4, 608.2n, 701.24c, 903.9b).
Tucked tokens and spell copies remain in the zone until state-based actions.
They are absent from card counts, the shuffle permutation and draws
(CR 111.6–8, 121.1, 704.5d–e, 707.10).

A **private top-card selection** looks at a fixed positive prefix of the
controller's library, chooses one card for hand, and puts the remainder on
the bottom in a private order (CR 401.2, 401.4, 608.2d, 701.20e). The
commander hand replacement still applies. Filtered or optional selection,
public top-card reveals, random bottom ordering, and other destinations
remain unsupported for this procedure.

## Battlefield entry replacements

A permanent's own entry replacement sets the tapped state or adds fixed +1/+1
counters before entry triggers and SBAs, on every move onto the battlefield (CR
122.6, 614.1c, 614.12). Conditional tapped entry can count permanents the
incoming controller controls (current structural characteristics, excluding the
entrant) or that player's live opponents; simultaneous entry uses the common
prior battlefield and ordered entry sees earlier moves.

Life-payment entry offers run through `begin_battlefield_entry`; a search asks
each payable entrant in order, counting life already committed, then commits one
simultaneous entry. The controller sees the entering card even from a hidden
zone. A forced tapped entry still offers a payable payment and enters tapped
either way (CR 614.12; the shockland ruling "If an effect puts this land onto
the battlefield tapped, you may pay 2 life, but it still enters tapped.", [Lorwyn Eclipsed release notes](https://magic.wizards.com/en/news/feature/lorwyn-eclipsed-release-notes)).
Payment and entry commit together; an unpayable offer is skipped.

An entry reveal ("you may reveal a [filter] card from your hand. If you don't,
this land enters tapped.", CR 701.20a) lowers to `RevealOrEnterTapped` over the
hidden-card filter vocabulary and uses the same continuations. The offer lists
Decline, the entrant as subject, then the matching cards of the incoming
controller's hand other than the entrant. A live controller is always asked,
with Decline alone when nothing matches, so the wait discloses nothing about the
hand (CR 400.2). A search asks reveals after all payments. A reveal publishes the
card's identity in that hand (`known_searched_hand`) without exceeding the
revealed copies. Other entry costs (behold, multiple cards) reject.

## Keywords, attachments and temporary effects

Battlefield static abilities use KIR scopes, predicates, conditions and
characteristic changes; the first applicable layer selects recipients for every
change of the effect (CR 611.3, 613.6). Layer 4 adds artifact and enchantment
types or named creature subtypes. Layer 5 sets colors, and layer 6 grants
supported keywords, including basic landwalk. Layer 7b sets base power and
toughness. Layer 7c applies signed modifiers, including anthems filtered by
live controller or subtype, and counters (CR 613.4c). An “other” filter
excludes the source. Layer-4 creature-subtype filters and layer-5 color
filters reject. Additive creature subtypes can read Creature membership after
a resolving animation grants it (CR 613.8). Ordered setters apply by source
or creation timestamp (CR 613.7). Simultaneous competing setters need a
timestamp choice and reject (CR 613.7m).

Changeling and a self "is every creature type" CDA are a printed characteristic
in every zone (CR 702.73a, 604.3), as is changeling on a token. Every subtype
check counts it only while the object is a creature (CR 205.3d, 205.3m). An
Equipment, Aura or source static can make its recipient every creature type, as
an unfiltered layer-4 change. It applies in timestamp order with resolved
animations, whose creature-subtype replacement removes it (CR 613.3, 613.7).
Scopes that reach other zones, conditional grants and resolving grants or
removals reject.

A printed `*`, `N+*` or `*+N` power or toughness needs one self CDA per starred
stat (`StaticEffect::DefinePowerToughness`, functioning zone `Any`; CR 208.2a,
604.3). Layer 7a sets the stat in every zone, before 7b setters and 7c anthems
and counters (CR 613.4a), whenever it is read; nothing locks it in, so SBAs see
a live zero toughness (CR 704.5f). Values: counts of permanents, graveyard cards
or whole hands; `Times`, `Sum` (the printed `N` is its one fixed term), greatest
mana value, devotion and `Distinct` card types (exhaustive over
`CardKind::CARD_TYPES`, CR 205.2a), basic land types or colors. They
read layers 1–5 of other objects, never P/T or abilities. Chosen players, life
totals, counters, event objects, exile and other values reject under
`static:cda-pt:*`; a star without a CDA rejects under `source.power` or
`source.toughness`.

Printed triggered keywords lower to the triggers their rules define, and each
instance triggers separately. Prowess and bushido N need a creature card. Afterlife N
and soulshift N need a permanent card. Prowess pumps the source +1/+1 on its
controller's noncreature cast. Bushido N pumps +N/+N when the creature blocks or
becomes blocked. Afterlife N creates N 1/1 white and black flying Spirits when the
permanent dies. Soulshift N may return a target Spirit card with mana value N or
less from your graveyard to your hand. These rules are CR 702.108, 702.45, 702.135
and 702.46. The keyword line must print each N, and the source keyword list must
name each keyword. X amounts reject. So do the remaining triggered keywords, such as
battle cry (an "other" set filter), exalted (attacks alone), undying/persist
(counter memory and entry counters) and extort (hybrid resolution payment).

Static guards can read the active player, public battlefield or graveyard
counts, and devotion (CR 700.5). Rule statics apply combat prohibitions and
attack requirements (CR 508.1c–d), countering prohibitions and flash timing.
Cost increases precede reductions, and excess colored or colorless reduction
reduces generic (CR 118.7a–d). Land allowances add plays per turn (CR 305.2);
hand-size effects use source timestamps at cleanup. Static control changes,
other characteristic-defining abilities, ability removal and casting from additional
zones reject.

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
counts, including a fixed quantity per matched object. Each instruction creates
one simultaneous batch (CR 111.1–4, 608.2h). Creature recipes preserve supported
printed keywords, secondary types and tapped entry. Treasure, Gold, Food and
Clue use their CR 111.10 templates through the normal mana and activation paths.
Restore rebuilds the template from the creating source and token identity.
Printed abilities, other predefined recipes, copy recipes in this path,
event-result counts and token-creation replacement abilities reject.

### Keyword actions and counters

- Investigate creates a Clue (CR 701.16a).
- Populate copies a controlled creature token's copiable values; animation can
  qualify the original (CR 701.36, 707.2).
- Proliferate adds one counter of each existing kind to chosen permanents and
  players simultaneously: permanent +1/+1 and player poison and energy; ten
  poison counters lose (CR 701.34a, 704.5c).
- Fight deals current power as simultaneous noncombat damage between two legal
  battlefield creatures; self-fight deals double power (CR 701.14).

### Continuous permanent control

`GainControl` captures one battlefield target, the exact source or a matching
battlefield set at resolution (CR 611.2c) for the invocation's controller or an
announced player. It lasts until cleanup, indefinitely, or while the exact
source remains on the battlefield; that duration must hold at resolution and
ends if the source leaves or phases out (CR 611.2b, 702.26f). Durations that
depend on who controls the source reject.

Layer 2 applies active grants in creation order; a later grant overrides an
earlier one, which applies again when the later ends. Each grant assigns a fixed
player to its captured incarnations, so grants never depend on each other (CR
613.8); with none left, the default controller applies.

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

### Temporary keyword and set families

A temporary effect needs a checked KIR operation and runtime support for its
exact target, duration and modifier. Grants and settled effects follow creation
order, expire at cleanup and affect only their captured incarnations (CR 400.7,
514.2, 611.2c).

- `SetCharacteristics` captures a battlefield target, the exact source or a
  matching set under one timestamp. Animation adds creature, optionally
  artifact, keeping previous types; fixed creature subtypes are added, or
  replace only creature subtypes where printed. It can also set colors, grant
  keywords and set base power and toughness, each stat's setters competing by
  timestamp. Counters persist after animation expires (CR 122.2).
- Signed pumps take fixed deltas or signed announced X, read at resolution, on
  one announced creature or an activated ability's source (CR 107.3, 611.2d).
  Until-end-of-turn grants add supported keywords to one announced creature,
  the source or a captured set. Copy effects, ability removal, chosen
  characteristics and computed pump amounts reject.
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
| `Cast` | the spell's frozen stack characteristics, with the caster in a player set and type or subtype predicates (CR 601.2i); `targets_source` requires the source among its targets as it became cast (heroic, CR 601.2c) |
| `DealsDamage` | the source's battlefield appearance as positive damage is dealt to a player (CR 120.3, 510.2) |
| `StepBegins` | upkeep, draw, beginning and end of combat, end, precombat and postcombat main (CR 503-513) |
| `BecomesTapped` | only the source itself (CR 701.26a) |
| `GainsLife`, `Discards` | the gaining or discarding player; each source's positive gain is one event (CR 119.9, 701.9a) |
| `Sacrifices` | the sacrificing player and the permanent on the battlefield just before the move (CR 701.21a, 603.10a) |
| `LeavesBattlefield` | a move off the battlefield or a phased-in permanent leaving the game with its owner, read as for `Dies` (CR 603.6c, 603.10a) |
| `Blocks`, `BecomesBlocked` | each declared blocker once, and each attacker unblocked until that declaration, before it (CR 509.3a, 509.3c) |
| `IsDealtDamage` | a creature dealt positive damage, once per simultaneous damage event (CR 120.3, 603.2c) |

Intervening comparisons can read this turn’s declared attacks, creature deaths
(any seat or under your control), departures under your control, spells cast
(all, noncreature, or from your hand), draws and total life gained. Departures
use the pre-move controller and types. Each check reads the current turn from
its start, including the event that caused the trigger (CR 603.4, 603.10a).

A per-turn player ledger (spells as frozen stack facts, draws, life-gain
events) counts from the turn's start, whether or not the source was present,
and resets with the turn. A `TurnOrdinal` on cast, draw or life gain triggers
only on the Nth matching event of the event player, or of every player for
"the Nth spell of a turn"; a filtered ordinal counts only spells that matched
as cast. "This ability triggers only once each turn" records a use per source
incarnation, face and ability once it actually triggers (after its intervening
check), read live so simultaneous occurrences see it; "one or more" under that
limit lowers as the per-object form when nothing reads the event. Witnesses
carry the matched count; other ordinals, periods and limits reject as
`trigger:occurrence:*` or `trigger:limit:*` (`TriggerTurnHistoryV1`).

Same-kind alternatives use one predicate. Mixed entry/death, entry/attack,
entry/cast and block/becomes-blocked pairs use one ability index without common event operands
(CR 603.2c). Recovery rejects duplicate occurrence members and repeated
entry or death of one incarnation. Event filters read frozen facts. Cast and
attack filters omit tapped status (CR 508.1f). Host exclusions reject. A bound
entrant or attacker can take fixed counters or deal directed damage. Entrant
damage can use its power, measured when the effect resolves or from its last
battlefield appearance (CR 400.7, 608.2h).

`kir::Intervening::of` admits cast provenance, empty hand, source tapped
status, own commander control, a creature power threshold and filtered
battlefield counts. Relative counts compare lands or creatures against each
opponent separately. Entry, attack and step triggers can compare the
controller's life, hand or own graveyard card count. Both stages record
`InterveningCheck` evidence (CR 603.4). These rows earn `Executes`
(`InterveningIfV1`, `EventTriggerV1`).

A source-relative combat-damage trigger can put fixed +1/+1 counters on the
incarnation that dealt the damage (CR 400.7). Cascade has no executable R5 form.

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
trigger's event player. Object payments are controller-only. An optional
sacrifice of one creature followed by a dependent clause shares the payment
continuation. The controller selects an eligible object or declines. A private
discard always offers decline, even with no eligible card (CR 400.2). An optional body is a draw, life change, mill, search,
shuffle, scry or surveil, or a tap, untap, move or counter verb on a target or
the source. Impossible choices are not offered and are withdrawn if they become
impossible while pending (CR 608.2d): no object can still change or move, no
live player, a zero amount, or a library smaller than the mill (CR 701.17b).
`IfPays` acts only after payment. A departed payer pays nothing. A
self-replacement that only changes an action's amount selects one admitted
branch before the action, retaining its original announcement (CR 614.15). These earn
`Executes` (`Cell::Control`, `kernel_control_v1`). Everything else rejects
under `compose.control:<detail>`, including "if you do"
after unsupported actions, other conditions, nested leaves needing persistent
provenance, and `forEach` amounts (CR 121.2a).

### Modal spells and abilities

A printed "Choose one", "two", "one or both", "one or more", "up to one"
or "any number" over two to five
bullets lowers to a `ModalSpell` declaration (CR 700.2): an instant or
sorcery's only block, or a trigger's or activation's text after its header.
Counts and bullets must match the AST and certified spans and every mode must
lower, or the card rejects under `verb:modal:<detail>`. Each legal set of
distinct modes is its own program in printed order (CR 700.2d), yielded by
`spell_programs` and `programs`; the base program keeps timing only.

Spells and activations choose modes before costs, X and targets (CR 601.2b);
a trigger chooses with its targets and leaves the stack with nothing offerable
(CR 603.3c). Only selections with legal targets are offered. The public `mode_mask` selects
the program, with a zero mask representing a chosen empty set. Spell
selections can announce independent object occurrences or a single counted
target group. Illegal targets suppress their own effects if another target
remains legal (CR 608.2b). These earn `Executes` (`modal_choice_v1`).
Repeats, entwine, escalate, spree, costed modes, upgrades, mixed target
announcement routes, and mana modes are outside this subset.

### Resolution-time counts

"[N] for each [filter]" and a sentence's "where X is [number]" are read as the
step that prints them is performed (CR 107.3, 608.2h), after earlier steps of
the same program: `Amount::Count` over objects you or opponents control,
graveyard cards or (not for draws) your hand, `Times` for N above one,
`Greatest` power, toughness or mana value among permanents (never below zero,
CR 107.1b) and your devotion (CR 700.5). Draws, life changes, set or player
damage, token creation and P/T changes read them; a P/T change keeps its value in the saved
effect (CR 611.2c) and a counted draw its recorded attempts. Several steps may
re-read one X only across draws, life changes and damage. A step that cannot
read X, a hand count re-read, announced or minimum quantities, event results
("this way"), another player's referents and per-object iteration reject under
`bindings:*` or `operand:bindings:*`.

## Derived capability tiers

Every compiled card is lowered one printed block at a time (`composed: true`,
`composed_ability_v1`). A definition consists of cells: the audited kernel cells
`kir::check` names (`kir::Cell`, one `(verb, operand kinds, context)` row each)
and the reviewed non-kernel surfaces named by their capabilities. Each cell
carries the tiers it has earned; a definition earns a tier only when every cell
has (the meet). The tables are `kir::cell_tiers` and `kir::surface_tiers`; no
card name, Oracle ID or profile appears in them, and a newly opened cell starts
at `Executes`.

| Tier | Earned when | Admits to |
| --- | --- | --- |
| `Executes` | the cell has an executor and a rules test | dynamic headless pods; a runtime fault is the pod's `RulesFault` |
| `HeadlessQualified` | it ran in the pod parity corpus with state validation and snapshot round trips | `--analytics-out` and paired runs |
| `Search` | its hidden-information behaviour is reviewed for the sampler | profiles requesting search |
| `Hosting` | every decision it raises has source labels and a client control | profiles requesting hosting |

The three reviews above `Executes` are independent. A rejected hosting or search
request names the capabilities (`UnreviewedCapabilities`) or kernel cells
(`UnqualifiedCells`) below its tier.

`Executes` is decided per surface, each row independent (CR 113.3, 602.1,
603.1, 608.2); a surface without a row rejects under `cell:<surface>:<detail>`:

| Surface | Executes |
| --- | --- |
| Instruction × stack object | A checked KIR node with operands bound in its spell, activated or triggered context. |
| Target route × stack object | Battlefield permanents/creatures and any-damage targets everywhere; players and own-graveyard cards for spells and triggers; stack targets for spells and activations. |
| Cost × route | The [costs](#cost-keywords-and-variable-costs) above, only where that announcement route pays the complete cost (CR 602.2b). |
| Event | An admitted `kir::EventPattern` on a battlefield permanent, with at most one audited intervening condition; reviewed patterns keep their capabilities, others use `EventTriggerV1`. |
| Static, mana, keyword | Own entry tapped, scoped continuous changes, mana grants, rule statics and P/T CDAs (`characteristic_power_toughness_v1`); the [mana abilities](#mana-abilities) (`kir::mana_ability_cells`); evergreen keywords without parameters. |
| Kernel node | A [kernel node](game-engine-catalogs.md#kernel-ir) with checked children, bindings and an executor for its stack context. |
| Card shell | Normal cards and [double-faced or adventurer cards](#double-faced-cards-and-adventurers) whose parts have complete bound source and supported spell costs and keywords. |

Sentences resolve in printed order (CR 608.2c); sorcery-speed activations use
main-phase timing (CR 602.5d); tokens recover from the creating recipe.
[Compositional lowering](game-engine-catalogs.md#compositional-lowering) owns
the R5 source, pronoun and binding checks.

## Admission and policy boundaries

The Foundations profile supplies the retained hosted and search decklists; other
R5 fixture catalogs exercise private rules and compiler tests.

Develop policy uses permitted observations and public IR features; it may
estimate value but never determines legality or inspects hidden state. Core and
policy share the `mtg_core::query` filter and board-wide predicates: core reads
derived state through a private adapter, a policy reads only `VisibleObject` and
`PermittedView`, and an unpublished fact answers `Unknown`. Core's own target
legality remains the authority. Paid accepted-pod prefixes, private board
setups, synthetic IR controls and fixed-index completion games stay separately
labeled in [evidence](game-engine-evidence.md), which owns producers, proof
locations and omissions; [catalogs](game-engine-catalogs.md) own reproduction.
