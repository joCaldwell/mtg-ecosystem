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

## Verified result

The settled implementation/fixture freeze is
`cabfb4399ea2d6e476b484e9f504885edd8cdd80`. The final documentation commit changes
Markdown only. Implementation commits are:

1. `8915433`: typed IR capability, exact source lowering, eleven positive
   definitions, two whole-card negatives and complete capability/min-version tests.
2. `86b6570`: typed pending/stack/event witnesses, transition collection,
   snapshot/bundle versions, native adversarial witnesses and stable reporting.
3. `cabfb43`: the proved 37-digest representation adjustment and six Node
   assertions updated to typed evidence; equality and incarnation checks remain.

This IR v21 implements only D2c-a's subset. The historical P6.11 proposal's wider
v21 card-flow vocabulary is not thereby admitted. Parent independent review is
still pending; no implementation sub-agents, PR or merge were used.

| Verification | Result |
| --- | --- |
| `npm run check:engine`, fresh full rerun | Formatting and warnings-denied Clippy; **959 Rust passed, 0 failed, 18 existing diagnostics ignored**; **57 Node passed, 0 failed, 0 skipped** |
| Frozen release compiler/contracts | **98 passed, 0 failed** |
| Frozen release step witnesses | **15 passed, 0 failed** |
| Frozen release interaction-report witness | **1 passed, 0 failed** |
| Release server + simulator build | Passed |
| SDK against the exact release server | **47 tests passed**, strict TypeScript, **33** response variants; generated artifacts retained and removed from the SDK |
| Native source closure | **547 files** verified; retained simulator/server identities equal the tested executables |
| Coverage | Two full CLI JSON/TSV pairs byte-identical, also identical to the post-guard result |
| Twelve unchanged profile batches | **216/216 winners**, zero draws, faults, truncations or failures; internal independent replay passed |
| Explicit CLI replay | **216/216 passed** |

All fifteen native witnesses are in `step_trigger_tests.rs`. They cover a real
source-backed Arena cast before correct/wrong upkeep; ordered draw/life, empty
library and lethal life; APNAP and same-controller order; exact-once collection
with one-step budgets; all trace modes; restore before entry and during placement;
normal draw and commander replacement before draw-trigger placement; owner/source
and controller departure; inactive-player turns; forged pending, stack and trace
witness rejection; ID-overflow and typed stack-capacity atomic rollback; inactive
graveyard sources, opponent end steps, source-type/program composition, complete
archived token/counter/damage/target/life families, and hosting/search rejection.
The compiler witnesses additionally cover every supported step/relation pair,
IR20 rejection, exact capability closure and whole-card unsupported forms.

The actual coverage is **1,923 / 32,116 (5.98767%)**, **19 additions and zero
losses** from D2b, an increase of **0.05916 percentage points**. All 1,904 baseline
legal Oracle IDs remain lowered and validated. Legal statuses: 1,923 lowered,
27,711 Unsupported, 2,482 unparsed, zero Invalid. The entire TSV has 38,721 rows.
The [coverage report](game-engine-cards-as-data-coverage.md#d2c-a-beginning-of-step-triggers)
contains the exact top fifteen remaining first reasons. The largest semantic
boundaries are `effect:gain-abilities` (953), `trigger:attacks` (919), `trigger:cast`
(817), `trigger:deals-damage` (526), and `static:conditional` (482); unparsed rows
remain the largest overall first reason (2,481, plus one excluded-record reason).

The nineteen newly lowered legal records, checked against their whole source
texts and ASTs, are:

Baleful Force, Bitterbloom Bearer, Bloodgift Demon, Celestial Force, Dogged Pursuit, Drana's Emissary, Fledgling Djinn, Fountain of Renewal, Fretwork Colony, Juzám Djinn, Magmatic Force, Moroii, Nettletooth Djinn, Nyx-Fleece Ram, Phyrexian Arena, Priest of the Blood Rite, Ravenous Giant, Serendib Efreet, Verdant Force.

`unchanged-workloads-proof.json` compares all twelve new batch manifests with the
retained D2b reports, excluding only the necessarily different build identity.
Every remaining manifest field is identical, including catalogs, policy, seeds,
pods, quantum and limits. Every profile has the same 18 winners as D2b. Batches
ran at most two profiles concurrently, after Cargo builds finished. No capacity
or completion limit was changed. Timings in the raw logs are functional evidence;
this build has no new quiet P7/search-performance claim.

| Profile | Winners | Explicit replays |
| --- | ---: | ---: |
| commander-attachments-v1 | 18 | 18 |
| commander-combat-keywords-v1 | 18 | 18 |
| commander-data-admission-v1 | 18 | 18 |
| commander-foundations-v1 | 18 | 18 |
| commander-four-colors-v1 | 18 | 18 |
| commander-keywords-v1 | 18 | 18 |
| commander-mana-v1 | 18 | 18 |
| commander-removal-v1 | 18 | 18 |
| commander-sacrifice-v1 | 18 | 18 |
| commander-staples-v1 | 18 | 18 |
| commander-tokens-v1 | 18 | 18 |
| commander-wipes-v1 | 18 | 18 |

## Identities and retained evidence

Engine build identity:
`82fa2fbc7789cb12d5d5feb3562fbe61b757c58b3a831a2d8ee20e5ccf562483`.
Server build identity:
`81c73397da525d9bd4c08ae0eac1e2503340d6d8a60ed1c4e55ff7f02cb4e9ad`.
Simulator executable SHA-256:
`ac8f4cdfee1ddbe2a27737693fdf6a92b07918b00a3308d0638095edab74e97e`.
Server executable SHA-256:
`96d98821ec1baf4e1e4e4ff3a7b113f84fa43cc7940653006440cb732cf45bb0`.
The retained copies were executed for source/identity introspection, not merely
hashed. Their reported identities and source manifest equal the tested binaries.
Every archived native source byte matches the compiled manifest; source archive
SHA-256 is `452bf748f55a6d86f1f7f6d32354538b057851a14863bef368a9e643106bdc03`.

Evidence is retained, ignored, at:

`/home/josh/.t3/worktrees/mtg-ecosystem/t3-d85b51a3/generated/final-engine-verification/d2c-a/`

It contains `verification-summary.json`, exact phase commands/results and logs,
all 216 game proofs, all 216 explicit replay logs, `bin/`, `sdk-generated/`, both
native manifests, `settled-native-sources.tar.gz`, the original and regenerated
baseline proof, the complete release demo bundle, source-to-IR fixture audit,
and all initially admitted candidates' source text/AST audit. Corpus results and
ID-by-ID additions/losses are under
`/home/josh/.t3/worktrees/mtg-ecosystem/t3-d85b51a3/generated/cards-as-data-coverage/d2c-a/`.

## Preserved failures and corrections

No failed or interrupted invocation is relabeled as passed. The final full gate
is `engine-check-2` and succeeds independently of the first failed full gate.
`pre-freeze-attempt-inventory.json` records suite counts for each development run.

| Earlier attempt | Recorded result and correction |
| --- | --- |
| First extraction | Failed before writing fixtures: adjacent accepted manifest/provenance registry had not been bootstrapped; exact accepted data was then copied and extraction rerun. |
| `compiler-dev-1` | 76 passed / 7 failed: formerly future IR21 is now valid; probes moved to invalid IR22, retaining rejection assertions. |
| `compiler-dev-2` | 96 passed / 1 failed across completed suites: the historical noncreature source template assertion was replaced by a supported-program positive and wrong-zone negative, as required by this capability's semantic source generalization. |
| `check-dev-2`, `clippy-dev-1` | Helper compilation and a fixture include path failed; corrected without changing engine semantics. |
| `core-dev-1` | 0 passed / 10 failed: combined private test catalog was not sorted canonically; fixture helper corrected. |
| `core-dev-2` | 8 passed / 5 failed: private replay helper needed the exact intermediate boundary, fork token comparison needed the original token, and an optional source program was correctly rejected. Positive composition uses a mandatory program. |
| `core-dev-4` | 14 passed / 1 failed: Master of the Feast reached the existing unsupported each-opponent draw fault. Whole-card/IR validation was strengthened; that source now rejects. The runtime guard stays. |
| First full `engine-check` | 958 passed / 1 failed / 18 ignored, before Node: stale trigger digest baseline. The 173-snapshot representation proof justified exactly 37 digest updates before a new commit and full rerun. |
| Representation proof preparation | Two external-reader attempts failed on omitted empty pending arrays and nested Commander evidence. Original scripts/logs remain. The successful proof reproduces all 173 prior hashes before writing any baseline. |
| Fixture hash audit preparation | Initial helper included the final newline rather than provenance's documented `trim_end` bytes. Corrected helper verifies all thirteen exact record hashes; no fixture was changed. |

The first coverage report's stale `ir_version:20` label and initial twenty-card
count are retained separately. Settled reports use IR21 and reject the one
unsupported multi-player draw. Local signed-commit setup waited on interactive
1Password signing and was stopped; the task commits use per-command unsigned
commits without changing signing configuration. None of these preparation issues
is counted as a passing engine verification.
