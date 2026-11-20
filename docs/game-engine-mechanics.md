# Supported engine mechanics

This is the current semantic map, not a claim of general Magic support. Read
the runtime invariants below before editing rules. Exact Rust IR validation in
`crates/contracts`, lowering in `crates/compiler` and complete archived source
fixtures define executable shapes;
[profiles](game-engine-catalogs.md#match-profile-data) independently admit decks
and consumers. Source/compiler support, private runtime witnesses, accepted-pod
prefixes, completion games, hosting/search review and browser acceptance are
separate.

The rules baseline is the frozen [2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Use exact `tests/fixtures` records/provenance under `packages/game-engine`, never
card facts from memory or live refresh. Historical IR11–33 labels are evidence
vocabulary; the only active executable schema is 1. No name/ID/profile dispatch:
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
Cleanup applies net hand-size adjustments to finite limits, floored at zero,
then clears temporary effects and marked damage atomically. Concessions are
trusted recorded external inputs processed before work or queries. Land usage
resets each turn. Listed mana abilities offer only printed colors.

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
decision resumes the same instruction before later instructions execute.

Departures preserve original seating and Commander history; all simultaneous
losers leave before cleanup/adjudication. Never resurrect their objects/mana.
Surviving activators retain paid abilities when a borrowed source's owner leaves.
Personal effects for a departed invocation controller skip rather than redirect.

<a id="announcement-priority-and-payment"></a>

Typed nonbasic lands derive intrinsic mana. Optional-life entry chooses before
debit/entry and atomically commits payments, entries and land-play usage.
Land-play and paid land-search continuations retain ownership, privacy and
recovery. Fetch activations atomically tap, pay life and sacrifice the source
before typed search; paid roots retain LKI through source-owner departure.

## Casting, costs, targets and stack

Stack projections run bottom to top. Saved announcement handles are historical
and never bind successor incarnations; reversal restores living players' passes.
Resolving frames retain frozen instances through supported source departure
(CR 608.2m). Only the caster answers targets and casting mana; modes are chosen
before payment and resolve in printed order.

Targets precede costs. Casting always offers a mana window and cannot cancel a
payable spell; reversal refunds only receipt-recorded casting-time mana and taps.
Costs never guess colors. X is announced nonnegative and frozen. Base or
alternative cost, X, additional costs and increases precede floor-zero
reductions; counted reductions freeze at CR 601.2f and replay without later-board
repricing. Additional costs never change mana value.

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
general replacement/layer interactions reject instead of approximating; damage
doubling compares the provider's current controller with the source's current or
exact-LKI facts (CR 609.7c).

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
recipients at resolution. Counts use typed outcomes, including exile-replaced but
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

Continuous subtype grants evaluate each permanent's current controller and
structural subtypes. An explicit creature restriction limits eligibility to
creatures. Additive Golem stats apply in the creature power/toughness layer.

### Continuous permanent control from Auras

Pure Aura sources may grant their current controller control of an enchanted
creature, artifact, land or general permanent (CR 109.5, 110.2, 613.1b). Only
creature control may carry one bounded modifier; unions, qualifiers and extra
abilities or costs reject. General control still needs dependency-aware and
paid-recovery qualification before publication, and source admission grants no
catalog or consumer admission. Shapes live in `crates/contracts`, probes in the
[schema harness](../packages/game-engine/crates/contracts/tests/aura-control-schema-probes.mjs)
and qualification in [evidence](game-engine-evidence.md#atomic-temporary-keyword-qualification).

The current source, exact attached target incarnation and attachment timestamp
determine authority. Acyclic attachment graphs reevaluate CR 613.8 dependencies
after each attempted operation; independent effects use timestamps. Structural
cycles, and an Aura combined with a synthetic control grant on one permanent,
reject atomically. Removing authority reveals remaining effects or default
control; a successor never inherits its predecessor's attachment.

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

### Shared temporary creature keyword family

Targeted temporary grants cover one complete ordinary nonmodal instant/sorcery
with one creature target (unfiltered or controlled by the invocation controller)
and one atomic grant until end of turn, optionally with a fixed signed P/T
modifier. A permanent form grants keywords to one controlled permanent, without
stats, optionally followed by fixed life gain. Older single-keyword and compound
capabilities keep their exact subset meanings; other combinations require the
general family capability. Split stat/grant sequences, modes, X, extra costs and
extra operations reject.

One captured incarnation receives both stat and keyword layers (CR 611.2c,
613.5–613.6), including departure LKI during concessions (CR 104.3a). Vectors are
nonempty, duplicate-free and limited to Flying, Deathtouch, Indestructible,
Lifelink, Hexproof, Haste, Trample, Reach, Vigilance, First Strike and Double
Strike, which reuse existing rules (CR 702.4, 702.7, 702.10, 702.17, 702.19,
702.20). Each keyword, modifier and target domain requires its own capability.
“You control” means current control at announcement and resolution, not
ownership. Hexproof uses existing targetability. Grants keep their captured
incarnation despite later control changes and expire atomically with cleanup.
[Schema probes](../packages/game-engine/crates/contracts/tests/temporary-creature-keyword-schema-probes.mjs)
hold synthetic controls; qualification is in
[evidence](game-engine-evidence.md#current-temporary-keyword-family-qualification).

### Temporary set keyword families

Set families grant the [shared vocabulary](#shared-temporary-creature-keyword-family)
to the invocation controller's creatures, all attacking creatures, or the
invocation controller's permanents. Each is one complete ordinary instant/sorcery
without targets, modes or additional costs, creating one end-of-turn effect with
an optional fixed P/T modifier (forbidden for permanents). Each requires keyword
primitives, the shared board-wipe set machinery and exactly its selector's family
capability; split pump/grant pairs reject.

The instruction captures one full set and creates all properties atomically:
CR 611.2c fixes membership and CR 613.5–613.6 applies both parts to the same
exact incarnations, so departure LKI never observes half an effect and later
entry or control changes never reselect (CR 608.2h). Restore and replay
authenticate captured membership. The compiler lowers one complete sentence and
never coalesces separate sentences.
[Creature](../packages/game-engine/crates/contracts/tests/creature-set-keyword-schema-probes.mjs)
and [permanent](../packages/game-engine/crates/contracts/tests/permanent-set-keyword-schema-probes.mjs)
probes hold the synthetic controls.

<a id="abilities-targets-and-triggers"></a>

## Trigger families and event witnesses

Collect triggers from actual typed occurrences, wait through SBAs/Commander
choices, then place APNAP, selecting targets/order at the proper boundary. Freeze
controllers, actors, incarnations and witnesses. Power-filtered entries use
derived post-event power; intervening conditions recheck at resolution. Source
status uses current incarnation or departure-refreshed LKI. Cast triggers collect
after successful payment; declared attacks use the real declaration batch.
Event-player, source controller, invocation controller and recipient cannot be
interchanged. Optional “may” is an actual decision; unsupported source
explanation blocks a browser control. Compiler presence does not expand
host/search allowlists.

Static grants add independent cascade instances to completed spells of the
source's current controller. A four-seat ledger tracks completed casts and mana
values each turn. Caster and grant provenance freeze at collection, surviving
grantor departure; whole batches preflight IDs, fanout, ledger and retained
payloads atomically. Cast witnesses freeze pre-completion firstness; failed
payments never consume it.

Grouped combat-damage triggers fire once per observer, ability, occurrence and
damaged player, using trigger-time appearance and frozen membership. Opponent
cast prohibition bars opponent spells, not lands, abilities or already-cast
spells; prohibition arising during cost determination or payment preserves a
completed proposal (CR 601.6).

## Admission and policy boundaries

Foundations and attachments remain the only hosted profiles; foundations alone
is search-reviewed. Headless damage, blocking and sacrifice pools are development
admissions whose membership comes from strict profile/catalog JSON.

Develop policy uses permitted observations and public IR features. It may
estimate value but never determines legality or inspects hidden state; new
behaviors need their own public-policy witnesses. Paid accepted-pod prefixes,
private board setups, synthetic IR controls and fixed-index completion games stay
separately labeled in [evidence](game-engine-evidence.md), which owns producers,
proof locations and omissions; [catalogs](game-engine-catalogs.md) own
reproduction.
