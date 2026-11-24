# Supported engine mechanics

Every executable card lowers completely from Archive R4 to schema-1 KIR.
`contracts/src/kir/tiers/executes.rs` validates the card shell and ability
context, `kir::check` the node and bindings, and `kir::tiers` derives the
[tiers](#derived-capability-tiers) the card earns. Match profiles independently
select decks and consumer features. Card admission requires a fresh
complete lowering and a matching catalog identity.

A spell can deal damage as its own source. Object operands that modify or put
counters on the source require a permanent ability. Temporary source keyword
grants require an activated ability and the exact supported keyword list.
An announced player can draw one card per KIR draw. Targeted counted draws
reject at admission until their executor can skip an illegal target.

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
imports contracts only at runtime: no compiler/parser, I/O, clocks, OS randomness,
async inference or unsafe code. Keep physical IDs, zone incarnations, stack IDs,
owners, default control and explicit grants distinct; quantities are exact and
structural arithmetic checked. Frames/inputs/events/outcomes serialize as data;
clones are independent and restore proves accepted-input replay with exact identity.

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

`Game::with_trace` selects Off/Compact/Detailed. Diagnostics enter neither semantic
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
Cleanup discards to the maximum hand size of seven (CR 402.2, 514.1), then
clears temporary effects and marked damage atomically. Concessions are
trusted recorded external inputs processed before work or queries. Land usage
resets each turn.

<a id="player-visibility-and-query-recovery"></a>

Public queries return only existing viewer-permitted handles and context. They
cannot publish views/history, mint handles, mutate semantics or authorize later
submission. Receipts and recovery sidecars remain private and executor/state
bound. Payment queries and [action beginnings](game-client.md#core-action-discovery) use
existing validators, with explicit incomplete/stale/unavailable outcomes. A
complete initiation menu does not enumerate every eventual target/payment choice.
Raw IDs, `decision_for`, full-state getters and snapshots are trusted host APIs;
public code consumes Observation/PlayerRequest/PlayerResponse. Public revisions
change only with permitted information. Real restore rejects sampled origins;
[search](game-engine-search.md) separately proves hypothetical origin/reconstruction.

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

Library manipulation is private to its chooser and replay-proven. Scry, Surveil
(CR 701.22, 701.25), mill (CR 701.17), library selection (not drawing, CR 121.5)
and top-ordering partition the available prefix, clamp short libraries and emit
nothing for zero. Optional shuffles consume no randomness when declined. Counted
draws make sequential unclamped attempts; each-player draws finish each seat in
turn order. Discards clamp to hand size, group choices stay private until one
simultaneous discard, and random discard samples without replacement. Library-top
events preserve order and cannot compose with a conflicting order write. These
all finish before Commander SBAs; departure never redirects attempts.

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

## Casting, costs, targets and stack

Fixed mana-producing spells and exact per-permanent mana multipliers use the
ordinary stack. Casting pays all costs before resolution. The resolving spell
counts matching permanents once and credits its controller through a staged
mana event. A zero count leaves the mana pool untouched.

Cast observers can match a spell subtype or a disjunction of subtypes against
its frozen paid cast event. A temporary creature combat restriction attaches to
the resolved target incarnation, expires in cleanup, and applies when blockers
are declared (CR 506.4a, 509.1b). Its paid source and ordered sibling instructions
remain distinct.

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
Entity, player, opponent and stack targets keep distinct domains; stack targets
exclude incomplete entries. Countered spells move to their owners' graveyards,
abilities disappear and costs stay spent. Spell copies are separately counterable
nonlegendary token spells.

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

### Target status and tap/untap

A tapped-status filter requires current tapped status; absence means no
restriction, not untapped. Power bounds use derived power at announcement and
resolution. Tap/untap spells and activations, tap-source untap or destruction,
cast-entry land untap and owner-hand return each accept only their complete
validated shape. Already-matching status emits no second event, combat membership
is preserved and source tap is paid first. Paid activations survive source
departure.

<a id="combat-damage-and-state-based-actions"></a>

## Combat, damage and life

Attackers, blockers and damage commit atomically. Readiness stamps continuous
control at turn entry and resets on control/incarnation change. Blockers have no
damage assignment order. Blocked and blocking status survive counterpart
departure until combat ends (CR 509.1g, 511.3). Skip blocker/damage steps only
when none attacked. First and double strike have distinct participant sets;
trample and deathtouch assignments, dealt damage and prevention remain distinct.

Validate simultaneous damage before life/history writes. Assignment stays
distinct from current or latest-departure source facts governing damage-time
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

Indestructible takes precedence over destruction. A paid regeneration shield
replaces destruction when permitted, tapping its source, clearing damage and
removing it from combat. The affected controller orders competing exile. Exile,
bounce, sacrifice, zero toughness, legend moves and departure remain distinct.
Removal filters use all types (CR 205.2b), controllers and statuses; positive type
lists are alternatives, exclusions reject any match, and sacrifice subtype lists
are conjunctive.

Compensation uses the exact target's current or last relevant public appearance,
not its successor; tokens have zero mana value. Entry returns target at trigger
placement; optional returns choose at resolution; no legal target removes the
trigger.

Sacrifice moves a controller's own permanent to its owner's graveyard and is not
destruction. Choose/pay sacrifices after targets; “another” excludes the source.
Fling-like effects use paid source-backed LKI power. Forced opponent sacrifices
choose in APNAP order with earlier choices visible, then move simultaneously; a
player with no candidate sacrifices nothing.

Board-wide sets include granted keywords at execution; destruction, exile, bounce
and all-recipient damage commit atomically, and temporary P/T sets capture
recipients at resolution. Destroy, exile and return-to-hand of "all"/"each"
permanents take a general filter, read as the instruction is performed
(CR 608.2h): conjunctions of card types, type alternatives, excluded types,
basic/legendary, subtypes, colors, controller (you or your opponents), tapped,
untapped, attacking, token, flying and fixed power, toughness or mana-value
comparisons. Types and supertypes are current structural ones, colors current,
subtypes, power, toughness and keywords fully derived; a permanent's mana value
is its printed one (CR 202.3). Any other filter term rejects the card.
Forbidding regeneration stays limited to the nonland-permanent wave. Counts use typed outcomes, including exile-replaced but
excluding prevented destruction. Turn-long exile replacement covers only
creatures actually dealt its damage, so exile causes no death. Indestructible
prevents destruction before replacement; generic departures never imply dies
triggers. Typed land search may find zero (CR 701.23b); found lands enter
simultaneously without reveal, then the library shuffles.

## Keywords, attachments and temporary effects

Intrinsic/static grants, counters, admitted anthems, Equipment and Auras
contribute to derived characteristics; scoped anthems check live control and
derived types. Grants disappear with actual attachment loss; paid abilities and
resolved spell effects keep independent lifetimes. Unimplemented
CDA/setting/copying/swapping and general layer combinations reject.

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

Signed attachment stats apply in the creature power and toughness layer.
Fixed-color protection follows the current creature and affects targeting,
blocking, damage prevention and attachment legality. Applied prevention keeps
the protection facts captured when the damage was processed.

Predefined Treasure, Food and Clue use their standard artifact token templates.
Creation stages the new objects together. The normal mana and activation paths
pay the printed token costs. Restore reconstructs the template from its creating
source and token identity, including after the token ceases to exist.

Continuous subtype grants evaluate each permanent's current controller and
structural subtypes. An explicit creature restriction limits eligibility to
creatures. Additive Golem stats apply in the creature power/toughness layer.

### Continuous permanent control

Kernel control grants determine control in layer 2; the latest supported grant
wins and removing it reveals the default controller (CR 613.1b). A grant whose
static source would itself need control-related exile rejects atomically.

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

<a id="abilities-targets-and-triggers"></a>

## Trigger families and event witnesses

The shared trigger pipeline operates on the events admitted by composition.

Collect triggers from actual typed occurrences, wait through SBAs/Commander
choices, then place APNAP, selecting targets/order at the proper boundary. Freeze
controllers, actors, incarnations and witnesses. Intervening conditions recheck
at resolution. Source
status uses current incarnation or departure-refreshed LKI. Cast triggers collect
after successful payment; declared attacks use the real declaration batch.

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
| Cost × route | Tap, mana, source sacrifice, one sacrificed permanent of a type, type union or subtype, and tap/mana/sacrifice combinations, only where that announcement route pays the complete cost (CR 602.2b). |
| Event | Self entry of any permanent type, self dies, becomes tapped, beginning steps, casts, attacks, damage to a player, other creature entries, creature deaths and controlled land entries, without intervening conditions. |
| Static, mana, keyword | Own entry tapped, uncounterable spells, controller creature modifiers; tap-only fixed/any/commander-color mana; evergreen keywords without parameters. |
| Kernel node | A [kernel node](game-engine-catalogs.md#kernel-ir) with checked children, bindings and an executor for its stack context. |
| Card shell | A single-face normal card with a fixed cost and complete bound source. Unsupported shells and keywords reject. |

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
