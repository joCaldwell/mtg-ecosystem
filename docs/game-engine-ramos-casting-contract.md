# Ramos M3: casting during resolution and basic cascade

The original research boundary was `0fdbd817e6304eca7873012f4cea40fa2d3bb038`
(M0/M1a/M1b/M2 plus `main8540`). The bounded M3b implementation below builds on
checked ordinary announcement/payment base `4fc61cfe86045b5a2dc14e434d7518ab07a8bc55`.
Both complete frozen basic sources now lower and have private paid runtime witnesses;
optional additional-cost source work and the full M3 exit remain open. No profile,
host, SDK or sampled-search qualification follows.
The [Ramos plan](game-engine-ramos-plan.md#m3-build-casting-during-resolution-and-basic-cascade)
remains the milestone authority. Parent owns combined native qualification.

## Frozen authority and complete sources

Use committed `packages/game-engine/tests/fixtures/ramos/` records, its
[revision-17 manifest](../packages/game-engine/tests/fixtures/ramos/revision-17.manifest.json),
`provenance.json`, `accepted-archives.json` and `archive-manifest-current.json`.
M0 already verified the archive; do not rehash the full archive or query the live
deck. The app/frozen text comparison remains unverified as recorded in M0.
Archive SHA-256 is `d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`;
header SHA-256 is `994bc48e8d8791696330f63338bc32ee0bd17c8c7e9901956ebd35c171f9315a`;
parser revision is `327b3360ea8e5cffb75b136223e75a348e903cbe`.

These are source witnesses, not a second per-card support matrix:

- [Shardless Agent record](../packages/game-engine/tests/fixtures/ramos/2afbaa9a-c171-4a8b-90f3-5250d8498356.json):
  Oracle `2afbaa9a-c171-4a8b-90f3-5250d8498356`, printing
  `b0824e77-c84b-464a-aa0c-44af5f6faa50`, archive line 6419/source index 6417.
  Complete source: normal-layout {1}{G}{U}, artifact creature, Human Rogue,
  2/2, green/blue colors and identity; its only ability is cascade.
  File SHA-256 `343297e44e4d7a6229956fe0a26ea9196e7f116ad50d65947aa61b0c4194cc81`;
  record `2f89bf8cf99de3b56052d9733888a8baea0484cf2076b154929af29ee796a076`;
  source `5bb608651d0437646252f3561bdd1999822a5d2874ac0b44ed16421cd69b4ad3`;
  AST `81e10bd7447faae0a18cf13c6f683c11009cfef9040a8f74833173181f20cbeb`;
  binding `c8f75f4e5b4dff62650986939d9a2a282eb95edfec4409d419d3f4d6a034e4ac`.
- [Bloodbraid Elf record](../packages/game-engine/tests/fixtures/ramos/3f0c9466-5ab9-4205-a84f-b4b27b5a678e.json):
  Oracle `3f0c9466-5ab9-4205-a84f-b4b27b5a678e`, printing
  `e2f12f6f-9383-47e6-a44f-2834ad130e51`, archive line 9502/source index 9500.
  Complete source: normal-layout {2}{R}{G}, creature, Elf Berserker,
  3/2, red/green colors and identity; haste and cascade are both obligations.
  File SHA-256 `7ae2f1c71a9c786567f8f68fc9d69fef5d886808953098ae0559cf34c73bad52`;
  record `14e67815bf69178374e884f9b4faad55bfeae82015b00db2140aa2e0ff9b80ab`;
  source `7b74156a9ae813c46183a139f7ad21b436e13c7c8645e807e7b15b6791a69d52`;
  AST `1f82b2d96c86eeddcae912574ecaab8d17fa8a244f0a51e618c0a79dcce19df8`;
  binding `f017da25b0a283d61d73502c4ac504bcb515714cee4878c41f0bd4c7b70346b5`.

Both parsed ASTs have parameterless `keywords`/`cascade`; Bloodbraid has a
separate `keywords`/`haste` block. Bindings identify cascade as triggered and
`you` as trigger controller under CR 702.85, with zero reference obligations.
Archive `fullyBound` does not prove executable support. M3b's compiler keyword
dispatch now lowers these entire records, preserving complete normalized
source/line/aggregate checks, artifact-creature type membership, haste and provenance.
The abbreviated reminder wording is not a substitute for the full keyword rule,
especially the resulting spell's second mana-value check.

Use the [official 2026-09-25 rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
The retained local copy was read and hash-checked; the web endpoint was unavailable.
Rules below are paraphrased from those pinned bytes, not current card memory.

| Obligation | Pinned CR |
| --- | --- |
| Self-cast trigger functions on stack; sequential exile, two strict mana-value checks, optional free cast, random bottom of uncast remainder | 702.85a |
| Future as-you-cascade action point; independent multiple instances | 702.85b–c |
| Casting permission does not override prohibitions; full announcement and legality | 101.2, 601.2a–i, 601.3 |
| Alternative versus additional costs; missing mana cost; X under free casting | 118.6/6a, 118.8–9, 107.3b, 202.3a/e |
| No priority during resolution; cast finishes before parent continues; deferred triggers/SBAs | 117.2e/5, 601.2i, 603.3a–b, 608.2g, 704.4 |
| Source independence and current/LKI information | 113.7a, 608.2h/m |
| Mana ability permission, immediate execution and nonreentrancy; illegal-action reversal | 601.2g–h, 605.3a–c, 733.1–2 |
| New zone incarnation, public face-up exile, hidden library order | 400.7/7h, 401.2/4, 406.3 |
| Copy insertion alone is not a cast; departures and Commander choices | 707.10, 800.4, 903.9a–b |

## Shared interface and ownership

Keep schema 1. Rust contracts own the types, capability closure and exported
constraints. Suggested names below describe typed shapes, not a required wire
spelling. Do not add an arbitrary effect interpreter, card-name dispatch, or one
capability per source/printed mana value.

1. Add a stack-functioning `SelfSpellCast` trigger condition, separate from the
   battlefield `SpellCast { caster, spell }` observer. Lower keyword cascade to
   one such ability with a targetless stack program containing
   `Cascade { threshold: SourceSpellManaValue, source_block }`. Use a family
   capability such as `CascadeV1`, requiring a shared `ResolutionCastV1` and its
   random-bottom/movement support. Validate the exact zone/condition/program/
   binding combination; merely relaxing all triggered zones is insufficient.
2. Extract one announcement entry point with a typed context:
   `PriorityCast` or `ResolutionCast { permission, completion }`.
   A `CastPermission` binds caster, exact eligible object incarnation and origin
   zone, permission owner `(resolving StackId, instruction cursor, source block)`,
   one-use lifetime, alternative cost, and resulting-spell predicate. M3's
   permission is exactly this cascade hit in face-up exile, now, with
   `WithoutManaCost` and strict mana value below the frozen threshold. It grants
   neither another exile card nor a land play nor a later-turn cast. A triggered
   invocation's stack-functioning source zone is separate from the originating
   spell's actual cast-from zone; preserve both rather than overload their meaning.
3. Share modes, X, targets, restrictions, locked total-cost calculation, complete
   cost payment and rollback. Persist the chosen alternative separately from
   printed cost and `paid`/exact spending. An empty paid mana cost is still a
   completed cast; `paid: None` must continue to mean unfinished announcement.
   Represent declared optional additional costs as typed selections with stable
   cost identity and chosen parameters, then form mandatory plus selected costs.
   Retain those selections on the invocation for source-bound paid-cost effects;
   an unattached optional-cost flag cannot stand in for their complete semantics.
   Never treat optional effects during resolution as optional casting costs.
4. Give `Casting` an owned completion enum: ordinary priority return, or a typed
   cascade return containing its `Resolving` owner and progress. Only one owner
   stores that parent; do not duplicate it in both a global slot and nested frame.
   Keep permission identity through `ManaAbility.resume`, spending and rollback.
   Success returns `CastCompleted { stack_id }`; failed announcement returns the
   exact exile incarnation and an unconsumed/renewed offer; decline consumes the
   permission. A failed attempt cannot resume the parent as a successful cast.
5. Use a serializable `CascadeProgress`: owner identity, caster/library owner,
   source-spell identity/current-or-LKI witness, fixed threshold, stage, ordered
   exiled successor incarnations, optional hit, cast result, randomization cursor
   and permutation. Stages are scan, offer/announcement, randomize, bottom, finish.
   Separate semantic stage from pending player input and random-word rejection.

The shared integration owner should own contracts/compiler dispatch plus actual
required edits in `engine.rs` and `engine/{casting,payments,actions,triggers,
multiplayer,events,visibility,work,action_query,state_digest,snapshot_codec,
recovery,capacities}.rs`; a small `cascade.rs` may own this family execution.
These are reservations to confirm before implementation, not permission to edit
all files. M1 payment/counter authority is complete; reuse it, with parent doing
integration. M2 search/partition is independent and must retain its behavior.

Peer `20c7abf6-fa6e-4294-8821-a470a1e2372f` reserves ordinary Pump/Grant lowering
of exact `control: 'you'` and temporary Hexproof at `main8540`; preserve
`bind_creature_target` for counter/removal/static semantics. M3 changes casting
context and calls the existing target authorities; it does not own their filter
meaning or Hexproof. Compiler dispatch/tests are a genuine shared-file overlap
requiring an explicit hunk handoff. Query/visibility or target-helper edits by
that peer would require renewed coordination; none are currently reserved there.

## Concrete implementation gaps at this base

This table records the pre-foundation research gaps, not current rejection
status. M3a/M3b below distinguish their implemented scope. Paths are relative to
`packages/game-engine/crates/`.

| Actual surface | Reuse and precise change needed |
| --- | --- |
| `compiler/src/lib.rs` permanent keyword dispatch; `contracts/src/lib.rs::Definition::validate` and exported constraints | Cascade falls through unsupported keyword; triggered validation requires battlefield functioning and narrow instructions. Admit the reviewed self-cast shape and dependency closure without weakening unrelated guards. |
| `core/src/engine.rs::{validate_response,step}` priority `Response::Cast` | Requires priority, own hand or designated command origin, ordinary timing/empty stack, then constructs stack/casting state inline. Extract construction/announcement stages; context supplies timing/zone permission. Keep ownership checks appropriate to that permission and global prohibitions in `casting.rs::validate_cast_permission`. |
| `core/src/engine/commander.rs::casting_cost`; `wipes.rs::{bind_x,stack_mana_value}` | Cost starts from printed mana cost and applies reduction/tax; X adds to total cost. Select alternative first, then additional costs/increases/reductions. Share exact mana-value arithmetic with nonstack X=0 and announced stack X, never total paid cost. |
| `payments.rs::{finish_mana_frame,payment_step}`; `casting.rs::reverse_cast` | Payment records cast/collects triggers then sets Priority and stabilizes; failure also stabilizes. Reversal pops top, restores origin, clears pending, restores passes and returns Priority. Branch completion by context, preserving receipts and parent, with no priority/SBA/trigger placement on resolution return. |
| `mana_permanents.rs::{ManaActivation,underlying_mana_frame,reserved_mana_source}` and receipt attachment | Typed nested payment/resume and Ramos counter/use receipts already exist. Carry resolution completion through all stages; expose mana activations only in their legal payment window and keep source reservations/rollback scoped to this cast. |
| `triggers.rs::{collect_cast_triggers,collect_trigger_witness,trigger_step,validate_trigger_source,validate_trigger_witness,validate_trigger_entry}` | Successful cast witness exists, but observers/source facts are battlefield-only; placement hardcodes battlefield origin; cast witness restore accepts hand/command only. Add the casting spell's own stack source exactly once and exile-origin facts authenticated by the new cast path. Preserve existing battlefield observers/APNAP and distinct trigger IDs. |
| `engine.rs::{StackInstance,Casting,Resolving,Frame}`; `actions.rs::{finish_ability,validate_resolving}`; `multiplayer.rs::begin_resolution` | Resolution keeps its entry on stack; restore requires it at top and completion pops top. New child is above the resolving owner. Bind/validate/remove the exact owner below its completed child, preserving unrelated entries; never pop the child or begin its resolution automatically. |
| `events.rs::{move_object_for,move_instruction_object}` and `ProposedEvent::Move` | Exact moves/new incarnations and library membership removal exist; library vectors are top-first. Generic library Move appends, while instruction library movement redirects to top. Add typed random-bottom group authority with exact membership/order; do not accidentally call the top-placement wrapper. |
| `actions.rs::ResolutionChoice`; `staples.rs::{apply_search_shuffle_word,valid_staples_choice}`; `setup.rs` sampler | Existing choices carry a Resolving owner and shuffle is budgeted/deterministic, but shuffles the whole library and increments shuffle history. Reuse RNG/rejection sampling, not that event: cascade permutes only its uncast exile group and never shuffles the library. |
| `visibility.rs` requests/responses/handles; `work.rs::Task` and response validation; `action_query.rs` | Exile is already public, libraries hidden; action initiation currently distinguishes priority/mana only. Add a caster-only scoped cast/decline offer and validation/query/recovery coverage, not a broad public exile-cast action. Ordinary announcement requests then reuse existing permitted targets/modes. |
| `engine.rs::validate_state`, `actions.rs::validate_resolving`, `multiplayer.rs` departure revalidation; digest/codec/recovery | Validate every owned parent/progress stage, source incarnation, child relationship, receipts and pending input. Current top-frame and priority-return assumptions recur in departure handling. Accepted-input replay remains the reachability proof, including rehashed forgeries. |

## Resolution and payment semantics

At successful ordinary or permission casting, emit one real cast occurrence only
after every cost is paid. Preserve `StackId`, caster, actual origin and spell
incarnation; collect Ramos/other battlefield observers and the spell's own
cascade from this same occurrence. Cascade triggers while the spell is on the
stack, never from entering the battlefield. Copy-only insertion must not reach
this boundary; copies remain unsupported M8 sources. Do not invent a copy runner.

At cascade resolution, determine the original spell's mana value once using its
exact current stack incarnation, otherwise bounded departure LKI (113.7a/608.2h).
Store that threshold for the entire scan/offer. Existing CastOccurrence has no
announced X; extend the witness/reference sufficiently for stack mana value,
without rewriting immutable cast-event facts. M1's frozen-color fallback is
valid only in its immutable-color pool. Reuse that bounded current/LKI pattern;
do not claim general mutable-cost/color/copy characteristics. Prune facts when
no pending trigger/continuation needs them. Printed mana cost, mana value, spell
colors, mana actually paid and commander identity are distinct authorities.

Exile the actual top card face up using a normal typed move, retain its actual
successor, then test it. Repeat one card at a time until the first nonland whose
mana value is strictly below threshold or exhaustion. A land with additional
types is still a land. Equal/higher nonlands are skipped; X outside the stack is
zero. Do not inspect future top cards in queries, search for an eligible card,
remove an entire prefix secretly, or continue scanning after an uncastable hit.
Empty/no-hit finishes without a failed draw or decking loss.

Offer that hit only. Global cast prohibitions still win. A creature/sorcery can
be cast here despite ordinary timing; restrictions actually printed on a spell
still apply. Check a legal complete mode/target/cost path; never expose an
impossible mandatory target decision. Decline or an uncastable hit leaves it in
the remainder. An illegal response is rejected atomically and is not a decline.
A legal announcement that cannot complete reverses only that announcement under
733.1, then permits a corrected legal attempt or decline at this same offer;
no unbounded internal retry loop. Engine capacity exhaustion is a typed fault,
not a decline; inability to pay an actual cost follows casting/reversal rules.

The existing prohibition is opponents-during-controller-turn only. These two
own-main-phase sources cannot naturally encounter it at their cascade offer.
Keep a synthetic guard test, but do not label it a legal paid prohibition witness:
that part of the original M3 exit needs a separately source-qualified reachable
casting context before signoff. Do not add flash to either complete source or
quietly waive the exit to manufacture that scenario.

Free casting selects an alternative, not printed cost {0}. Keep mana value and
colors unchanged; X must be zero for the supported undefined-X family (107.3b).
Recheck the resulting spell's strict mana-value bound, even though normal-layout
M3 cards have the same value in exile and on stack. No second alternative cost
may be combined. Mandatory additional costs still apply; optional additional
costs may be selected or declined before targets/cost lock and are then paid in
full. Reuse existing mandatory sacrifice and exact spending authorities. Current
IR has `additional_sacrifice`, not general optional costs: add only reviewed typed
optional-cost forms with a frozen complete-source witness before claiming that
part of the shared interface implemented. Unknown kicker/buyback/other branches
remain rejected, not silently treated as absent. The basic-source checkpoint
alone cannot claim the entire M3 optional-cost exit.

A free cast is not a general mana-activation opportunity. Apply 601.2g/605.3a to
the actual locked cost/payment requirement; mana-bearing additional costs or
increases can create the window. Distinguish acknowledgment of a real {0} mana
payment (118.5) from absence of a mana payment. Existing ordinary casting always
offers its mana window, even when funded; preserve that established path while
making the permission path obey its actual payment scope. No nonmana activation,
pass, unrelated cast or opponent response occurs during the announcement.

Payment reversal keeps prior floated mana and unrelated actions, reverses only
recorded eligible mana receipts in dependency/LIFO order, and restores the exact
hit exile incarnation/offer. Retain versus undo applies to legal mana activations,
not to the earlier cascade exile/reveal sequence: 733.1 forbids reversing those
library-to-public moves. No partial additional costs, phantom cost triggers,
Ramos use resets, stale permissions or lost original invocation may survive.
Stage complete payment atomically with existing transaction machinery. Costs
involving unsupported library/random/irreversible actions remain rejected.

After the child becomes cast, collect its triggers but do not place them, run
SBAs or offer priority. Resume the parent only after the child's entire
announcement/payment has completed or been reversed/declined, not merely when
its proposal or a nested mana ability finishes. Randomize all still-exiled cards
from this cascade that were not cast, including a declined/uncastable hit; exclude
the cast child. Put that group on the caster's library bottom, then finish/remove
this resolving cascade by its own identity. Only now run the normal SBA/Commander
and APNAP trigger-placement boundary and give priority to the appropriate active
seat. Original spell, child and each cascade invocation retain distinct IDs.

The deferred Commander boundary here means graveyard/exile state-based handling
under 903.9a. A commander being moved from exile to its owner's library instead
offers the 903.9b command-zone replacement before that move. Preserve the cascade
continuation through that choice, remove any redirected card from the bottom
group, and never expose the group's private randomized order.

For Bloodbraid into Shardless, the latter's own cascade/Ramos triggers wait until
the Bloodbraid cascade has bottomed its remainder and finished. They are then
ordered normally above Shardless, with Bloodbraid still below it. Nested cascade
is subsequent stack work, not recursive immediate child resolution. Countering
the original before its cascade resolves does not cancel the trigger; countering
the cascade itself performs no exile/cast/bottom program and never refunds the
original spell. An already resolving invocation may finish after supported source
movement; concessions obey 800.4, revalidate offers and remove departing owners'
objects without resurrecting them. If its controller leaves, cancel that player's
pending announcement/resolution as appropriate; never move cards into a departed
library or redirect a personal permission to another player.

## Privacy, randomness, bounds and recovery

Face-up exiled identities/movements are public sequential facts, with new handles
per incarnation. Only the caster receives the actionable cast/decline request;
opponents may know the hit but cannot answer. Library contents/order, RNG seed/
words, permutation, internal IDs and trusted continuation/receipts remain private.
Once bottomed, current exile handles expire. Permitted history retains the known
identities/set and already public exile sequence, never the hidden randomized
bottom order. Random bottom is not a library shuffle: do not increment
`library_shuffles` or discard valid prior top knowledge as if a shuffle occurred.
Equal permitted histories with different hidden permutations must yield equal
player projections/requests/query work accounting, until a later public event
legitimately distinguishes them. No query consumes randomness or mints handles.

Use the match's deterministic RNG and unbiased Fisher–Yates/rejection sampler,
with a private permutation of only this remainder. Persist index and accepted
swaps; rejection consumes its actual word and resumes the same index. Zero/one
cards consume no random words. Commit bottom placement with one explicit typed
order/membership event group; ordinary zone moves supply successor identities.
Preserve the existing library prefix. Review movement order/visibility refresh so
intermediate public history never publishes the secret permutation. Reusing a
whole-library shuffle or exposing a player bottom-order choice is incorrect.

One scan/movement or random-word attempt per budgeted step gives bounded work;
queries use permitted rows, not library-length scans. A cascade list is bounded
by this player's physical library (at most 100 owned cards under the common
policy, including tucked commanders). These own-card basic sources leave at least
one owned original outside that library, giving a tighter 99-card scan bound.
The shared primitive should validate against the common physical bound; extend
and prove the sampler through 100 rather than silently export its current
reviewed 2..=99 assumption to future permissions with different ownership. Do not
use event-group depth as a cascade depth limit. Basic cascade needs one active
resolving owner plus its announcing child; later cascades wait on stack. Charge
owned progress/permutations and retained facts/history, count self-cast triggers
against the shared stack-plus-pending ceiling, and revisit generic trigger sizing
for stack sources. Preserve common capacity policy and checked cursor/ID math;
no card/profile-specific ceilings or rules outcomes for exhaustion.

Restore at each scan, offer, mode/X/optional cost/target, nested mana, spending,
failed-payment return, random-word/rejection, bottom and finish boundary. Validate
owner cursor/instruction/capability, unique exact exiled membership, hit/threshold,
permission lifetime, original and child stack IDs, selected costs, current stage,
receipts and permutation prefix. Recompute rule-derived facts; reject forged
thresholds, missing/duplicated parents, foreign/stale input, already-consumed
permissions, child substitutions, impossible origins or completed lower-entry
changes. Save/restore and accepted-input replay must agree before and after each
pending input; recovered observation/validation/action Work remains exact-state,
executor and viewer scoped. Queued departure processing precedes exposing choices.

## M3a announcement/payment foundation (basic cascade still rejected)

This historical checkpoint was deliberately narrower than basic cascade.
At that source, `casting.rs` owned ordinary-priority preflight and a
consumed `SpellAnnouncement` proposal (caster, exact object and actual origin),
one stack proposal constructor, and common X/mode/target routing. The only
implemented permission authority at M3a was ordinary hand/designated-command casting;
no exile or resolution permission was constructible through player inputs.
`payments.rs::pay_announcement_costs` completes the existing locked mana and
mandatory sacrifice costs and collects the one actual cast occurrence, then
returns a typed `AnnouncementPayment::{Completed, Unpayable}`. Completion carries
stack ID/controller and does not grant priority, stabilize or reverse the proposal.
The ordinary `payment_step` caller owns that return boundary and the existing
reversal. Activation payment uses the same primitive and still never emits a
spell-cast occurrence. Empty explicit mana spending can complete a zero total cost;
printed mana value remains independent. The generic unbiased word sampler now
covers 2 through the common physical-card limit of 100, including tucked commanders;
initial setup still requires its separate 99-card library. M2 library shuffling
uses that physical bound while retaining its existing whole-library history/event.
This is not cascade's future private random-bottom operation. The 100 bound is
verified directly at the generic sampler, including every accepted residue and
rejected tail for each range 2..=100. A 100-card M2 continuation/accepted-replay
witness remains omitted: an own search spell or own activated search source is
outside its caster's library, bounding that source scenario at 99. A borrowed
search activation with all 100 owned cards in the library needs a separately
reachable control context. Private midgame scenarios have no trusted RNG; adding
a seeded match config starts the real 99-card setup rather than authenticating an
arbitrary seeded midgame root. This checkpoint adds no control producer or new
replay-origin authority to manufacture that missing witness.

No IR capability, self-cast condition, alternative cost, resolution permission,
owned parent continuation, public/query form or snapshot format is added here.
Both exact frozen sources still reject with `keyword:cascade`, at
`abilities[0].keywords[0]` (Agent) and `abilities[1].keywords[0]` (Bloodbraid),
including Agent's artifact/creature membership and Bloodbraid's haste obligation.
The dedicated compiler negatives pin those full record bytes and source/AST
identities; they do not admit a partial definition. Direct payment-boundary and
locked-zero-cost controls are explicitly synthetic and are not paid free-cast,
legal-pod or resolution-continuation witnesses.

Focused foundation verification retained 426 distinct passing Rust tests across
source-frozen passes: 96 contracts, 229 compiler and 101 selected core tests,
plus workspace all-target compilation, formatting and the structural guard.
The own debug CLI's 888 compiled-source rows match their actual files; schema
SHA-256 remains `57cb2f4e6d04f2511f4364b508e3ba5417d6cd8ad35a2574d4c80945f384a397`.
Exact commands, copied executables, optimization/build identities, five-source
catalog, source manifests, failures and review continuity are retained under
`generated/ramos/m3a-foundation/` in the isolated implementation worktree.
Two new compiler diagnostic expectations were corrected after a failed pass.
Two old departure expectations and one query scratch-budget expectation also
failed identically on preserved parent producer `0fdbd817`; only those tests were
corrected against the existing production guard and charged initialization unit.
The one read-only production review found no actionable issue; production bytes
remained unchanged through all test repairs. No full runtime/release/profile,
Node parser/Ajv, SDK, host/search or consumer qualification is claimed.

Combined source `4fc61cfe86045b5a2dc14e434d7518ab07a8bc55` integrates this
foundation with controlled-protection main `6a880cbb`. It passed 425 selected
Rust tests (97 contracts, 236 compiler, 92 core), workspace all-target compilation,
formatting and 1,709 exported-schema probes. All 895 compiled-source rows match;
the three-source Ramos/Scales/Corpsejack IR is unchanged. Only additive compiler
test registrations needed manual merge resolution. The reviewed foundation's
production semantics are unchanged; main adds a core test registration and a
documentation comment within those seven paths. Exact copied binaries,
optimization/build identities, source inventories, timing and omissions are in
the coordinator's `generated/ramos/m3a-integrated/integration.json`.
This focused combined result does not expand the foundation's admission or
consumer scope. Native work is released; the 100-card continuation/replay and
broader runtime/profile/SDK/client/host/search checks remain omitted.

The following basic work remained at M3a and is implemented by M3b below:
the shared owned resolution cast permission,
`WithoutManaCost` and X=0, stack-functioning self-cast triggers, complete source
lowering/capability guards, sequential cascade exile/current-or-LKI threshold,
caster-only offer, exact owner-below-child resumption, private random remainder
bottom with Commander replacement, and authenticated recovery/privacy/departure
coverage. M3a did not qualify these exits. The separate source-qualified
optional-additional-cost and reachable paid casting-prohibition exits remain open
after M3b. No full target-deck, profile, hosting, search, SDK or consumer claim follows.

## M3b complete basic cascade checkpoint

IR schema remains 1. `SelfSpellCast` functions only on the exact casting stack
incarnation and admits one standalone targetless `Cascade` program. Its threshold
is `SourceSpellManaValue`; its family closure requires `CascadeV1`,
`ResolutionCastV1` and `RandomBottomV1`. Ordinary battlefield cast observers keep
their separate functioning zone. Both observers collect from the same real cast
occurrence after all costs, including zero mana spending. Cast facts retain actual
origin and announced X separately from the trigger's stack source zone. Agent's
artifact membership and Bloodbraid's haste remain complete source obligations.
Multiple/granted cascade, copies, alternate faces and other permissions still reject.
Nonland source with an empty mana-cost string now rejects explicitly until the IR
represents absent costs; printed `{0}` and land sources retain their prior behavior.
This is an engine representation boundary, not a claim that CR 118.6a forbids a
free alternative for an object without a mana cost.

A one-use `CastPermission` binds the exact face-up exile hit, caster, resolving
stack owner/cursor/source block, `WithoutManaCost` and strict resulting mana-value
bound. `Casting::completion` alone owns the boxed parent progress, including through
`ManaAbility.resume`; there is no global duplicate parent slot. Modes and targets
use the existing authorities, X is fixed to zero for the free alternative, and
mandatory additional sacrifice uses ordinary payment. A free alternative with no
mana payment grants no mana-activation window; ordinary casting preserves its
existing window and explicit zero-cost acknowledgment. The resulting spell is
checked again before payment completion. Decline or a failed attempt never emits a
cast occurrence. Reversal restores the exact exile incarnation and offer while
retaining prior public exile and unrelated resources; the existing receipt authority
owns keep/undo of nested mana activations.

`CascadeProgress` owns scan, offer, randomize, bottom and finish stages. Scan moves
one actual top card face up per step and stops at the first strict nonland hit,
including an uncastable one. Empty/no-hit never draws or causes decking loss. The
threshold freezes once from the originating spell's exact current stack instance
or immutable cast witness/LKI. A paid Counterspell-original witness exercises
source independence; a paid Stifle-cascade witness performs no cascade program.
The caster alone receives the existing token-scoped `OptionalEffect` cast/decline
request. No new public request/context/query form was added; those formats retain
18, with a new internal decision discriminator. Private snapshots use **23**, with
22 reserved by the independent Aura lane; no Aura source was imported here.

The child remains above its resolving owner and becomes fully cast before the
parent continues. Its cast triggers are collected immediately, then wait until
parent bottoming/completion before SBA/APNAP placement. Exact owner removal never
pops or resolves the child. Only this invocation's uncast exile group is permuted
with the shared unbiased physical-100 sampler, one random-word attempt per step.
Zero/one groups consume no words. An ordered typed move group appends the private
permutation, preserves the library prefix and never increments `library_shuffles`.
Commander 903.9b choices follow public scan order, remove redirected objects from
bottom placement and keep the child/parent owned continuation; 903.9a SBAs stay
deferred. Progress/permutations are charged against common retained capacity.
The own original outside the library bounds these source scans below 100; no new
100-card M2 accepted-input continuation witness is claimed.

Focused qualification is **429 distinct passing Rust tests**: 97 contracts,
237 compiler and 95 core. The core selection is cascade 17, foundation 6, ordinary
cast triggers 22, payments 12, payment queries 3, spending 7, mana 14, Ramos M1 5,
snapshot codec 6 and streamed snapshots 3. Cascade's real 99-card setup exercises
private random bottom, every-stage accepted-input snapshot verification, rejected
permutation forgery, equal observations/query work for different hidden orders,
and budgets 0/1/unlimited in Off/Compact/Detailed modes. Other private paid cases
cover both complete originals, creature/target/modal/X=0 hits, mandatory sacrifice,
strict equal/above and uncastable-first-hit behavior, both Commander choices,
nested cascade, source/trigger countering, genuine Ramos observers, stale/foreign
queries, Work recovery, rehashed owner/membership/cursor/child forgeries and departure.
Synthetic prohibition and locked mana-increase Signet/Ramos keep/undo probes are
explicitly not optional-cost source or accepted recovery-origin witnesses.

Workspace all-target compilation, formatting, structural guard and production
Clippy passed with explicit existing-base exceptions: contracts
`collapsible_if` (two unchanged blocks), core `large_enum_variant` and
`needless_question_mark` (three unchanged diagnostics). Compiler Clippy is strict.
No source lint allowances or unrelated production rewrites were added. The one
source review found an accepted pending offer escaping after the last target's
owner conceded; the fix consumes only the frame-owned invalidated choice, and both
accepted-perform and accepted-decline regressions now restore/replay in all three
trace modes. The later absent-cost rejection was inspected separately. One old
spending expectation failed identically on the retained exact 4fc baseline binary;
only that test was corrected under CR 601.6/608.2b, with both failures preserved.

Original M3b native receipts remain in the isolated producer checkout's
`generated/ramos/m3b/focused8/` plus `supplement/`: frozen source inventory
`0749a2c484c6ba75d31605a8c5eafd563fb28744104e6ccc94f66d057337b55c`,
copied actual tests/CLI, `source.tar`, schema and native-source manifest. The own
CLI verifies **897** compiled source files; opt-0 CLI build
`f4f8d52440445039d3a6737e4da1d993a0ae914b858b9d3077a83d5b830a4182`
is distinct from opt-2/debug-asserting test build
`1eaa984d197f7be41dffb49496be4f50a1e7f55f174bdfa4e43ac4d1142feb33`.
Schema SHA-256 is `0cf64f239d17edeebb96669cd4ca636657db3726fc787f5ec35970ad153bd7dd`;
the two-source catalog digest is
`b3ea8c0b214d1363c4188c607f0dee281ea29361cfcd78011974c554842ada6b`.
External Ajv 8.20.0 from the existing root installation passed **27** isolated
schema probes (3 positives, 24 negatives) against these exact exports, including
both complete records, capability/zone/condition closure, multiple instances,
threshold shape and forbidden root-program/intrinsic-keyword paths. This required
no install, Cargo or root engine import; `schema/result.json` retains the result.
The catalog file SHA-256 (including its encoding) is
`988f9f3fe8019bc634041312fcaba15bbe8e5133c3dcaf8a327d65f31859e301`.
Three early zero selectors are retained as **not executed**; supplement uses the
same copied executable and actual nested module paths (5/6/3 tests). Earlier stale
cross-worktree contracts metadata failures retain file/artifact hashes and mtimes;
repairs refreshed only the owned source timestamp under lock, without changing
bytes or deleting cache. No old executable is relabeled as this source.

The complete basic checkpoint is source-qualified separately from later parent
integration. Full M3 remains open for source-qualified optional additional costs
and a naturally reachable paid prohibition context. Full runtime/release/profile,
exact-deck/pod admission, host, SDK, sampled search, parser/Node dependency setup and
consumer qualification remain omitted. Frozen revision 17 and its 85/100/37 pins
are unchanged.

The combined source `6c2c295bed4a8575c8b78974d13618a9b9454ab4` integrates M3b
with Aura/control/composition main `ef8fc6da`. It passed **498 distinct selected
Rust tests** (102 contracts, 244 compiler, 152 core; 499 executions with one repeated
combined witness), workspace all-target compilation, formatting and **1,832
exported-schema probes**. All 908 compiled-source rows match. The original
three-source Ramos/Scales/Corpsejack and two-source basic-cascade IR exports remain
byte-identical to their separately qualified producers. A new private paid
witness composes Control Magic, Bloodbraid, free controlled-target Dive Down and
the creature owner's concession, with restore/replay in all three trace modes.
Both the inner Aura entry/control-reconciliation hooks and outer owned casting
completion are retained. The combined private snapshot is 23; public/context/query
forms stay 18. Exact binaries, build identities, failed integration attempts,
merge resolutions, timings and omissions are in the coordinator's
`generated/ramos/m3b-integrated/integration.json`. Fresh combined Clippy,
full runtime/profile/release, SDK/client/host/search and exact-deck qualification
remain omitted. Native work is released; full M3 remains open for its two
additional-cost/prohibition source exits.

## Bounded checkpoint and focused exit evidence

The basic checkpoint above qualifies both frozen basic cascade sources,
ordinary and resolution-context announcement/payment completion, sequential public
exile, legal creature/target/modal free casts, Bloodbraid-to-Shardless nesting,
random bottom and owned recovery. Include mandatory additional sacrifice and
rollback composition using existing complete fixtures. Do not postpone the
second mana-value guard, global prohibitions, owner-below-child completion or
privacy to later admission. The next bounded M3 checkpoint supplies optional
additional-cost source admission and a reachable paid prohibition context;
M3 stays open until those obligations are source-qualified. No new generic runner
or profile is necessary for private source witnesses; extend existing fixture and
accepted-input scenario harnesses, with legal paid development before cascade.
Concrete reusable hit fixtures are `tests/fixtures/{grizzly-bears,unsummon,
healing-salve,village-rites,bone-splinters,earthquake}.json` under the engine
package: vanilla creature, return target, both one-mode alternatives, mandatory
creature sacrifice with draw or target destruction, and X damage respectively.
Their complete local sources were inspected; use their existing archive/provenance
authority rather than relabel them as M0 records. Bloodbraid into Shardless
supplies the nested witness from the two M0 sources themselves.

| Focus | Meaningful required cases |
| --- | --- |
| Compiler/contracts | Both entire records with original source pins; haste and artifact type preserved; shared parameterized IR. Missing capability, wrong functioning zone/self binding, extra effect/keyword parameters, malformed threshold/predicate or source mirror reject. Exported constraints agree with Rust validation. |
| Core semantics | Strict below/equal/above values; land/nonland membership; creature, targeted and every supported modal branch; decline, empty library, exhaustion/no hit, uncastable first hit, prohibition, original countered and cascade countered. Check exact remaining prefix, membership and bottom timing, not merely counts. |
| Cast/payment | One cast event per successful cast, genuine Ramos color quantity despite zero printed mana paid; no event for decline/failed proposal/copy-only insertion control. X=0 free alternative, mandatory additional costs, optional paid/declined branches when admitted, failed keep/undo with nested Signet/Ramos receipts, preserved unrelated mana/counters/use and complete exile restoration. |
| Stack/trigger ordering | Paid original; simultaneous Ramos/self-cascade ordering; Bloodbraid into Shardless; child cast trigger collection before parent bottom but placement afterward; no priority/SBA leak; original/child/cascade separately targetable at the proper later windows. Child and original survive exact parent removal. |
| Recovery/privacy | Accepted-input scenario replay and restore at every new stage, budgets 0/1/ordinary and all trace modes; zero/one/many remainder and rejected RNG words; query-order purity, recovered Work, stale/foreign handles and rehashed continuation/permutation/receipt forgeries. Opponents see exiled cards, never library order or actionable caster choices. |
| Capacity/departure | Tucked-commander membership and synthetic 100-card sampler range control, countered/source-moved original, caster/other owner concessions at offer/payment/bottom; typed atomic faults with no phantom moves or natural winner. Synthetic forks supplement paid legal source scenarios. |

Use current compiler/contracts libraries, focused core tests near
`cast_trigger_tests`, `payment_query_tests`, `mana_tests`, `staples_tests`,
`ramos_m1_tests`, snapshot-codec and Work/recovery regressions; extend existing
accepted-input scenarios rather than build an alternate runner. A later executable
owner selects these under the single native lock and records actual counts and
omissions. These are proposed tests, none executed in this research checkpoint.

Keep multi-face/modal-double-face/split/adventure/meld, unsupported missing-cost
representations, other exile permissions, grant/multiple cascade, Averna actions,
affinity, copies and unrelated effects explicitly rejected pending complete source
work. Do not call an unsupported source a legally uncastable hit: admitted catalogs
must reject it before play. Synthetic IR can isolate X=0/resulting-value/copy
boundary controls; only complete archived sources establish source admission.
Future M4 reuses self-cast identity, permission, return and per-instance progress;
its grant/history rules require separately retained successful-cast facts, including
casts before grant-source entry. Trace history is not a semantic ledger. M3's
cast occurrence/current-LKI references do not claim that turn-history support.

Source lowering, a private paid witness, legal profile/pod admission, useful
public-observation policy and consumer qualification are separate claims. Hosting,
SDK and sampled-search reviewed capability sets remain unchanged until explicit
follow-on work; enum exhaustiveness is not that review. The exact Ramos deck and
full M3 exit remain unqualified by this document.

Preparation verification: supplied base/clean owned scope, both local file/record
pins, manifest/provenance/accepted-record membership, exact AST/binding review,
pinned local rules digest and cited paragraphs, local document links and
`git diff --check`. No full-archive rehash, live deck fetch, dependency install,
Cargo, build, test, source/fixture edit, native evidence or consumer qualification.
