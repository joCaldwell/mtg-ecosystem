# Supported engine mechanics

This is the current semantic map, not a claim of general Magic support. Read
the runtime invariants below before editing rules. Exact Rust
IR validation and complete archived source fixtures define executable shapes;
[profiles](game-engine-catalogs.md#match-profile-data) independently admit decks and consumers.
Source/compiler support, private runtime witnesses, accepted-pod prefixes,
completion games, hosting/search review and browser acceptance are separate.

The rules baseline is the frozen [2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Use exact `tests/fixtures` records/provenance under `packages/game-engine`, never
card facts from memory or live refresh. Historical IR11–33 labels are evidence
vocabulary; the only active executable schema is 1. No name/ID/profile dispatch.

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
Untap, upkeep, draw, combat, end and cleanup use recorded transitions. Cleanup
atomically clears temporary effects and marked damage. Concessions are trusted
recorded external inputs processed before ordinary work/query exposure.
Land usage resets only when a new turn begins.

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
commander count/eligibility, color identity, basic-land mana and the selected
frozen ban policy. These are separate from executable support. Shuffle/mulligans
and bottoming are deterministic given trusted seed/input. Seeds and library rows
never enter public streams. Open decklists are a chosen information policy.

Command-zone tax is physical and per caster, incremented only after a completed
command-origin cast. Failed casts restore original zone/incarnation. Graveyard/
exile Commander return is an owner SBA choice after actual movement (CR 903.9a);
hand/library return replaces movement (903.9b). Decline consumes that incarnation's
opportunity. Commander combat damage tracks physical source/recipient across
control and zone changes, independently of life changes and noncombat damage.
Drawing a tucked commander uses an existing public designation with `None` object
handle; visible return choices use `Some(handle)`. One replaced draw completes
once and preserves the next card.
Library-top events preserve order and cannot compose with a conflicting order write.
Drain concessions and revalidate owner choices before exposing queries.
Commander-return decisions are resumable continuations, never priority windows.

Departures preserve original seating and Commander history; all simultaneous
losers leave before cleanup/adjudication. Never resurrect their objects/mana.
Surviving activators retain paid abilities when a borrowed source's owner leaves.
Personal effects for a departed invocation controller skip rather than redirect.

<a id="announcement-priority-and-payment"></a>

## Casting, costs, targets and stack

Stack projections run bottom to top. Saved announcement handles are historical:
never rebind to a later incarnation or authorize a new response. Lower entries
stay completed while the top is announced; reversal restores prior passes filtered
by living seats. Resolving frames retain frozen instances through supported source
departure (CR 608.2m). Only the caster answers targets/casting mana.

Targets are selected before costs. Casting always offers its mana window, even
when already funded. FinishMana attempts full payment and cannot cancel a payable
spell. Reversal refunds only receipt-recorded casting-time mana/taps, preserving
pre-cast mana, combat taps and departures; retained legal mana activations clear
passes. Mixed-color spending needs explicit choices. Nested permanent-mana costs,
colored production and X use exact supported cost shapes, never guessed colors.
X is announced nonnegative and frozen; only stack mana value includes it. Generic
self-cost reduction cannot erase colored costs.

Activated abilities use a separate stack ID/historical source handle; their card
stays in place. Tap-source costs enforce readiness/continuous control, subject to
supported haste. Mana, tap and additional sacrifice costs retain separate guards.
Mana-cost activations announce, offer their mana window and require exact spending;
costs without mana offer no mana opportunity. Failed payment removes the proposal
and optionally reverses only its receipt-recorded mana activations.
Sacrifice costs use ordinary post-activation SBAs; resolution sacrifices/discards
wait for the complete invocation. Additional costs never change mana value.

Object target slots share domain/filter because the request supplies one common
candidate set; unequal per-slot filters reject. Entity/player/opponent/stack
targets retain distinct domains. Only current `StackHandle` → `StackId` mappings
authorize stack targets; exclude the announcing/incomplete entries. Countering a
spell moves it to its owner's graveyard; an ability disappears without moving its
source. No cost refunds or countered resolution. Typed counter filters and
uncounterability are explicit. Aggregating counter membership cannot be put in
an unsupported simultaneous group.

Target legality is checked at proposal and resolution start. Loss of target
legality after completed proposal during payment does not reverse casting
(CR 601.6), including departure before an accepted spending input is consumed;
casting continues through payment without undoing legal mana activations. A paid spell can then fail
to resolve. If all targets are illegal,
skip the entire program, including compensation/draw. Target incarnation cannot
rebind after bounce/reentry. Once resolution begins, use its frozen legal set and
refresh current/LKI facts only at their defined boundaries.

### Target status and tap/untap

`PermanentFilter.tapped: true` requires current tapped status; absent/false means
no restriction, not untapped. Complete-source status must be exactly `["tapped"]`;
empty/null/mixed/unknown statuses reject. Stack filters reject battlefield status.
Capability derivation traverses modes/activations/triggers too.

`BlockingBattlefieldCreature` / `BlockingCreatureTargetV1` is limited to ordinary
instant/sorcery fixed P/T pumps until cleanup. Other opcodes, modes, extra sacrifice
costs, activations and triggers reject. Committed blockers retain status through
EndCombat even after the attacker leaves; their own combat departure clears it.
Damage links alone do not define blocking. Existing target/query predicates use
this rules state without adding a public object field.

| Family | Complete supported shape; other contexts reject |
| --- | --- |
| Ordinary tap (`TapPermanentV1`) | One unqualified creature target, instant/sorcery tap, optionally followed by one mandatory controller draw-one |
| Artifact tap activation (`ActivatedTapPermanentV1`) | Battlefield artifact, finite mana plus tap-source cost, one tap instruction, artifact/creature/land target union; no sacrifice/modes/triggers/extra effects |
| Ordinary untap (`UntapPermanentV1`) | One ordinary nonmodal instant/sorcery, one unqualified permanent target and instruction; no additional costs/qualifiers/extra instructions |
| Tap-source creature untap (`TapSourceUntapCreatureV1`) | Battlefield creature, pure tap-source cost, instant-timed stack, one mandatory creature-filter target at slot zero and one untap instruction |
| Tap-source destruction | Separately validated creature activation and tapped-creature target; preserve cost/readiness and ordinary destruction guards |
| Owner-hand return | Explicit supported ordinary spell or separately fenced activated permanent-return program; source departure never rebinds the target |

Already tapped/untapped is a legal target; produce no false second status event.
Tap/untap does not itself remove combat membership. Self-targeting tap/untap
abilities still pay tap first. Paid activations survive source departure.
Complete Pressure Point, Icy Manipulator, Burst of Energy, Seeker of Skybreak,
Royal Assassin, Temporal Adept and Unsummon source fixtures witness these shapes;
they do not authorize broader combinations. See compiler tests for exact fencing.

<a id="combat-damage-and-state-based-actions"></a>

## Combat, damage and life

Attacker/blocker declarations and damage are typed atomic events. Readiness uses
continuous control stamped at turn entry, reset on control/incarnation change.
Multiple blockers have no damage-assignment order in the pinned rules. Preserve
blocked status after blocker departure and blocking status after attacker departure.
Skip blocker/damage steps only when no attackers were declared; blocking lasts
through EndCombat priority until combat ends (CR 509.1g/511.3), independently of
damage links. First/double strike have distinct participant sets; trample assignment, deathtouch
lethal assignment, actual dealt damage and prevention are different facts.
Vigilance, flying/reach, haste, defender, menace, hexproof/shroud, lifelink and
other admitted keywords use common predicates, never card text in the runtime.

Validate an entire simultaneous damage group before life/history writes. Capture
assignment facts separately from current or latest-departure damage-time source
appearance. Damage-time deathtouch/lifelink and its beneficiary use that current/
LKI source, including equipment departure and simultaneous common-prior facts.
Noncombat damage retains the source incarnation and frozen invocation controller;
that controller is distinct from the source's current or last-known controller.
Old assignment-time keywords cannot fabricate damage-time benefits. Commander
combat history uses only actual combat damage. Direct life gain/loss is distinct.
Run SBAs after the complete program: intermediate life loss/gain is not an early loss.

Replacement/prevention applies once per effect/member, under affected-recipient
choices and APNAP ordering. Freeze the consumed prefix; later source departure
cannot undo it. Remaining eligibility rechecks current facts. Damage allocation,
prevention budgets, zero damage and actual outcomes remain exact. Unsupported
general replacement/layer interactions reject instead of approximating.

Controlled-creature doubling (`ControlledCreatureDamageReplacementV1`) supports
exactly all damage kinds to permanent-or-player, multiplied by two. Partial filters,
other multipliers or narrower scopes reject. Provider's current controller is
compared with source current/exact-LKI creature/controller facts (including
nonbattlefield source applicability, CR 609.7c). Global doubling stays separate.
Public effects omit trusted matching facts. General replacement ordering is not
implied by this family.

## Movement, sacrifice and board-wide effects

Destroy respects indestructible before movement/replacement. Exile, bounce,
sacrifice, zero toughness, legend moves and departures are distinct movement
paths. Removal filters cover admitted type unions, exclusions, controllers and
statuses. All additional types count (CR 205.2b): positive type lists are alternatives,
exclusions reject any matching type; sacrifice subtype lists are conjunctive.

Compensation uses the exact target's current or last relevant public appearance,
not its successor: controller, power and mana value can differ from ownership.
A token has mana value zero. Optional targeted triggers select targets on stack
placement and optionality on resolution; no legal target removes the trigger.

Sacrifice moves a controller's own permanent to its owner's graveyard and is not
destruction. Choose/pay sacrifices after targets. “Sacrifice a creature” may use
the source; “another” cannot. Fling-like effects use paid source-backed LKI power.
Complete Shrapnel Blast lowers one-artifact additional sacrifice through that
same family. Forced opponent sacrifices choose in APNAP order with earlier choices
visible, then move simultaneously. A player with no candidate sacrifices nothing.

Board-wide sets are evaluated at instruction execution, including granted
keywords. Set destruction/exile/bounce and all-recipient damage commit atomically.
Temporary set P/T effects capture members on creation; later entrants are excluded.
Counted quantities use the instruction's actual typed outcomes, not a later board
scan. Destruction counts exclude prevented destruction but include successful
movement replaced by exile. An Anger-style turn-long replacement applies only to
creatures actually dealt its damage; later sacrifice/zero-toughness deaths may be
exiled and do not die. Indestructible prevents destruction before that replacement.
No creature dies trigger is inferred from generic departure.

## Keywords, attachments and temporary effects

Intrinsic/static grants, counters, admitted anthems, Equipment and Auras contribute
to common derived characteristics. Attachment identity, equip/attach costs and
revalidation are explicit. Grants disappear with actual attachment loss; paid
abilities and resolved spell effects retain independent lifetimes. Unsupported
CDA/setting/copying/swapping or general layer combinations must be implemented
before admitting sources that need them.

### Attachment entry and ordering

Resolving admitted Auras enter the battlefield already attached to their exact
legal target, before entry observers capture the resulting state (CR 608.3c).
The typed entry occurrence owns both movement and attachment; its outcome retains
the successor incarnation, target and semantic attachment timestamp. Ordinary
completion still owns stack removal, completion notification and stabilization.

Actual attachments receive a checked monotonic timestamp (CR 613.7e, 701.3c).
Reattaching to the same object changes neither timestamp nor allocator (701.3b);
unattachment and zone changes clear the source's attachment metadata. Target
movement never redirects a grant to a successor incarnation. A reachable pause
before attachment SBAs may retain the historical target until cleanup. Ordering
never comes from object IDs, map iteration or trace retention. Simultaneous
competing attachments require the unimplemented APNAP/player-selected timestamp
ordering of CR 613.7m and reject explicitly.

Attachment timestamps and their allocator are canonical state and replay inputs.
Current formats belong to the [current map](game-engine-current.md); attachment
support does not broaden catalog/hosting/search admission. Continuous control
uses the source family below.

### Continuous permanent control from Auras

`aura_control_v1` binds complete normal pure Enchantment — Aura sources to
battlefield `ControlEnchantedPermanent`, using their current controller
(CR 109.5, 110.2, 613.1b). Nonrecursive `aura_control_domain` returns Creature,
Artifact, Land or Permanent; `aura_control_source` tests presence. Nonserialized
`AuraControlDomain::accepts` uses `has_type` or supported permanent-kind membership.
Creature control uses unfiltered `BattlefieldCreature`, optionally with one bounded
modifier. Pure artifact/land/general control uses `BattlefieldPermanent` with
explicit filter types `[artifact]`, `[land]` or `[]`; other restrictions stay
empty/false. Missing/null/empty-object filters reject. Targets remain
controller-announced slot zero with nonempty selection IDs.

General source requires exactly `{"cls":"permanent"}` and “You control enchanted
permanent.” Source nouns/filters must agree; explicit empty IR types select General.
Unions, qualifiers, source-type changes, extra instructions/abilities/costs and
noncreature modifiers reject. Creature modifiers reject negative static P/T, Flash and Aura-granted
Indestructible.
Names, IDs, amounts, costs and descriptive source labels never dispatch. Creature
targets require Creature Target; other domains require Permanent Removal. All retain Attachments, Aura Control and
cast/layout closure. Serialization retains schema 1 and disjoint arms.

Confiscate witnesses General.
[Evidence](game-engine-evidence.md#atomic-temporary-keyword-qualification) owns
qualification. The [schema harness](../packages/game-engine/crates/contracts/tests/aura-control-schema-probes.mjs)
prepares probes without schema execution. General source requires dependency-aware
runtime and paid-recovery qualification before publication; source admission
grants no catalog or consumer admission.

The current battlefield source, exact attached target incarnation and semantic
attachment timestamp determine authority. Reachable acyclic attachment graphs
reevaluate CR 613.8 dependencies after each attempted operation; independent effects
use timestamps. Same-controller dependencies are a CR-derived inference, without a
located official worked example. Structural attachment cycles reject atomically;
admitted sources lack reattachment/animation producers. Removing authority reveals
remaining effects or default control, independently of ownership. A
source/target successor never inherits the predecessor's attachment. Existing
synthetic control-effect ordering remains separate: an Aura and such a grant on
the same permanent reject rather than invent a shared clock.

Reconciliation finishes each occurrence or complete simultaneous group before
entry/ordinary observers. All cache transitions precede derived keyword facts;
common-prior damage/death facts stay frozen. An actual controller change removes
combat membership and resets readiness (302.6, 506.4); a same-controller grant or
removal does neither. Counters, marked damage, tapping, ownership, default control
and attachments persist. Controlled-target predicates, Hexproof, activation
permissions and anthems use current control; paid invocations retain their own
frozen controller. Queries never repair the cache by mutation.

Departure removes every losing owner's objects before reconciling survivors.
When the final grant ends and no living default/remaining controller exists,
800.4a/c exile is immediate, recorded through typed movement with consumed control
authority and ordinary incarnation/LKI facts. Unsupported synthetic source-exile
dependencies reject. Reconciliation never grants priority, stabilizes, places
triggers or replaces a casting/resolution continuation. Restored-object reversal
reconciles surviving Aura authority without resurrecting departed owners; the
sacrificed-source receipt regression is synthetic because current source admission
rejects sacrifice mana costs. It establishes no paid sacrifice/recovery claim.
No new snapshot/public/query format or playable/consumer admission follows.


`indestructible_v1` supports intrinsic creature/artifact/enchantment/land keywords.
Its separately fenced equipped grant requires Artifact + exactly Equipment subtype,
Equipped scope, zero P/T, exactly Indestructible and no restrictions. Other scopes,
source kinds, mixed keywords/stats, tokens and unsupported grants reject. Ordinary
existing attachment grants retain their own supported shapes.

Destruction prevention records `DestructionPrevented` with no false movement.
Indestructible blocks lethal/deathtouch destruction SBAs but not damage, lifelink,
triggers, sacrifice, exile or zero toughness. Completed SBA checks consume the
current deathtouch window for protected survivors too, atomically before another
check/priority; old nonlethal deathtouch damage cannot kill after protection loss.
Ordinary lethal marks remain actionable until cleanup. Fresh deathtouch reopens a
window. Simultaneous destruction consumes common-prior keyword facts, so a granting
Equipment leaving earlier in event order cannot remove protection mid-event.

| Temporary spell capability | Supported program |
| --- | --- |
| `temporary_indestructible_spell_v1` (+ `indestructible_v1`) | One unqualified creature, sole Indestructible grant until cleanup |
| `temporary_flying_spell_v1` (+ `flying_v1`) | Same pure one-target program granting Flying only |
| `temporary_deathtouch_spell_v1` | Same fenced pure Deathtouch family, with current/latest-departure source authority |
| `compound_indestructible_spell_v1` | One atomic grant of fixed signed P/T and Indestructible on the same one target; parameterized source-backed amounts |

The original pure subset capabilities reject duplicate/mixed grants, modes,
filters, additional costs, activations/triggers and mixed instructions. Reviewed
new combinations use the general family below; subset meanings remain unchanged. Effects capture exact target
incarnation, survive spell-source movement, union with intrinsic/static grants
and expire atomically with marked-damage cleanup. Historical source-keyword
validation accepts only validated grant vocabulary; replay still proves occurrence,
latest appearance and duplicate/departure invariants.

### Shared temporary creature keyword family

`temporary_creature_keyword_program_v1` adds reviewed combinations through the
existing grant/damage/cleanup machinery. One complete ordinary nonmodal instant/
sorcery has exactly one unfiltered or controller-only battlefield-creature target at slot 0, a
nonempty selection ID and controller choice at announcement. Its program is one
`GrantKeywords` targeting slot 0 until end of turn, with an optional shared
`CreatureStatModifier { power, toughness }`. Pure grants omit the modifier on
serialization, accept absent or null as None, and preserve source-block provenance.
A nonnull modifier requires both exact signed quantities, including zero and
values beyond machine integers, denies unknown fields and requires source block 0.
The old `ModifyPowerToughness`/grant sequence rejects in either order; standalone
`ModifyPowerToughness` remains supported in its existing scope.

The combined grant creates one continuous effect with one captured target
incarnation. CR 611.2c and 613.5–6 apply all its keyword and P/T parts to that
same object across layers. Departure LKI must observe the complete effect,
including during a CR 104.3a concession; separately created stat/keyword effects
cannot satisfy this contract.

Keyword vectors are nonempty, duplicate-free and limited to Flying, Deathtouch,
Indestructible, Lifelink, Hexproof, Haste, Trample and First Strike; serialized order
is retained. Pure Lifelink/Hexproof/Haste/Trample/First Strike, mixed vectors and
modifier-plus-Flying/Deathtouch/Lifelink/Hexproof/Haste/Trample/First Strike require
the general capability. Haste, Trample and First Strike reuse existing readiness
and combat-damage rules (CR 702.10, 702.19, 702.7); this vocabulary extension adds
no opcode, capability or runtime mechanism.
Existing unfiltered singleton Flying/Deathtouch/Indestructible and modifier-plus-singleton
Indestructible retain their old temporary/compound subset capabilities; the general
capability is spare and rejects on those shapes. Every granted keyword still
requires its primitive capability; modifiers require `temporary_power_toughness_v1`
and unfiltered targets require `creature_target_v1`. Controlled targets require
`attachments_v1` and the general family capability, even for an otherwise old
subset shape. Hexproof also requires `attachments_v1`; an unfiltered Hexproof
program therefore needs both target capabilities. No schema/catalog/pool migration.

"You control" means the invocation controller's current control, not ownership,
at announcement and resolution. It remains part of the shared target binding;
mixed plain/controlled uses of one selection ID reject. Bare controlled pumps
remain unsupported outside this complete keyword family. Hexproof uses existing
targetability: opponents cannot target the protected creature, its controller
can, and a previously announced opposing target is checked again at resolution.
Resolved grants retain their captured incarnation until cleanup; later control
changes do not end the grant (CR 611.2c).

Contracts' borrowed `TemporaryCreatureKeywordProgram` descriptor and
`temporary_creature_keyword_program` / `supported_temporary_creature_keywords`
recognizers share the complete structural predicate; the definition validator or
caller additionally checks ordinary spell kind/context. The existing compound
helper remains a narrow singleton wrapper. Additional costs, modes, activated/
triggered contexts, filters, multiple/nonzero-slot targets, other durations,
split/reordered/extra operations, duplicate/unreviewed keywords and compound
source block 1 reject. The compiler consumes the whole sole typed sentence
`[Pump, GainAbilities]` only in an empty ordinary program with no bindings, costs
or modes, at source block 0. Both complete source target expressions pass through
the existing exact plain/controlled binder and must share selection ID, domain,
count 1 and slot 0; both durations must be end of turn. It never joins distinct
sentences or scans arbitrary instruction pairs. Pure-grant and set-grant lowering
retain their own paths; decoding independently rejects unconsumed clauses.
Complete frozen sources witness the family; card identities never dispatch it.

Synthetic typed controls preserve the original five-keyword ordered-vector coverage
and add bounded representative Haste/Trample/First Strike vectors in both forms and
domains, including mixed and reversed orders. They cover missing/spare capabilities,
signed/large quantities, preserved legacy provenance and unsupported/context negatives.
[Schema probes](../packages/game-engine/crates/contracts/tests/temporary-creature-keyword-schema-probes.mjs)
retain the prior 230-, 1,657- and 1,955-probe stage counts while converting compound
controls to atomic IR and replacing obsolete split-only mutations with genuine
negatives. The current harness declares 1,999 probes after appended atomic/null/
malformed/split controls; execution and its new hashes require parent qualification.
Historical split-format hashes remain separately labeled and reconstructed for
comparison, preserving exact pure controls rather than claiming all old payloads
are unchanged. The separate 409-probe set harness and its payload bytes are unchanged
by the shared Rust type rename.
`--prepare-only` checks
construction without Ajv/schema execution; explicit `--schema PATH --ajv-root
NODE_MODULES_DIRECTORY` validates exported schema and uint32 using existing Ajv2020.
No install/generation is implicit. Qualification is source-bound in
[engine evidence](game-engine-evidence.md#current-temporary-keyword-family-qualification).
This foundation expands no playable pool, policy, hosting/search or consumer acceptance.

### Temporary set keyword families

The creature (`temporary_creature_set_keyword_program_v1`) and permanent
(`temporary_permanent_set_keyword_program_v1`) families share complete ordinary
instant/sorcery programs with no targets, modes or additional sacrifice cost. The
stack instruction program is exactly one `GrantKeywordsSet` with source block 0
and an optional typed `CreatureStatModifier { power, toughness }`. It creates one
continuous effect lasting until end of turn; a separate `PumpSet`/grant pair
rejects. Creature selectors are `ControllerCreatures` (current creatures
controlled by the frozen invocation controller) and `AttackingCreatures`
(current attacking creatures, regardless of controller), with optional exact signed
P/T parameters. `ControllerPermanents` selects every current battlefield permanent
controlled by the frozen invocation controller, including noncreatures; its modifier
must be absent/None, and even Some(0/0) rejects. `TemporarySetKeywordProgram` and
`temporary_set_keyword_program` expose the shared selector, optional modifier and
reviewed vector. `temporary_creature_set_keyword_program` and
`temporary_permanent_set_keyword_program` are disjoint narrow wrappers. Identities
and quantities never select mechanics.

The family reuses `supported_temporary_creature_keywords`: Flying, Deathtouch,
Indestructible, Lifelink, Hexproof, Haste, Trample and First Strike, nonempty and
without duplicates. Every keyword requires its existing primitive capability;
Hexproof uses `attachments_v1`, and First Strike serializes as `first strike`
with `first_strike_v1`. Every set grant additionally requires `board_wipes_v1`
for the shared set machinery, exactly the selector's family capability, normal
layout and its ordinary cast-kind capability. The permanent selector never infers
the creature-set label; the real protective pair requires Attachments and
Indestructible in addition to Board Wipes and its ordinary/family capabilities.
Targeted subset/general labels never admit this shape. The optional modifier requires no targeted P/T capability. Standalone
`PumpSet` retains its existing behavior and Board Wipes capability.

The instruction captures one full current ordered set and creates all properties
atomically. CR 611.2c fixes that membership when the effect begins; CR 613.5–6
applies its keyword and P/T parts to the same objects across layers. A shared
capture followed by separately created effects is insufficient: departure LKI
must never observe only half of this effect. Membership and handles bind exact
battlefield incarnations; later entry, departure/reentry or control changes
never dynamically reselect the effect (CR 608.2h, 611.2c).
Cleanup expires it through the existing temporary-effect path. Restore validates
the exact source opcode, modifier, keywords, source block and historical member handles;
authenticated input replay checks the captured membership. The new controller
selectors reject in all other movement, damage, bare-pump, mode, activation and
trigger programs, and in cost-conditioned, additional-cost, targeted or malformed
programs. Exact complete Heroic Intervention source lowers its sole pure GainAbilities
sentence to ControllerPermanents with Hexproof then Indestructible, no modifier,
and its unchanged {1}{G} ordinary instant cost. This source-backed protective pair
is separate from synthetic vectors of the other reviewed keywords.

Frozen Titanic Ultimatum, Overrun, Stampede, Make a Stand and Burst of Speed
sources witness the two creature selectors and optional modifier. The compiler consumes
one complete typed sentence with no bindings: a pure gain, or exactly a fixed
signed pump followed by a gain with matching reviewed selectors and duration.
It never coalesces separate sentences, later arbitrary programs or cost-conditioned
clauses. Dedicated compiler and contracts tests cover exact costs/order/capabilities,
old split-IR rejection and whole-source negatives.
[Set schema probes](../packages/game-engine/crates/contracts/tests/creature-set-keyword-schema-probes.mjs)
retain their original 409 controls and payloads. The
[separate permanent controls](../packages/game-engine/crates/contracts/tests/permanent-set-keyword-schema-probes.mjs)
cover the new selector, disjoint family/primitive closure, signed/zero modifier
negatives and all other opcode/context fences. These are bounded synthetic IR
controls; `--prepare-only` constructs them without schema execution. Source-qualified tests/schema evidence is indexed in the evidence guide; acceptance
never transfers to later source.
The targeted atomic-grant probes remain separate; their prior split-format 1,955-probe inventory has its own historical identity. This family
changes no catalog bodies, pool/profile/policy,
hosting/search or consumer admission; Card IR remains schema 1.
<a id="abilities-targets-and-triggers"></a>

## Trigger families and event witnesses

Collect triggers from actual typed occurrences, wait through SBAs/Commander
choices, then place APNAP, selecting targets/order at the proper boundary. Freeze
controller, event-player, source incarnation and family-specific witnesses.
Intervening conditions check at collection and resolution. Source status uses
current incarnation or departure-refreshed LKI bounded by live continuations;
never retain an unbounded permanent LKI archive. Cast triggers collect after
successful payment; declared attacks use the real declaration batch. Ordinary
step, death, ETB, draw/discard and combat/noncombat-damage families each retain
strict source/program validation.

Live battlefield static grants add independent cascade instances to completed spells
of the source's current controller using disjoint mana-value, first-cast or
first-from-exile predicates. A constant four-seat ledger counts every successful
cast, including observer-free and resolution casts, and resets only on a new turn.
Intrinsic indexes and grant-source incarnation/static indexes are distinct identities.
The cast spell remains rules source; caster and grant provenance freeze at collection,
surviving later grantor departure/control changes. Whole batches preflight IDs,
fanout, ledger and retained payloads atomically. Cast witnesses freeze pre-completion
firstness; failed proposals/payments never consume it. Complete frozen Nexus and
Wild-Magic Sorcerer reuse canonical cascade; Yidris remains unsupported.

Later finite extensions cover opponent-damage conditions, optional controller
draws, event-player discard/sacrifice, filtered sources, controller discard,
damage-quantity draws, attacking-creature keywords/Snow and event-time appearance.
Presence in the compiler does not expand host/search allowlists. Event-player,
source controller, invocation controller and recipient cannot be interchanged.
Optional “may” is an actual decision; unsupported source explanation blocks a
browser control even if a boolean exists in the schema.

Grouped power-qualified combat damage (`GroupedCreatureDamageTriggerV1`) groups
positive actual player damage by observer, ability, genuine occurrence and damaged
player. Multiple qualifying creatures hitting one player trigger once; two players
trigger twice; first/double-strike occurrences are separate. Qualification uses
trigger-time battlefield appearance and each damager's own base power. Prevention,
assignment and life changes cannot substitute. The full canonical group/member
appearance is frozen, surviving later departure/token cessation. Current additive
P/T/counter semantics make definition power the supported base-power authority;
other P/T layers require extension before admission.

`OpponentCastProhibitionV1` follows current source controller and retained turn
owner, even on a turn with no active player. It bars opponent spells from all
admitted zones, not lands/abilities/triggers or already-cast spells. Revalidate
before cost stage with normal rollback; prohibition arising during determination/
payment of costs preserves a completed proposal (CR 601.6).

## Admission and policy boundaries

Foundations and attachments remain the only hosted profiles; foundations alone
is search-reviewed. Headless damage and blocking profiles and the evolving
sacrifice pool are development admissions. The blocking pool admits status,
tap/untap/return and temporary protection/flight/compound sources; the sacrifice
pool admits complete Shrapnel Blast, intrinsic indestructible sources, Plate and
Collar through ordinary mechanics. Exact pool/deck membership comes from strict
profile/catalog JSON; do not infer it from a historical definition count.

Develop policy uses permitted observations and public IR features, target domains,
amounts and costs. It may conservatively estimate value but never determines
legality or inspect hidden state. Protection, equip improvement, defensive untap,
flight and damage-source behavior need their own narrow public-policy witnesses.
A paid accepted-pod prefix, private board setup, synthetic typed IR control and
fixed-index completion game must remain separately labeled in evidence.

See [source-qualified evidence](game-engine-evidence.md) for current producers,
proof locations and omissions, [catalog commands](game-engine-catalogs.md) for
reproduction.
