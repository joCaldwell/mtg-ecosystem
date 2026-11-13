# Temporary characteristics and damage replacement/prevention

> Historical capacity proofs: D1b-2 supersedes this slice's per-profile bounds
> with the [generic capacity policy](game-engine-capacity-policy.md). The original
> arithmetic and evidence below describe the historical release only.

Current [damage-source appearances](game-engine-controlled-damage-replacement.md)
separate immutable assignments from current/LKI damage-time keywords and static
replacement predicates. The historical evidence below retains its source scope.

Frozen contract for the next bounded P4 slice, prepared October 15, 2026.
The parent, runtime and independent rules review approve the six source-backed
positives, three paired negatives, exact five configurations, data/decision
interfaces and format versions below. Implementation follows this frozen contract. Final checks and retained artifact
identities are recorded in [the progress ledger](game-engine-progress.md).

## Source, vocabulary and admission

Use complete historical schema-3 archived records, preserving their original
source/AST/binding bytes and diagnostic metadata. The Rust compiler must prove
all new event scopes independently; historical unbound-scope diagnostics are
neither executable approval nor a reason to discard complete supported syntax.
No parser edits, fresh card refresh, name dispatch or text execution fallback.

| Positive | Index | Oracle identity | Source SHA-256 |
| --- | --- | --- | --- |
| Giant Growth | 13072 | `5748ebf1-24e3-499d-ab7c-c2cebd462a24` | `39f0e631d644137618c46213840c570637bc446f50e072c0aea567322c6b0656` |
| Disfigure | 18009 | `77eafe49-b9c5-461d-89c5-cec217dd2974` | `c48a5295c40e24aa708efbbb08a5d737c375942c5d1161fe622eca7a4e555d10` |
| Fog | 5988 | `27e9db49-7af7-4bef-ad4c-bf5dfb92030d` | `429a99f6401f0c807cd153650297d19fa0458d2851d62ea0c1587c2618134d7e` |
| Samite Healer | 22519 | `95a0ca48-d924-47f4-86ed-42c673ee778c` | `27d9bf0d40404ffc25215c54808c6342bd36bc40c851115a40be64bf98ee1834` |
| Furnace of Rath | 15657 | `68715465-6cf9-4006-87e9-31f227fe9ed3` | `c25586ad2af602cc8f2ee5753ab7a3f1055e8ad31ea442d237ff8d80762db7f2` |
| Yoked Ox | 3931 | `19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8` | `e2cbc722e77cbf6545235079ca7e1e4cd26fc02a8c03be382e240d7f848dc2b7` |

Growth is a G instant giving its one creature target +3/+3 until end of turn.
Disfigure is a B instant giving its one creature target -2/-2 for that duration.
Fog is a targetless G instant preventing all combat damage for this turn.
Healer is a 1W 1/1 with a tap ability creating one finite point of prevention
for one any-target recipient this turn. Furnace is a **1RRR** non-Aura
Enchantment whose battlefield static ability doubles each source's damage to
any permanent/player. Ox is a W 0/4 with a complete empty ability list.

At the original source boundary, the positive catalog has exactly 30 definitions:
the previous 24 trigger cards plus these six. Thirty-five source records retain
those positives, existing Flame Rift/Deathgreeter negatives and three then-new
complete negatives: Righteousness (9534, blocking-creature target restriction),
Healing Salve (21291, modal gain/prevention choice), and Gratuitous Violence
(18654, creature-source/controller restriction).

These exclusions qualify that historical source boundary. Later work completes
all three; the [blocking-target extension](game-engine-blocking-targets.md)
records Righteousness support, and the [current map](game-engine-current.md)
links the other extensions. The existing effects admission pool remains separate.
Every filter, mode and clause must still compile completely or the whole card
must reject.

IR v9 adds fixed signed additive P/T modification, global combat prevention,
finite recipient-bound prevention, Enchantment permanent casting and the finite
all-source/permanent-or-player static damage doubler. Preserve the complete
abstract any-target and permanent/player meanings while only creature/player
damage recipients inhabit these exact setups. No general layer dependencies,
setting/switching stats, counters, keywords, redirection, unpreventable damage,
prevention after-effects, modal casting, regeneration, zone replacements,
copy/token generation or expanded trigger families are admitted.

Recovery versions: snapshot 15, bundle 16, projection/context 13, query sidecar
12. All new effect/application/choice identities must be typed, checked,
serializable, replay-proved and distinct from physical cards/incarnations,
trigger IDs and stack IDs. Public choices use viewer-scoped opaque handles;
internal event IDs, source facts and application histories remain host-only.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf),
with independently verified pinned TXT SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules: 104.3a, 109.2, 120.1–8, 506.4, 510.1–3, 514.2–3,
611.2a/c, 611.3a/b, 613.4c, 614.4–7, 615.3–7, 616.1e/f–2,
704.3–5, 800.4a/e/m and 903.10a. Rule 800.4i concerns player LKI;
it does not establish duration persistence.

## Frozen exact tables and bounds

All objects have incarnation 0; battlefield objects are untapped, with initial
creature continuous-control eligibility. P0 begins turn-1 precombat main with
priority, original seating, 40 life per seat, no control grants, empty pools,
stack, pending triggers, Commander histories and resolved-effect tables.
Initial battlefield setup does not trigger ETB abilities. Physical 0 is the
only designated Commander. Ordered libraries are ascending IDs within each seat.

| IDs | Seat | Definition | Zone |
| --- | --- | --- | --- |
|0|0|Isamaru|battlefield designated Commander|
|1|0|Yoked Ox|battlefield|
|2|0|Samite Healer|battlefield|
|3|0|Prodigal Sorcerer|battlefield|
|4|0|Blood Artist|battlefield|
|5|0|Giant Growth|hand|
|6|0|Fog|hand|
|7|0|Disfigure|hand|
|8|0|Lightning Bolt|hand|
|9,10|0|Forest|battlefield|
|11|0|Plains|battlefield|
|12|0|Swamp|battlefield|
|13|0|Mountain|battlefield|
|14|0|Forest|ordered library|
|15|0|Plains|ordered library|
|16,17|1|Grizzly Bears|battlefield|
|18|1|Samite Healer|battlefield|
|19|1|Blood Artist|battlefield|
|20|1|Unsummon|hand|
|21|1|Stifle|hand|
|22|1|Counterspell|hand|
|23|1|Lightning Bolt|hand|
|24|1|Giant Growth|hand|
|25,26|1|Island|battlefield|
|27|1|Forest|battlefield|
|28|1|Mountain|battlefield|
|29|1|Island|ordered library|
|30|1|Forest|ordered library|
|31,32|2|Furnace of Rath|hand|
|33,34,35,36|2|Mountain|battlefield|
|37|2|Forest|battlefield|
|38|2|Grizzly Bears|battlefield|
|39|2|Blood Artist|battlefield|
|40,41|2|Mountain|ordered library|
|42|3|Blood Artist|battlefield|
|43|3|Mountain|battlefield|
|44,45|3|Mountain|ordered library|

`effects()` uses this base. `effects_furnace()` starts Furnace 32 on the
battlefield. `effects_two_furnaces()` starts both Furnaces 31/32 there.
`effects_borrowed()` instead changes Healer 2 and Sorcerer 3 to owner P2,
retaining default controller P0. `effects_low()` uses the single-Furnace setup
and starts every player at 4 life. Other facts remain exactly the base table.
Owner counts are 16/15/11/4; borrowed counts are 14/15/13/4.

Twelve physical creatures and four Artists bound all death-trigger production
in a nonempty-stack episode by 48. No table ETB triggers, recursion, copies,
tokens, untap effects or reanimation exist; creature recasting requires an
empty stack. At most nine instant cards, one permanent proposal, two Healer
activations and one Sorcerer activation add thirteen ordinary entries.
The conservative combined bound is 61, below the reviewed 128-entry capacity.
Two Growth cards, one Disfigure, one Fog and two eligible tapped Healers produce
at most six live resolved effects between cleanup operations; use a checked
reviewed cap of sixteen. Recasting a Healer does not bypass continuous-control
eligibility. Monotonic identity exhaustion is a resource fault, never a game
outcome or silent dropped effect.

## Derived characteristics, incarnation and cleanup

Base card characteristics remain immutable. Apply admitted signed fixed deltas
in additive layer 7c to the bound target incarnation. Multiple additions commute
in this subset; no general timestamp/dependency or other-layer claim follows.
Current observations, combat assignment, lethal/zero-toughness SBAs, consumed
pre-event facts, trigger look-back and damage-source LKI use the same evaluator.
A source that leaves retains its actual derived characteristics, not invented
or printed-only facts. Replace previous checks equating all source facts to
static definition characteristics with independently proved current/LKI facts.

Negative power remains a real characteristic; combat assignment is zero when
power is nonpositive. Disfigure makes Ox -2/2 and it survives while assigning
no damage; Furnace cannot resurrect a nonexistent zero-damage event. Toughness
at or below zero uses the non-destruction SBA; prevention cannot save that body.
Modified positive toughness with lethal marked damage uses the separate lethal
SBA. Both actual grave entries may create Artist triggers and Commander returns.
A pumped Sorcerer still deals the fixed one damage in its ability; power is
source metadata rather than the instruction's damage amount.

Resolved modifiers bind the exact target incarnation and do not follow a
bounce/recast. They continue across control changes of that same permanent.
Resolved Growth, Fog and Healer shields survive their creator/source owner's
departure through their stated duration under 611.2a/c. Furnace's static rule
instead exists only while its exact source is on the battlefield (611.3b).
An effect's creator leaving is not a general deletion rule.

Cleanup removes marked damage and ends all until-EOT/this-turn effects in one
atomic turn-based action before SBAs (514.2). A Growth-modified 1/1 with three
marked damage becomes an unmarked 1/1 and survives; there is no intermediate
lethal state. Effects created in a cleanup priority exception expire at the
next cleanup operation, using the existing repeat-cleanup continuation.

## Replacement order and finite allocation

Fog prevents each combat damage occurrence globally, including future
creatures, but affects neither noncombat damage nor direct life gain/loss.
Healer binds one player or damageable object incarnation and supplies **one
shared point**, not one per source/event. Its source may leave after activation;
the independent ability still creates the shield if its target remains legal.
A fizzled/Stifled ability creates no shield and never refunds its paid tap.

Furnace modifies each positive damage occurrence by exactly two. Separate
Furnaces have separate identities. Each may apply once to an original event
and its modified descendants (614.5); do not repeatedly double to exhaustion.
Recompute applicability after each application (616.1f/616.2). Zero or fully
prevented damage cannot later be doubled back into existence (614.7a/120.8).

The affected player, or affected permanent's current controller, chooses the
next applicable replacement/prevention effect under 616.1. Neither the damage
source controller nor shield creator owns this choice. Choices for multiple
affected players follow APNAP order. A finite shield spanning simultaneous
sources requires allocation by that affected chooser **when applying the
shield**, using the then-modified damage occurrences (615.7). Allocation cannot
be frozen independently on unmodified amounts before the ordering choices.

Bolt 3 with one Furnace and a one-point shield yields 5 for double then prevent,
or 4 for prevent then double. Two Furnaces give 12 from unprevented Bolt 3,
once each. Fog-first leaves a finite shield unused; shield-first consumes it
before Fog prevents the remainder. A later noncombat ping distinguishes those
states. These choices remain resolution/turn-based processing, never priority.

## Applied prefix, concessions and simultaneous actual damage

Three different boundaries must remain explicit:

1. An accepted but unapplied input is a proposed choice. Revalidate it against
   the current rules state after a concession; invalidate/reissue stale choices.
2. Applying a selected replacement/prevention is a rules step under 616.1f.
   Record the transformed amount, used-effect identity and consumed shield
   allocation as an applied prefix. Once applied, later source departure does
   not retroactively undo that prefix. For example a Furnace that already
   changed 3 to 6 cannot disappear from the history merely because its owner
   leaves before the final damage write. Future unapplied applications from
   that absent Furnace are unavailable.
3. Final actual damage to life/marks and Commander history remains simultaneous
   for the complete combat group. No partial recipient damage, early SBA,
   trigger placement or ordinary priority appears between ordering/allocation
   decisions. Retain original occurrences, each applied transformation and
   actual results as separate evidence.

Staging is an implementation technique, not permission to label every previous
application an uncommitted plan and restart it on any departure. Newly rejected
choices or failed application steps are atomic; do not roll back an earlier
successfully applied prefix. Frozen invocations continue after source departure
as already reviewed; damage to a missing recipient is skipped rather than moved
to a new incarnation/player. Drop a removed recipient's still-unapplied choices
without redirecting its shield to another recipient. Previously consumed shield
quantity remains consumed even if that recipient subsequently departs; it is not
refunded. A remaining legal chooser
is determined from the current affected entity; no departed seat is recycled.

Before combat assignment, ordinary departure removes objects from combat and
revalidates assignment using 506.4/510.1/800.4e. **After assignment has been
frozen**, retain its original occurrences, source LKI, quantities and recipients;
do not re-power or reassign the surviving group after a source departure.
This is an explicit cross-rule inference: 510.2 requires assigned damage to be
dealt; 120.2/120.7 permit historical object sources; 506.4 removes a creature
from combat but gives no instruction to undo its already-assigned damage.
Missing object recipients cannot receive damage under 120.1a/109.2; departed
players cannot receive personal effects. No stronger claim about an explicit
special concession sentence in 510 is made.

Only actual positive surviving combat damage advances Commander history.
Prevention creates no damage/death trigger; amplified actual lethal damage
creates ordinary deaths and the reviewed look-back/APNAP pipeline. Direct
Artist life loss/gain bypasses all damage replacement/prevention. Source,
recipient, applied identities, affected chooser and remaining budget cannot
be inferred from final life deltas.

## Required evidence

The frozen data and decision interfaces follow below. Prove complete-source
lowering and rejection of the
three paired negatives, with nearby rehashed source/AST/scope mutations and
older-schema vocabulary downgrade rejection. Keep earlier pools and hashes.

Positive natural proofs cover signed/additive/current/LKI characteristics,
negative-power combat, zero-toughness versus lethal death, incarnation and
caster departure, atomic cleanup, Fog combat/noncombat distinction, partial
and shared finite shields, fixed tap cost and source removal, actual Furnace
casting/static source removal, both one-shield orderings, two distinct Furnaces,
Fog-versus-budget order and actual death/trigger/Commander outcomes.

Independently test wrong/stale/foreign chooser/handles, allocation sums and
source attribution, repeated effect application, unapplied input invalidation
versus applied-prefix persistence, assigned-source departure versus missing
recipient, no partial simultaneous damage writes, derived-source/budget/cursor
forgeries, nested resolution/trigger continuations and repeated cleanup.
Compare budgets, traces, forks, restore/replay, query recovery and historical
families; complete native debug/release/Clippy, source-to-runtime evidence and
fresh retained offline rebuilding before accepting this slice.


The following exact interfaces are adopted from the runtime/data review.
Private struct layout may be refactored without changing serialized semantics;
public field names, distinct identities, rules boundaries and proof obligations
are frozen. No blanket transaction restart or omitted choice is permitted.

## Frozen source/IR and data interfaces

Use the compiler proposal verbatim, adding Disfigure/Ox as approved:

```rust
pub enum Duration { UntilEndOfTurn }
pub enum Instruction {
    ModifyPowerToughness {
        target_slot: u32, power: Quantity, toughness: Quantity,
        duration: Duration, source_block: u32,
    },
    PreventCombatDamage { duration: Duration, source_block: u32 },
    PreventNextDamage {
        target_slot: u32, amount: Quantity,
        duration: Duration, source_block: u32,
    },
    // Existing variants unchanged.
}
pub struct StaticAbility {
    pub functioning_zone: FunctioningZone,
    pub effect: StaticEffect,
}
pub enum StaticEffect { DoubleDamageToPermanentOrPlayer }
```

`Definition.static_abilities` is default/skip-empty. Add Enchantment kind with
ordinary main-phase permanent casting. New capability distinctions cover
resolved additive P/T, prevention-all-combat, finite damage budget, damage
quantity replacement and enchantment casting. No name-based runtime dispatch.
Samite uses existing AnyDamageTarget and existing targeted tap-announcement
before payment. Prodigal still deals fixed1 damage even when pumped; its derived
characteristics are damage-source facts, not its ability's damage expression.

## Internal identities and effects

All IDs below are host-only, checked monotonic counters or transaction-local
ordinals. Old profiles serialize none of their empty/default fields.

```rust
pub struct EffectId(pub u64);
pub struct DamageTransactionId(pub u64);
pub struct DamageId(pub u32); // one original source/recipient occurrence

#[serde(tag = "effect", rename_all = "snake_case")]
pub enum EffectKey {
    Resolved { id: EffectId },
    Static { source: ObjectId, ability: u32 },
}
pub enum DamageKind { Combat, Noncombat }

struct EffectOrigin {
    stack_id: StackId,
    source: ObjectId,
    controller: PlayerId,             // remains meaningful after departure
    source_handles: Vec<ObjectHandle>,// four saved public historical references
    cursor: u32,
    source_block: u32,
}
struct ResolvedEffect {
    id: EffectId,
    origin: EffectOrigin,
    expires_in_cleanup_of_turn: u64,
    kind: ResolvedEffectKind,
}
enum ResolvedEffectKind {
    ModifyPowerToughness {
        target: ObjectId, power: Quantity, toughness: Quantity,
    },
    PreventCombatDamage,
    PreventNextDamage { target: TargetEntity, remaining: Quantity },
}
```

Recompute static Furnace instances from current battlefield definitions; their
identity is source incarnation + ability ordinal. Do not materialize a
source-independent Furnace effect on entry. Recompute base characteristics plus
active additive modifiers on every reference query. Growth/Disfigure capture a
fixed incarnation. Resolved effects last for the duration even if their creator
or source leaves (611.2a/c); static Furnace ends with its functioning source
(611.3b). Do not cite800.4i for duration persistence; it concerns player LKI.

## Damage preparation ledger and continuations

```rust
struct PreparedDamage {
    id: DamageId,
    source: ObjectBefore,        // derived current/frozen assignment/LKI facts
    controller: PlayerId,
    recipient: DamageRecipient,
    original_amount: Quantity,
    amount: Quantity,
    applied: Vec<EffectKey>,
}
struct DamageApplication {
    damage: DamageId,
    effect: EffectKey,
    before: Quantity,
    after: Quantity,
    prevented: Quantity,
    budget_before: Option<Quantity>,
    budget_after: Option<Quantity>,
}
struct DamageTransaction {
    id: DamageTransactionId,
    kind: DamageKind,
    entries: Vec<PreparedDamage>,
    applications: Vec<DamageApplication>,
    cause: BatchCause,
    continuation: Box<DamageContinuation>,
}
enum DamageContinuation {
    Combat,
    Instruction { frame: Resolving },
}
// Add Frame::DamageEffects { transaction: DamageTransaction }.
```

`DamageId` is local to this transaction, never a physical object ID. Separate
source/recipient occurrences remain distinct even when they share a recipient;
prevention allocation occurs before recipient-total aggregation.

Capture combat assignments once. Once this damage transaction starts, do not
re-power, reassign blocked damage to players, or substitute a new incarnation.
Before assignment, departures use the existing combat revalidation. After
assignment, retain frozen source/LKI and assigned amount even if source or its
controller leaves. This is the explicit cross-rule inference from510.2
(all assigned damage is dealt),120.2/120.7 (source identity), and506.4 (removal
from combat is not a reassignment rule). Missing recipient objects cannot receive
damage (120.1a/109.2); departed players receive no personal application.

Spell/ability damage keeps its started invocation and existing legal-target
snapshot. Source departure uses derived LKI; recipient departure skips that
occurrence without restarting or retargeting the started program. Walk the new
continuation when retaining source LKI, just as current Commander continuations
retain a detached Resolving frame.

## Candidate pairs and chooser API

A selectable application is an **(effect, original damage occurrence)** pair.
The same shield may offer multiple pairs for simultaneous sources to its one
fixed recipient. Selecting a shield pair allocates its next1 to that source now;
allocation is not fixed before prior replacement choices.

```rust
pub struct DamageEffectCandidate {
    pub effect: EffectKey,
    pub damage: DamageId,
}
Request::DamageEffect {
    decision_id: u64,
    player: PlayerId,
    recipient: TargetEntity,
    candidates: Vec<DamageEffectCandidate>,
}
Response::DamageEffect { effect: EffectKey, damage: DamageId }
```

Public clients receive no internal IDs:

```rust
pub struct EffectHandle(pub String);
pub struct DamageHandle(pub String);
pub enum DamageEffectKind {
    DoubleDamage,
    PreventCombatDamage,
    PreventNextDamage,
}
pub struct VisibleDamageEffectCandidate {
    pub effect: EffectHandle,
    pub damage: DamageHandle,
    pub kind: DamageEffectKind,
    pub effect_source: VisibleTarget,
    pub effect_oracle_id: String,
    pub effect_controller: PlayerId,
    pub damage_source: VisibleTarget,
    pub damage_oracle_id: String,
    pub amount_before: Quantity,
    pub amount_after: Quantity,
    pub budget_remaining: Option<Quantity>,
}
PlayerRequest::DamageEffect {
    token: DecisionToken,
    player: PlayerId,
    recipient: PlayerTargetEntity,
    candidates: Vec<VisibleDamageEffectCandidate>,
}
PlayerResponse::DamageEffect { effect: EffectHandle, damage: DamageHandle }
```

The recipient's player/controller chooses (616.1), never the damage source
controller or shield creator. Fixed-recipient groups are independent here:
process groups in APNAP chooser order then stable public recipient order. This
is an implementation ordering of commuting groups, not an expansion to general
shared budgets spanning recipients. Within each group, request pairs as below.

After every application, recompute candidates using transformed amounts,
remaining budgets, source presence and per-occurrence applied identities.
Do not offer an effect already applied to that DamageId (614.5). Zero damage
has no next application. A missing recipient has no candidates.

Automatic application is allowed only when there is no meaningful/required
choice:

- One legal pair: apply it automatically.
- Multiple pairs using one identical unbudgeted effect on distinct original
  occurrences: apply in deterministic occurrence order; operations commute.
  This covers a lone Furnace or Fog affecting multiple simultaneous sources.
- Multiple eligible source pairs for one finite shield: **ask**, despite one
  distinct effect, because allocation matters (615.7).
- Multiple distinct applicable effects: **ask**, even if one example's final
  life total would happen to agree. Do not replace616.1 with an assumed effect
  priority. Fog-first vs shield-first can leave different future budgets.

Handles are allocated by authoritative transitions/refresh only. `DamageHandle`
is viewer/transaction scoped and maps to the original occurrence. `EffectHandle`
is viewer scoped and maps to a currently known static/resolved identity. A
response must resolve to a currently offered pair, not merely two valid handles.
Source refs are saved public historical handles plus optional current handle;
no hidden library/hand row is minted. Query decision/observation enumerates one
pair per budget unit; query checking resolves handles then validates current pair
and chooser in bounded units. Work, receipts and tokens retain executor/state
scoping and recover only through the existing proved sidecar mechanism.

## Applied prefix, atomic final damage and concessions

Do not implement blanket transaction restart after concessions. Distinguish:

1. **Accepted input, not yet applied:** revalidate against the latest candidate
   set/chooser. If now invalid, keep journal history but clear pending input and
   reissue the correct mandatory choice or perform the new automatic step.
2. **Applied effect prefix:** preserve transformed amount, applied identity and
   consumed shield quantity. A Furnace that subsequently leaves cannot undo an
   already-applied double (616.1f); it only ceases future applicability.
3. **Final damage:** commit all remaining actual damage simultaneously, with no
   intermediate life/marked-damage/Commander-ledger writes or SBA checks.

Frozen event split:

```rust
ProposedEvent::ApplyDamageEffect {
    transaction: DamageTransactionId,
    damage: DamageId,
    effect: EffectKey,
}
EventOutcome::DamageEffectApplied { application: DamageApplication }
ProposedEvent::CommitDamage { transaction: DamageTransactionId }
EventOutcome::DamageCommitted {
    transaction: DamageTransactionId,
    actual: Vec<ActualDamage>,        // strictly positive, exact original IDs
    results: Vec<DamageResult>,       // aggregated recipient before/after
    commander_damage: Vec<CommanderDamageResult>,
    cancelled: Vec<DamageId>,         // missing recipients, distinct from prevention
}
```

An ApplyDamageEffect rule step atomically changes the serialized transaction
ledger and the resolved shield's budget. Life, marked damage and Commander
history remain untouched. These budget reservations are semantically consumed;
do not refund them merely because a player later concedes. No admitted
prevention-trigger or after-prevention instruction observes an intermediate
prefix. A failed application step rolls back that whole step; a failed final
commit rolls back that final damage group, without erasing earlier valid applied
prefixes. This is the same distinction as paid costs versus later invocation
failure. Native evidence records each applied prefix and its consumed facts.

If a static provider leaves after an unapplied response was accepted, revalidate
and reissue; if it leaves after its application, retain that prefix. Unrelated
departures preserve still-legal accepted inputs. A recipient departure removes
its remaining damage applications; already consumed shield use stays consumed,
and fixed-recipient effects cannot be retargeted elsewhere. A remaining living
recipient's shield still applies to preserved assigned source damage even after
that source/controller leaves. Skip gone recipients before final commit. Never
redirect choices to a departed controller's other player's cards.

All candidate/application steps remain inside DamageEffects; no priority/SBAs
occur there. Once no applicable pairs remain, CommitDamage validates the whole
remaining group before writing any recipient/history. Fully prevented entries
emit no positive actual damage and no Commander row; original amount and
prevention applications remain auditable. Direct life events bypass this path.

## Event proof and projections

Consumed pre-event facts must bind transaction identity/current amounts/applied
sets, source derived/LKI facts, recipient incarnation/controller, effect source
and static functioning zone, and shield budget before each application. Final
commit binds the completed prefix and all surviving recipients against the
current state. Snapshot restore replays the application prefix; it must reject
forged derived modifiers, source LKI, repeated Furnace identity, budget reserves,
original/current amounts, source/recipient rebinding and changed chooser.

Expose `VisibleObject.characteristics: Option<Characteristics>` only in the new
profiles (default/skip-none for historical views). This gives agents current
negative/modified power/toughness without trusting definition-base guesses.
Expose new optional-empty `PermittedView.effects: Vec<VisibleEffect>` so public
Fog/shield/modifier state is explicit before an agent chooses to attack.
`VisibleEffect` is a tagged union mirroring only public resolved modifier,
Fog/shield budget/target/duration, and live static Furnace instances, using
EffectHandle and historical/current source/target refs. Do not expose internal
EffectKey/ObjectId/origin frame.

Cleanup gets one new aggregate event that clears marked damage and expires all
turn-scoped resolved effects together before SBA (514.2), with actual expired
IDs/cleared marks in trusted evidence. An ordered pair of ClearDamage and expire
would incorrectly expose a lethal transient state if stabilization slips in.
Growth/Fog/shield creation and cleanup expiration use typed events; static
Furnace applicability is a query over current source state.

## Bounds and exit evidence

Parent tables are closed and satisfy owned<=16: base16/15/11/4, borrowed14/15/13/4.
Twelve creatures *4 Artists gives<=48 accumulated death triggers per nonempty
stack episode; <=13 ordinary entries makes<=61<128. No ETB-trigger observers,
recursion, tokens, untap effects or reanimation are present. Main-phase permanent
recasts require an empty stack. The six possible resolved effects between
cleanup operations are Growth(P0), Growth(P1), Disfigure, Fog, and one activation
per two Healers; safe reviewed live-effect cap16. A double Furnace variant has
two distinct static effect identities. Existing128 stack+pending cap remains.

Damage occurrence bound can be based on these12 creatures: no multi-block
ability exists; each blocker contributes one reverse damage edge, and each
attacker-to-blocker assignment contributes at most one edge per blocker, plus
unblocked attackers. Thus<=24 original positive combat occurrences, plus one
noncombat occurrence for a single instruction transaction. Fixed effect count
bounds applications/choices; no effect can reapply to the same original ID.

Required natural branches include once/twice Furnace, both shield/Furnace
orders, Fog/shield budget order, simultaneous source allocation, negative-power
Ox attack, zero-T Disfigure versus lethal marked damage, pumped Sorcerer fixed1
source/LKI, modifier bounce/recast, atomic cleanup, active-player departure,
source/provider departure before versus after application, and prevented versus
amplified actual death/APNAP. Every choice boundary needs budget/trace/restore/
replay and stale/foreign/accepted-but-invalid input pairs. No profile may silently
exclude a legal affected-recipient ordering or shared-budget allocation.

### Exact public effect projection rows

```rust
pub struct VisibleEffectSource {
    pub source: VisibleTarget,
    pub oracle_id: String,
    // Resolving invocation controller for a resolved effect; current static
    // source controller for Furnace. Departure does not erase that seat.
    pub controller: PlayerId,
}
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum VisibleEffect {
    ModifyPowerToughness {
        handle: EffectHandle,
        origin: VisibleEffectSource,
        target: VisibleTarget,
        power: Quantity,
        toughness: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    PreventCombatDamage {
        handle: EffectHandle,
        origin: VisibleEffectSource,
        expires_in_cleanup_of_turn: u64,
    },
    PreventNextDamage {
        handle: EffectHandle,
        origin: VisibleEffectSource,
        target: VisibleEntityTarget, // existing Player / historical Object union
        remaining: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    DoubleDamageToPermanentOrPlayer {
        handle: EffectHandle,
        origin: VisibleEffectSource,
    },
}
```

Use ordinary strict snake_case serialization consistently. Effect/declaration
handles may refer to public historical source or target incarnations; the
`current: None` member explicitly prevents rebinding/response authorization.
Zero-budget shields can be omitted from active-effect rows once their consumed
prefix is durably recorded; this must not erase the application evidence or
resurrect a consumed budget if a recipient/source subsequently leaves.
