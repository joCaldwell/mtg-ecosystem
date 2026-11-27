# Engine mechanics: core, casting and stack

Every executable card lowers completely from Archive R5 to schema-1 KIR.
`crates/contracts/src/kir/tiers/executes.rs` validates the card shell and
ability context, `kir::check` the nodes and bindings, and `kir::tiers` derives
the [tiers](#derived-capability-tiers) the card earns. Admission requires a
fresh complete lowering and a matching catalog identity; match profiles
independently select decks and consumer features. Every shape outside the
contracts below rejects explicitly with a named detail.

This file owns the runtime invariants, turn execution, card shells, Commander
and hidden information, mana abilities, casting, costs, targets and the stack,
plus [derived tiers](#derived-capability-tiers) and admission. Combat,
permanents, battlefield entry, keywords, attachments and triggers live in
[board mechanics](game-engine-board-mechanics.md).

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
lands could produce. A printed mana bundle of one type may multiply by an
audited battlefield or public graveyard count after the activation costs
(CR 605.3b, 608.2h). The planner applies earlier simulated taps and the source's own tap
before reading the count from the permitted view. When quoting a spell with
convoke or improvise, a reserved permanent contributes only if it matches
both before and after its reserved tap. Mana production precedes those costs
(CR 601.2g–h). A zero count adds no mana.
A bundle is chosen in full as the ability resolves. A devotion choice offers
all five colors, then reads devotion to the chosen color (CR 700.5).
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
- **Delve** lets the caster exile cards from their graveyard to pay generic
  mana after the cost locks (CR 601.2h, 702.66). The engine commits the
  chosen exiles with the remaining mana payment.
- **Convoke** lets the caster tap untapped creatures they control after the
  mana ability window to pay generic mana or mana of each creature’s color
  (CR 601.2h, 702.51). Summoning sickness does not limit this payment.
- **Improvise** lets the caster tap untapped artifacts they control for
  generic mana after the mana ability window (CR 601.2h, 702.126).
- **Assist** reports `keyword:assist`. It requires a second player’s mana
  ability window and generic allocation before the caster pays (CR 702.132).
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
- **Evoke** (permanent without X or hybrid): a fixed alternative mana cost
  announced with the optional costs on hand or command-zone casts; it
  replaces the mana cost, not the mana value, and a copy keeps it (CR
  118.9, 702.74a, 707.10). The entry trigger reads the completed cast and its
  controller sacrifices that exact permanent (`SacrificeSource`, CR 603.4,
  701.21a). Develop evokes only when the full cost cannot be funded.
- **Unearth** (permanent, fixed mana): a sorcery-speed graveyard ability
  whose delayed trigger returns the exact card, captures the move and exiles
  that permanent at the next end step. The returned incarnation has haste and
  is exiled instead of leaving the battlefield for any other zone; a card
  that left the graveyard returns nothing (CR 400.7, 603.7, 614.1a, 702.84a).
- **Encore** (single-faced creature, fixed mana): a sorcery-speed graveyard
  ability exiling the card as its cost; its copies follow
  [delayed triggers](game-engine-board-mechanics.md#delayed-and-reflexive-triggers)
  (CR 702.141a).
- **Dash** (creature without X or hybrid): a fixed alternative mana cost
  announced like evoke; haste and the return follow
  [delayed triggers](game-engine-board-mechanics.md#delayed-and-reflexive-triggers)
  (CR 702.109a). Rebound, cumulative upkeep, eternalize, embalm, madness and
  scavenge reject as `keyword:<name>`.
- **Flashback** (instant/sorcery without X or hybrid): cast from its owner's
  graveyard at normal timing (CR 702.34a, 118.9), exiled instead of leaving the
  stack any other way; an abandoned cast returns it to the graveyard (CR 733.1).
  The public stack entry marks it.
- **Retrace** (instant/sorcery without X or flashback): also cast from its
  owner's graveyard at normal timing for its mana cost, discarding one land
  card from hand as a chosen-object cost after mana; not offered without one,
  and the spell leaves the stack normally (CR 702.81a, 601.2h).
- **Escape** ("Escape—[mana], Exile N other cards from your graveyard", no X,
  hybrid, delve or other graveyard route): a graveyard cast whose fixed mana
  replaces the mana cost and whose exile picks N other own graveyard cards
  after mana; not offered with fewer, and the spell leaves the stack
  normally (CR 118.9, 702.138a). Other escape costs and "escapes with"
  reject. Develop plans retrace and escape only from the engine quote.
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
  not your turn are typed windows. "Only once each turn" and "no more than
  N times each turn" (stack abilities only) number each use per source
  incarnation and ability, surviving control changes (CR 400.7, 602.5b–d).
  "Activate only if" is read once as activation begins (CR 602.5): an audited
  `kir::ActivationCondition` counts permanents you control, your graveyard or
  hand, the source's facts, your turn facts (tokens created, spells cast,
  instant/sorcery spells, attacks, life gained; any creature died), one
  opponent's matching permanents or more lands/creatures than you, your
  largest same-name group (CR 201.2), card types in your graveyard, or life
  above your starting total, kept on the stack entry for restore. Develop's
  mana plans leave sources with the newer conditions out.

### Targets, selection and tap/untap

Battlefield targets use one legality check for castability, announcement and
resolution (CR 601.2c, 608.2b). Criteria can combine card types, subtypes
(including changeling), supertypes, colors, supported keywords, token status,
control, tapped/untapped and attacking/blocking status, and fixed stat bounds.
An ability with one target occurrence can exclude its source with “another.”
Graveyard targets check the announced incarnation, its owner's graveyard and
its printed criteria, including secondary card types and fixed mana-value bounds
(CR 400.7). Exile, return to the owner's hand and placement on top of the
owner's library can target a card in any graveyard or an opponent's graveyard. A permanent returned from your graveyard
uses the battlefield entry procedure for Aura attachment and land entry choices.
A creature can enter from any or an opponent’s graveyard under the specified
controller while retaining its owner (CR 110.2, 110.2a). A following life-loss
instruction can read that card’s last-known mana value (CR 202.3, 608.2h).
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
