# D2b: source references, flash, entry replacements and legend choices

D2b extends the compositional compiler and common runtime. It introduces no
branch on a particular card name, profile selector or per-card IR operation. Implementation
source freeze: `3eb4da9`. The saved `80aadb7` work was resumed on 2026-11-03;
its previous verification evidence was unavailable, so fresh checks are required.

## Rules and representation

All rule references use the pinned 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
[official text](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).

- **Legend rule (704.3, 704.5j, 101.4).** Every same-name legendary permanent
  group controlled by one player requires a keep choice. Players choose in
  APNAP order before the simultaneous SBA wave. Unkept objects enter their
  owners' graveyards without destruction or sacrifice. Commander returns are
  subsequent choices for the fresh graveyard incarnations (903.9a). Departures
  discard the unfinished selection and recompute surviving groups.
- **Source names (201.5).** `ref:self` is canonicalized only with an exact
  source node, identity, AST reference path, expression and unique bound target
  in the archive binding. It becomes the existing `this` source encoding.
  Missing, unresolved, duplicate or wrong bindings reject the complete card.
  Existing damage-source references retain their independently checked form.
- **Flash (702.8).** A permanent's spell uses ordinary instant timing, with
  normal priority, payment and resolution. A land remains uncastable. Flash
  cannot appear as an attachment-granted combat keyword.
- **Entry tapped (614.1c, 614.12).** `EntersTapped` changes the entering source's
  tapped state inside movement, before triggers and priority; it does not emit
  a later tap event. Conditional and other-source shapes remain Unsupported.
- **Ability words (207.2c).** Typed optional labels carry no new rules meaning.
  All costs, effects, conditions, filters and player relations still lower or
  reject together. Triggered labels were already supported at the D2a base.

IR v20 now explicitly requires `FlashV1` and `EntersTappedV1` for their shapes;
valid source-backed fixtures prove acceptance at v20 and rejection at v19.
Neither capability is included in the reviewed hosting or search sets. Feature
requests using either are rejected by the existing capability gate.

The public legend choice uses viewer-scoped object handles and validation
receipts. Rust request/response enums remain protocol authority. The simulator
and SDK example policies use a generic keep choice; exhaustive client/server
variant checks include `legend_rule`.

## Intake boundaries

Rule-bearing subtypes have explicit compiler and IR rejection: Saga, Class,
Case, Vehicle, Curse, Spacecraft, Attraction, Room, Contraption, Fortification,
Role, Adventure and Omen. Aura and Equipment retain their existing complete
shape checks. Ordinary subtype labels add no implicit abilities. Unsupported
card types and supertypes remain rejected.

Artifact lands retain both type memberships, land play and intrinsic mana, but
never acquire `CastArtifactV1`. The core witness excludes them from nonland
wipes and includes them in artifact-removal targets.

Diagnostics descend through the failed typed node, preserving its ability
index and source field order. Repeated discriminator values elsewhere on the
card cannot select a different node. A paired regression reverses token
power/toughness fields with identical bad values. It proves attribution follows
the first field in the source. Report codes still normalize array indices.
Harald, King of Skemfar now reaches `effect:look-at` after its bound self-entry
reference; Dalek Drone remains rejected as `trigger:unbound_death_controller`.

## Corpus measurements

The input is the same read-only accepted archive as D2a, SHA-256
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`,
parser revision `327b3360ea8e5cffb75b136223e75a348e903cbe`.

The baseline was rebuilt from `a5f59b1` in an ignored source archive containing
only the compiler/contracts workspace and fixture closure. Sequential stages
used the final compiler with later features explicitly disabled in that copy:
source canonicalization, flash lowering, entry replacement and nontrigger
annotations. No production source or validation guard was relaxed. Offline
Cargo reused the original dependency versions while pruning unused workspace
packages from this diagnostic copy's lockfile. The stage source, mask, runner,
commands, full TSVs and JSON summaries are retained under
`generated/cards-as-data-coverage/d2b/`.

| Stage | Legal lowered | Delta | Percent |
| --- | ---: | ---: | ---: |
| D2a merged base | 1,828 | — | 5.69187% |
| Intake corrections and diagnostics | 1,828 | +0 | 5.69187% |
| Bound source references | 1,828 | +0 | 5.69187% |
| Flash | 1,882 | +54 | 5.86001% |
| Source enters tapped | 1,904 | +22 | 5.92851% |
| General annotation labels | 1,904 | +0 | 5.92851% |

Final coverage is **1,904 / 32,116 legal records (5.92851%)**: **76 added,
zero lost** versus the independently rebuilt 1,828-card base. All 38,721 archive
records have report rows. Legal statuses are 1,904 lowered/validated, 27,730
Unsupported, 2,482 unparsed and zero Invalid.

There are 56 admitted flash cards and 22 entry-tapped cards. Daring Thunder-Thief
and Undersea Invader need both. Bound non-damage source references unlock
Timeless Lotus and Winter Soldier only once entry replacement is available.
Thus the sequential source-binding delta is zero, while its prerequisite is
covered by both source-backed runtime witnesses. All five admitted labeled
cards already used supported triggered labels; wider annotations add zero.

The new first-boundary reasons differ because diagnostics now descend to the
actual failing node and earlier supported features expose later boundaries.
No rejected record is counted as admitted just because its reason changed.

| Remaining first rejection | Legal records |
| --- | ---: |
| `status:unparsed` | 2,481 |
| `trigger:phase` | 1,133 |
| `effect:gain-abilities` | 922 |
| `trigger:attacks` | 918 |
| `trigger:cast` | 808 |
| `trigger:deals-damage` | 526 |
| `static:conditional` | 479 |
| `static.scope:unsupported_modifier_scope` | 459 |
| `effects.steps:unsupported_activated_operator_controller_or_cardinality` | 417 |
| `static:enters-with-counters` | 350 |
| `mana:choice` | 345 |
| `multi_face_layout:transform` | 341 |
| `abilities[*].effect.condition` | 329 |
| `effect:reveal` | 328 |
| `effect:tap` | 305 |

## Source spot checks

An independent read-only audit checked exact source text and AST against
lowering and runtime operations for these 24 new admissions. It found no
missing semantics. These are source/IR checks, supplemented by capability
runtime witnesses; they do not claim every card was played in a batch.

| Cards | Checked facts |
| --- | --- |
| Ambush Viper; Winged Coatl; Benalish Knight | Flash plus deathtouch, flying and first strike as printed. |
| Pouncing Cheetah; Bounding Wolf | Flash; Wolf also retains reach. |
| Agent Venom | Flash/menace, other controlled nontoken creature death filter, draw then life loss. |
| Resolute Reinforcements | Flash, self-entry creates one white 1/1 Soldier. |
| Affa Guard Hound; Briarpack Alpha | Flash, targeted entry pump with printed amounts/duration. |
| Spectral Sailor | Flash/flying and the separate paid draw activation. |
| Cathar Commando | Flash, mana plus source-sacrifice cost, artifact-or-enchantment destruction. |
| Flaming Sword; Alexi's Cloak | Flash Auras preserve enchant target, pump/first strike or shroud. |
| Dictate of Heliod | Flash and controlled-creature +2/+2 anthem. |
| Dictate of Erebos | Flash, controlled-creature death triggers each opponent's sacrifice. |
| Dictate of the Twin Gods | Flash and permanent-or-player damage doubling. |
| Moss Diamond; Worn Powerstone | Tapped entry and exact mana production. |
| Custodian of the Trove; Carrion Crow | Tapped entry with defender or flying. |
| Daring Thunder-Thief; Undersea Invader | Both flash and unconditional tapped entry. |
| Winter Soldier, Bucky Barnes; Timeless Lotus | Exact shortened/full-name source bindings, tapped entry; Lotus's five-color mana. |

Owlbear, Sanguinary Priest, Assault Intercessor, Slaughterhouse Bouncer and Red
Dragon were also checked as annotation controls. Their underlying trigger,
filter, target and condition semantics remain present. Dalek Drone is a negative
control for an unresolved player relation.

## Review and verification

Independent feature review found no semantic blocker in legend choices, flash,
entry replacement, binding checks or capability admission. Its diagnostic-order
finding was fixed and its additional commander witness added. A separate
follow-up review approved those fixes, read-only. The frozen `npm run check:engine` passed end to end: formatting, warnings-denied
Clippy, **940 Rust tests passed / 18 existing diagnostics ignored**, and **56
Node checks passed**, zero failures. The new diagnostic and commander witnesses
also passed their focused runs. Release compiler/contracts checks passed **95 tests**. Release core checks
passed **10 distinct focused witnesses** (11 executions because the admission
gate appears in two filters). Production server/simulator builds passed. SDK
verification passed **47 tests**, strict TypeScript and **33 response variants**;
generated SDK files were retained as ignored evidence and removed from the SDK.

The native manifest verified all compiled source bytes. Engine build identity:
`4005f4819d69cb6d0687b17475f49aae206b0a45059bc8bac522462bd5699e1a`.
Executable SHA-256: simulator
`cd416c42f9ad99b23b54247cfc582a4cec934e5237340dcca5610e3b9b0031e4`,
server `003da64539e8deddb783c77dfc480161e5bc36611aa47dc4133c0d42e69a725e`.
Exact binary copies and hashes are retained in the ignored verification directory.

Two full CLI coverage runs produced byte-identical JSON and TSV; both are also
byte-identical to the fully enabled measurement stage. All **216/216 games finished with winners**, with **zero draws, faults,
truncations or batch failures**, matching D1c's 18-winner count per profile. All
**216/216 explicit CLI replays passed**, in addition to each batch's independent
semantic replay. The 12 profiles used their original workloads and limits;
two profiles ran concurrently. This is functional evidence. Search performance
has not been remeasured for this build.

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


Server build identity:
`1094bd19802a11c6374d4bdcc5036f2e89c3b2985e9dbb542a8b5ea9ea618ab6`.
Retained simulator/server copies match the tested executable hashes. Executing
the retained simulator independently reverified its 526-file compiled closure;
the retained server reported the matching engine identity.

Raw verification evidence is retained at
`/home/josh/.t3/worktrees/mtg-ecosystem/t3-98099b68/generated/final-engine-verification/d2b-resumed/`.
It includes exact phase commands/results, SDK output, binaries, source manifest,
all game journals/reports, 216 explicit replay logs and review notes. The primary
integration worktree also retains a copy under
`generated/final-engine-verification/d2b-resumed/`.

Earlier failures remain separate: the first engine check caught omitted
minimum-version witness registration (fixed in `3eb4da9`); diagnostic-copy
preparation hit a pruned-workspace lockfile restriction and stale source mtimes;
and the first batch wrapper omitted its output parent directory, causing four
invocations to fail before any game began. The final runner reuses only the
unchanged successful release/SDK/identity/coverage phases and records new,
successful batch/replay calls. Original failed logs and status files remain
unchanged. No interrupted or failed phase is relabeled as a pass.

The D2b result closes its intake/capability brief. General filters/durations,
remaining template-only IR encodings, broader trigger families and P6.11 card
flow remain open in the cards-as-data plan. The next increment is generic
beginning-of-step triggers with typed, replayable event witnesses.
