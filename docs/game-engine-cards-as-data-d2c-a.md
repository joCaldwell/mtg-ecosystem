# D2c-a: compositional beginning-of-step triggers

D2c-a follows merged D2b `c691800a59ef4f2763c3f21909e2fa5b534a5f0b`.
It adds a capability, not a card-shaped instruction or a new legal match profile.
Independent review remains the parent's next step; the implementation agent did
not delegate, create a PR or merge this work.

## Exact supported subset

IR v21 adds `StepBeginningTriggerV1`. `TriggerCondition::StepBegan` composes an
independent `BeginningStep::{Upkeep,Draw,End}` with
`TurnRelation::{Controller,AnyPlayer,Opponent}`. The source mirror accepts only
`phase` values `upkeep`, `draw-step`, `end-step` and exact `whose` values `your`,
`each-player`, `each-opponent`. Unknown fields, relations and phases fail for the
whole card. Every other ability on the card still must compile and validate.

Only battlefield-functioning sources are supported. Ordinary creatures,
artifacts and enchantments may carry the same complete triggered program.
Existing Aura-return restrictions remain; lands are not added as trigger sources.
Programs reuse the existing mandatory trigger instruction vocabulary: controller,
target-player draw/life and each-opponent life; fixed source damage; supported targeted
permanent destruction, targeted temporary P/T, fixed defined creature tokens,
+1/+1 counters and the existing supported mandatory sacrifice/recursion programs.
All target/filter/quantity and capability-closure checks still apply. This is a
finite whitelist, not admission of every spell instruction as a trigger.

Step programs reject optional destruction, optional recursion, optional damage,
additional casting costs, modes, intervening conditions and death-event
recipients. Compiler unknown-field checks also reject optional groups/payments,
other optional effects, unsupported player relations and graveyard-functioning
forms. Event-player recipients (`that player`), source-status checks (including
Howling Mine), cast/attack/damage/draw-event triggers, delayed triggers, combat or
main phase beginnings, library selections and optional payment are excluded.
Neither hosting nor search's reviewed capability set includes the new capability.

## Collection, entry actions and evidence

Rules come from the pinned 2026-09-25 Comprehensive Rules file, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`, at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-11fdee9d9b50bb8d-2Y0CLl/rules-20260925.txt`.
CR603.2b collects a beginning-step trigger from the actual committed
`TurnTransitioned` occurrence, before `Frame::EnterStep` executes. Repeated
`advance` calls and restored frames cannot manufacture another boundary.
CR503.1a/504.1/513.1/117.5 retain normal entry work, then SBAs and APNAP placement,
then priority. A draw-step trigger is pending before the normal draw; it is placed
only after that draw and any commander draw replacement. An empty library or
lethal life total loses at stabilization after the complete invocation, including
its later ordered effects (CR121.4/704.5b/608.2c).

`TriggerEventWitness` replaces the pair of optional movement/token fields and the
stack/event's assumed object subject. It has exactly three shapes: `Moved`,
`TokenEntered`, `StepBegan`. Step evidence freezes turn number/step, turn owner and
optional active player, and separately retains the actual occurrence ID. Pending
triggers, placed invocations and trusted collection events carry this evidence.
Movement and token events retain their exact incarnation-bearing object subject;
step events have no object subject. Both pending and stack witness shapes are
validated. Restore additionally proves the full state and trusted evidence by
independent accepted-input replay, including occurrence paths and frozen facts.

Source facts, controller and historical handles freeze at collection. Source
removal does not erase a surviving controller's pending or resolving invocation.
Controller departure removes its pending/stack abilities under CR800.4a/d. Atomic
batch staging preserves turn, mana, handles, IDs and evidence on overflow; typed
stack-capacity faults remain distinct from rules-defined game outcomes.

Under CR800.4j the departing active player's turn continues **without an active
player**. Its real upkeep/draw/end boundaries still occur. Unqualified `each`
step patterns match; controller/opponent relations require an actual active
player and do not match that boundary. Frozen turn owner remains evidence and
the APNAP anchor, not a substitute event player. Priority goes to the next living
seat. No personal normal draw is performed for the departed player. CR800.4k
still skips that player's later turns.

## Source and representation review

The read-only archive is
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl`,
SHA-256 `d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`,
parser `327b3360ea8e5cffb75b136223e75a348e903cbe`. Accepted archive extraction
preserves the exact record bytes and records each archive line in provenance.
The new small catalog contains eleven complete source programs; Howling Mine and Master of the Feast are
retained whole-card negatives, excluded from that catalog.

| Source | Archive line | Program witness |
| --- | ---: | --- |
| Phyrexian Arena | 36046 | Actual cast, correct/wrong upkeep, ordered draw/life, empty library and lethal life |
| Moroii | 9855 | Flying and mandatory controller-upkeep life loss retained by compiler |
| Nyx-Fleece Ram | 13540 | Enchantment creature, controller-upkeep gain; ordering and wrong-step control |
| Celestial Force | 20928 | Each-upkeep gain, multiple controllers/APNAP and inactive-player turn |
| Baleful Force | 36673 | Each-upkeep ordered draw/life; APNAP placement |
| Fountain of Renewal | 33636 | Ordinary artifact upkeep gain; separate paid sacrifice-draw ability retained |
| Fretwork Colony | 22621 | Source counter then life loss; can't-block static retained |
| Verdant Force | 18409 | Each-upkeep defined Saproling token |
| Magmatic Force | 35394 | Mandatory any-target source damage and target announcement |
| Bloodgift Demon | 34730 | Shared target-player draw/life binding |
| Dogged Pursuit | 22983 | Controller end step, each-opponent loss then controller gain |
| Master of the Feast (negative) | 303 | Each-opponent draw is explicitly unsupported at runtime and now rejected before admission |

All twenty initially reported candidates' full text and AST are retained in the ignored
`new-admission-source-audit.json`. The eleven fixture programs are tested at the
compiler boundary, and representative program families execute through native
source-backed private scenarios. These are not claims that the new cards played
in the twelve unchanged legal profile batches. Synthetic IR boundary tests cover
draw/end/opponent predicates, artifact program composition and malformed shapes;
they are labeled separately from archived-source witnesses.

Snapshot version advances **18 → 19** and demo bundle version **19 → 20** because
existing trigger witness serialization changes, including old movement and token
triggers. The public projection/context/sidecar versions remain 16/16/15.
Older expected catalog IR/digests are not regenerated: unit trigger conditions
retain their representation and all existing definitions remain byte-identical.
Only the new catalog IR is generated. Old trigger tests now inspect the typed
witness's subject rather than an invented object for all event kinds. Unknown
future-IR probes advance from 21 to 22, preserving their rejection requirement.
One historical source-kind negative becomes a supported-program positive plus a
wrong-functioning-zone negative: its arbitrary noncreature enchantment exclusion
was the template restriction this task explicitly removes. No capacity, source
integrity, unsupported-semantics or replay guard is relaxed.

## Historical digest adjustment

The first complete gate passed 958 Rust tests and failed one simulator assertion
against the old trigger digest baseline (18 existing diagnostics ignored; Node
had not started). `engine-check.log` and its nonzero status remain unchanged.
The full release CLI demo subsequently passed all native outcome/recovery checks,
all 540 retained trace-mode executions and independent bundle verification.
Its exact executable, source manifest and 2.9 GB replay bundle are retained.

The external `representation-baseline-proof.json` verifies all **173/173** final
snapshot hashes against their canonical State bytes. It then reverses only the
movement/token witness encoding and the exact change in retained trace-byte
charge. Collection events are charged both in their trace record and duplicate
event storage, so the charge delta is twice the serialized event-size delta.
This reproduces **every** frozen D1b-3 digest. Exactly **37** current digests
change; all other fields, inputs, outcomes and capacities are untouched by this
comparison. The original baseline is retained as `baseline-original.json`.
Only after that proof was `d1b3-history-scenario-digests.json` regenerated.
The Rust and Node equality assertions remain. Six Node object-subject assertions
now derive the same incarnation from typed movement/token evidence and reject a
leftover legacy `subject` field. No compatibility field is emitted.

## Verification

Verification is in progress. Pre-freeze focused release checks pass fifteen
step-trigger tests and the compiler/contracts checks. The initial corpus measure
was 1,924/32,116 (5.99078%), twenty additions and zero losses against D2b's 1,904.
A runtime witness then exposed unsupported each-opponent drawing in Master of
the Feast. Its complete source and IR now reject; that failed attempt remains
retained and is not counted as accepted coverage.
The settled-source results, exact identities, failures and evidence paths will
replace this paragraph after the required complete verification sequence.
