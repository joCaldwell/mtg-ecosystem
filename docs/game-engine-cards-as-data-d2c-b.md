# D2c-b: frozen event players and source-status conditions

D2c-b starts at reviewed merged D2c-a
`45de0b52891404439a844b5350e9669363150a6e`. This is a finite capability increment;
it adds no match profile, hosted capability or search capability. Parent independent
review follows this implementation report.

## Supported representation

IR v22 adds `EventPlayerRecipientV1` and `SourceStatusConditionV1`.
`LifeRecipient::EventPlayer` composes with mandatory fixed draw, gain-life and
lose-life instructions. The structured step pattern supplies its optional actor;
ordinary spells/activations and unrelated trigger patterns cannot bind that player.
The existing opponent-creature-death controller binding remains separate.
The beginning-step grammar remains upkeep/draw/end crossed with controller,
any-player or opponent relations, on battlefield-functioning sources. Source draw
counts of one or two lower to ordered one-card IR instructions under CR 121.2;
other cardinalities remain rejected.

`InterveningCondition::SourceStatus { status: Tapped | Untapped }` composes with
existing beginning-step predicates. `ControllerHandEmpty` also composes with
those predicates while retaining its existing self-dies semantics. Unsupported conditions/functioning zones and new cast, attack, damage and
draw-event trigger families remain rejected; existing dies/enters families are
unchanged. Optional payments/groups,
multiplayer draw and library selection remain outside this increment. Master of
the Feast retains its whole-card rejection and the runtime each-opponent-draw guard.

The event player is frozen in `TriggerEventWitness::StepBegan.active`; it is
independent of invocation controller, source controller and physical owner. Turn
owner remains an APNAP anchor. CR 800.4j can leave a real step with no active player:
that supplies no actor. Actor-only instructions skip without a fabricated draw
failure or redirection; later controller instructions still execute. Controller
departure retains the existing removal of that controller's invocations.

## Current and last-known status

The authority is the pinned 2026-09-25 Comprehensive Rules, SHA256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
[official rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).
The exact local rule lines and interpretation are retained in
`pinned-rule-interpretation.json`.

CR 603.4 requires collection and resolution-start checks. CR 608.2h/113.7a use the
original source's current information in its expected public zone, or its last
information there if it has left. CR 110.5/110.5d distinguish battlefield status
from the physical state of a card elsewhere; CR 400.7 distinguishes incarnations.
Consequently a source tapped after collection and then removed fails an untapped
condition, while an untapped source removed before resolution still passes.
A returned incarnation cannot replace either source's information.

The engine already updates per-invocation `ObjectBefore` facts in
`retain_source_lki`/`retain_source_facts` at actual departure, including paid tap
state and simultaneous lookback. D2c-b reuses that facility. It does not infer LKI
from collection evidence or diagnostic traces. Those facts live only with existing
pending/stack/resolving references, bounded by the existing stack/pending resource
limit; no permanent LKI archive is added.

`InterveningCheck` is a typed tagged enum. The hand-empty variant preserves its
old `condition/controller/hand_count/passed` JSON shape exactly. The status variant
records source incarnation, desired status, actual tapped flag, current/LKI origin
and result, without a fabricated hand count. Restore proves those facts through
independent accepted-input replay as well as structural checks.

## Archived sources and runtime witnesses

The archive remains
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl`,
SHA256 `d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`,
parser `327b3360ea8e5cffb75b136223e75a348e903cbe`. Archive extraction/provenance tools
retain exact record bytes and source-line identities.

| Source | Archive line | Evidence |
| --- | ---: | --- |
| Howling Mine | 31744 | Complete untapped condition and event-player draw; actual cast, normal draw before trigger, wrong step, double check, commander replacement, failed draw |
| Seizan, Perverter of Truth | 772 | Frozen actor loses 2 life, then two individual draws; lethal life/empty-library cases finish the program before SBAs; commander continuation preserves the next draw |
| Serendib Efreet | 33421 | Existing D2c-a admission; direct untargeted source damage regression |
| Fretwork Colony | 22621 | Existing D2c-a admission; synthetic flash/reentry setup proves no counter on successor while invocation life loss continues |
| Master of the Feast | 303 | Preserved negative and runtime guard |

The three-definition D2c-b fixture catalog includes Mine, Seizan and the Efreet
regression. It does not imply three new admissions. The previous D2c-a catalog
and every older catalog remain byte-identical.

The 15 new tests in `event_player_tests.rs`, the 15 previous step tests, compiler
pairs and schema tests cover the finite representation. Synthetic mana/flash/body
changes, alternate predicates and appended controller instructions are explicitly
labeled domain cases. They are not printed card abilities. `witness-map.json`
separates them from unchanged archive programs in private scenarios.

Coverage includes opposite current/departure/reentry statuses; actor/controller/
owner departures; absent-player upkeep/draw/end boundaries with multiple-controller
APNAP; both source-status polarities; hand-empty collection/resolution composition;
budget fragmentation and restore through matching, placement, cached resolution
and commander choices in all trace modes; rehashed actor/status/LKI forgeries;
and atomic ID/capacity overflow rollback. Exact min-IR, complete capability closure,
unbound ordinary spell/activation and unknown-field pairs remain asserted.

## Version and baseline proof

IR advances 21 → 22; genuinely unsupported future probes advance 22 → 23. Snapshot/
bundle stay 19/20 and projection/context/sidecar stay 16/16/15. The persistent
extension is additive: existing hand-empty checks serialize identically, and new
checks are typed variants bound by their IR v22 catalog. No breaking old persisted
shape is introduced.

`baseline-byte-proof.json` verifies all 73 existing manifest/expected-IR/digest/history
files across 24 catalogs against the reviewed parent. No expected baseline was
regenerated. Node's release demonstration checks all **173 historical state
digests** against the unchanged references; complete Rust debug bundle recovery
also passed. All **216**
unchanged-profile final state hashes also equal D2c-a. The exact hand-empty JSON
encoding has a direct regression assertion. These are equality proofs; no failed
hash was replaced.

## Verification

Source freeze: `02f0d4df92343be43241065b916cbcfe7cfd2cc9`. The final report/manual commit changes Markdown only.

| Check | Verified result |
| --- | --- |
| Full `npm run check:engine` (`engine-check-2`) | Formatting, warnings-denied Clippy; **976 Rust passed, 0 failed, 18 existing diagnostics ignored; 58 Node passed, 0 failed, 0 skipped** |
| Proportional release checks | **100 compiler/contracts + 30 step/runtime witnesses + 1 simulator interaction witness = 131 passed** |
| Production server and simulator builds | Passed |
| SDK against the exact absolute release `MTG_SERVER_BIN` | **47 tests**, strict TypeScript, **33** response variants; generated output retained then removed |
| Native compiled-source closure | **554/554 files** and every retained archive member verified |
| Retained executable copies | Bytes/hashes equal tested binaries; both copies executed for source/build introspection |
| Corpus coverage | Two final CLI JSON/TSV pairs byte-identical, also equal to the frozen-source preflight |
| Twelve unchanged profile batches | **216/216 winners**, zero draws, faults, truncations or failures; internal independent replay passed |
| Explicit CLI replay | **216/216 passed**, each output's exact profile/state hash verified |

Both complete debug bundle recovery and the Node release demonstration finished.
The SDK report qualifies its scope as schema/generated-types/mock-HTTP checks;
it does not expand hosting admission. This increment retains native source and
executables; it makes no new fresh-offline-rebuild or quiet P7 performance claim.

| Identity | SHA-256 |
| --- | --- |
| Engine build | `ff48d2ce466d315950a998c350a485be8507b58a04c9035de1d51928eb1dafc2` |
| Server build | `3d5e28e17321cd32574026d550772962ac0d8680a174546c16710d5bd1dcf2da` |
| Simulator executable | `b1f7cab25c079c3ac638ad8c8d57dd5dd31ced43e9b0f241ba594ebae50cb096` |
| Server executable | `f8a56f6c1967e9062992ae672ec90a48c1f402592384fe1c04a69287f4f66e10` |
| Native source archive | `d3cbf5bf4c4df8f4243abd9f69cfdeb92f839246bbae9124aa910c117d909448` |

`source-closure-proof.json` binds the tested/retained executables, manifests and
source archive. Every compiled source file and archive member matches its pinned
byte length and SHA-256. The retained introspection results equal the tested
executables' results, rather than relying on hash labels alone.

## Coverage and unchanged workloads

The final result is **1,929 / 32,116 Commander-legal records (6.00635%)**:
**six additions and zero losses** from D2c-a's 1,923 (5.98767%), an increase of
**0.01868 percentage points**. Legal statuses are 1,929 lowered/validated,
27,705 Unsupported, 2,482 unparsed and zero Invalid. All 38,721 archive rows are
classified. Lowered Oracle-ID sets, rather than first-reason counts or fixture
counts, establish this delta.

| Newly lowered source | Archive line | Complete supported program |
| --- | ---: | --- |
| Seizan, Perverter of Truth | 772 | Each upkeep: actor loses 2 life, draws twice sequentially |
| Kami of the Crescent Moon | 8498 | Each draw step: actor draws one additional card |
| Font of Mythos | 17097 | Each draw step: actor draws two additional cards sequentially |
| Nim Abomination | 26591 | Controller end step: source untapped condition, controller loses 3 life |
| Dictate of Kruphix | 28684 | Intrinsic flash retained; each draw step actor draws one additional card |
| Howling Mine | 31744 | Each draw step: source untapped condition and actor draw |

All six complete source texts, face/ability ASTs and bindings were audited.
`source-admissions/` additionally retains exact tool extraction/provenance and
validated IR for all six, built after source freeze without engine edits. Nim's
status-only condition and Dictate's existing flash demonstrate separate capability
composition; Mine requires both new capabilities. The raw ID comparison and all
source records are in `new-admission-source-audit.json` and the retained coverage
`regression-summary.json`. Master of the Feast remains rejected.

Top fifteen remaining first-failure reasons (these are not predicted gains):

| Reason | Legal records |
| --- | ---: |
| `status:unparsed` | 2,481 |
| `effect:gain-abilities` | 953 |
| `trigger:attacks` | 919 |
| `trigger:cast` | 817 |
| `trigger:deals-damage` | 526 |
| `static:conditional` | 483 |
| `static.scope:unsupported_modifier_scope` | 461 |
| `effects.steps:unsupported_activated_operator_controller_or_cardinality` | 418 |
| `static:enters-with-counters` | 350 |
| `mana:choice` | 346 |
| `effect:reveal` | 342 |
| `multi_face_layout:transform` | 341 |
| `abilities[*].effects[*].steps[*]:data_did_not_match_any_variant_of_untagged_enum_sourcecounterrecipient` | 331 |
| `abilities[*].effect.condition` | 329 |
| `effect:tap` | 317 |

Every workload-manifest field except build identity equals the retained D2c-a
manifest: catalogs, policies, seeds, pods, quantum and all work/completion limits.
Every game's final state hash, winner, outcome, advances, decisions, query count,
rule steps, turns, survivor counts and verified losses also match D2c-a. Wall-clock
times are not equality inputs. No workload or limit was changed.

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

The unchanged 12 profile batches exercise their existing cards. They do not establish
that new admissions played there. New capability behavior is established by the
separate source-to-runtime and synthetic witnesses above. No new quiet P7/search
performance claim is made.

## Source freezes, failures and retained evidence

Implementation commits begin with `f7242c7` (contracts/compiler/fixtures) and
`a33f997` (runtime, typed checks, witnesses and continuation validation).
The settled implementation freeze is `02f0d4df92343be43241065b916cbcfe7cfd2cc9`. Implementation commits are:

1. `f7242c71a0ad52c33fc24db47fdcc84dc8e6c1f9` — generic contracts/compiler, source fixtures and minimum-version/capability tests.
2. `a33f997028acb150b942baa0453e5007d15c5978` — frozen actor execution, typed current/LKI checks, runtime witnesses and actor-aware commander continuation validation.
3. `02f0d4df92343be43241065b916cbcfe7cfd2cc9` — strict nested status fields, direct unbound spell/activation binder negatives, and extended Seizan/Mine commander/source-departure proof.

The final documentation commit records only verified results. Independent parent
review of this increment is still pending.

The first full gate is preserved at its original freeze: 976 Rust tests passed,
18 existing diagnostics were ignored, and Node passed 57/failed 1. The new nested
status unknown-field negative exposed the missing strict-field attribute. It was
fixed by rejecting unknown fields, with direct Rust and focused Node checks;
no assertion or expected digest was weakened. `core-dev-3` separately preserves
the commander-draw restore failure that led to the actor-aware continuation helper.
All earlier development compile/test attempts remain separately named.

An early development Cargo invocation waited on the build lock while an errored
build finished other jobs; compilation did not overlap. Settled full/release
verification runs are serialized. Per-command unsigned commits were used without
changing signing configuration. No parser code, primary checkout, PR or merge was
changed or created.

Retained evidence is outside this worktree at:

`/home/josh/.local/share/mtg-ecosystem/engine-evidence/d2c-b-02f0d4df9234-20261103/`

It includes `verification-summary.json`, phase commands/exit codes/logs, all 216
game proofs and explicit replay logs, `unchanged-workloads-proof.json`, the 173-entry
baseline/73-file byte proof, `bin/`, native introspection, the verified source
archive, `sdk-generated/`, source/rules/witness audits and all failed attempts.
`coverage/` contains preflight/final/repeat JSON/TSV, the exact copied baseline
and ID-by-ID additions/losses. `source-admissions/` contains the six exact records,
provenance and compiled IR. A SHA-256 seal inventories the retained files.

Working logs also remain ignored at
`/home/josh/.t3/worktrees/mtg-ecosystem/t3-6e9a8ddb/generated/final-engine-verification/d2c-b/`, with corpus outputs at
`/home/josh/.t3/worktrees/mtg-ecosystem/t3-6e9a8ddb/generated/cards-as-data-coverage/d2c-b/`.
The original D2c-a evidence and failed logs were read without modification.
An auxiliary source-audit summary helper initially indexed an omitted empty
`keywords` vector; that preparation failure is retained separately, and neither
source extraction nor compiled IR was changed to correct it.
