# D2c-d: ordinary per-creature declared-attack triggers

Implementation starts at `b3c7449`. Independent **source review passed** for this
separate future implementation candidate. D2c-c and the receipt optimization
remain outside this worktree's verification ownership; no combined acceptance
result is claimed here. No parser, other-checkout or optimizer-owned production
file is changed.

Source freeze: `c6893aaf6e67d5a52b99154705c385222edb19e3`; tree
`a535e97c56e5319570fd393417bcaea59cc1644c`. The subsequent report commit changes Markdown only.
The parent owns source review and the eventual combined full gate.

Independent read-only review approved handoff `39e010d` and source `c6893aa`
with no blocking findings. It matched all 17 complete admissions, all 23 fixture
record hashes, 79 prior references, 604 inventoried native source inputs and 69
evidence artifacts. Parent source integration `1668215` matches all 46 reviewed
paths and preserves receipt code; the shared Node contract conflict resolved to
the exact reviewed D2c-d blob, including the corrected `trigger` selector.
The accepted `a7f5571` freeze excludes D2c-d. Pinned Rust 1.99.0 focused checks,
fresh executable/source closure and repeated coverage subsequently passed as
recorded below. Full acceptance remains pending. Earlier 1.99.1 results and
source-only reconstruction retain their original qualifications.

## Semantics and finite scope

IR24 adds `AttackTriggerV1`, `TriggerCondition::CreatureAttacked` and
`AttackSubject`: the exact source incarnation, or an attacker filter whose
controller relation is controller/any player/opponent. Ownership is independent.
The common `TypePredicate` replaces the Rust name `SpellPredicate`; its JSON
encodings are unchanged. Cast and attack filters use the same bounded full-type
membership predicates, conjunction, negation and explicit disjunction. Source
ObjectFilter arrays remain AND; `anyOf` remains OR. `PermanentFilter.types`, which
means alternatives, is not reused. Bounds remain 32 nodes and depth 8.

Collection consumes only actual successful `EventOutcome::AttackersDeclared`
members, after the complete declaration validates and installs taps/combat, and
before SBAs/APNAP/priority. Each matching attacker produces one occurrence for
each matching observer ability. Multiple attackers with split defenders retain
separate invocations; an empty declaration produces none. Vigilance still
triggers. Tapping, casting, entry, later characteristic changes, combat flags
and put-onto-the-battlefield-attacking do not fabricate attack events. No attack
history, invented batch path, new event counter, or per-declaration grouping is
introduced. Existing atomic event/advance transactions roll back the entire
declaration on typed faults, including taps, combat, IDs, pending triggers,
journals, traces and retained charges.

The complete local pinned CR witness includes 508.1f-m, 508.2a-b, 508.3a-f,
508.4, 603.2c/g and 603.3a-b. This separates ordinary per-creature declarations
from player-attacks, blocked/unblocked and put-attacking events. Attack costs,
requirements, alone/alongside/whom, group/one-or-more patterns, histories/limits,
conditions, optional effects and unsupported functioning zones remain rejected.
Unknown fields, ownership/status/subtype/color filters and unsupported recipients
reject complete cards. No defending-player or event-attacker recipient is added.
Existing generic mandatory programs are reused, including the already supported
resolution-start attacking-creature pump set; no new set/filter instruction is
introduced.

For Raging Regisaur's attacker-pronoun damage, the compiler proves that the
attacker equals the exact invocation source. General filtered triggers reject
`DamageSource::It`; they cannot silently deal damage from the observing source.
Two-Headed Hellkite's draw-two already lowers to two singleton draw instructions;
the runtime singleton guard remains intact. Gleam of Battle, Fangren Firstborn,
Falkenrath Perforator, Seasoned Marshal, Sigiled Skink and Novablast Wurm remain
exact complete-card negatives.

## Frozen occurrence and continuation evidence

`TriggerEventWitness::CreatureAttacked` freezes the attacker's full
`ObjectBefore` and `defender_player_id`. It uses the declaration's existing real
`TriggerOccurrenceId::Batch(OccurrenceId)`; the exact attacker member distinguishes
aggregate members, as with token witnesses. Pending, placed and resolving
invocations retain this typed evidence. Structural validation checks the
condition, identity domain, battlefield creature, untapped eligibility, source
relation, owner/controller identities and defender domain. It deliberately does
not require current combat membership or current defender/attacker liveness.
Independent accepted-input replay proves the actual declaration and historical
facts, including coherently rehashed lies that pass structural validation.

The shared triggered-entry validator now also checks resolving frames. Existing
stack-entry checks are retained, with collection-level ID uniqueness kept separate.
Source facts remain separately frozen and departure-refreshed for damage LKI;
the immutable attack witness never becomes current source status. Exact source
bounce/reentry does not rebind a surviving invocation. Source/owner/attacker or
defender departure preserves a surviving controller's invocation; controller
departure cancels it. APNAP and normal target/illegal-target/Stifle behavior are
reused. Traces are nonsemantic.

Snapshot/bundle stay **19/20**; projection/context/sidecar stay **16/16/15**.
Old witness and catalog encodings are unchanged. Future-version probes advance
24 to 25. Hosting and search continue to reject the new capability.

## Complete source and corpus audit

The read-only pinned JSONL is SHA256
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`, parser
revision `327b3360ea8e5cffb75b136223e75a348e903cbe`. The local 2026-09-25 CR
SHA256 is `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Exact excerpts and source paths are retained in `pinned-rules.json`.

The new fixture catalog contains **all 17 actual admissions**, not just the eight
initial promising witnesses. All 23 positive/negative fixture lines are copied
exactly from the fully hashed archive. Their complete source, all abilities,
source/AST/binding/provenance identities and compiled definitions are audited in
`source-admissions-final.json`. Synthetic relation/type/keyword/flash/resource
mutations are explicitly labeled and are not claims about printed abilities or
hosted legal pods. Named-source `ref:self` binding uses the existing generic
source binder; no card identity selects semantics.

Exact legal lowered-ID coverage is **1,968 → 1,985 / 32,116 (6.18072%)**:
**17 additions, zero losses**, +0.05293 percentage points. All archive rows are
classified: 1,985 legal lowered/validated, 27,649 unsupported, 2,482 unparsed,
zero invalid; 38,721 total records. Across legal and nonlegal rows the lowered
set is 1,997 → 2,014. Every addition is legal. No gain is inferred from first
rejection counts.

The additions are Infectious Horror; Righteous Cause; Vicious Conquistador;
Two-Headed Hellkite; Raging Regisaur; Screeching Soulbreaker; Audacious Thief;
Burrenton Shield-Bearers; Iroh, Firebending Instructor; Moonglove Extractor;
Pianna, Nomad Captain; Sanguine Syphoner; Boomerang, Blade Flinger; Herald of
Faith; Shopkeeper's Bane; Library Larcenist; Pulse Tracker.

All **79** prior catalog/expected-IR/digest/history reference files are byte-equal
to the base, including the D2c-c catalog. No old golden is regenerated. Final
runtime equality with the 173 historical references remains a parent acceptance
check, not a source-only or focused-check claim.

## Pinned focused completion

The unchanged reviewed `39e010d` source passed all **102 distinct focused Rust
cases under Rust/Cargo 1.99.0**, including all **18 attack cases in one green
run**, Clippy 0.1.99 with warnings denied across all targets of the four affected
crates, and formatting. Actual cheap Node entry checks passed **26/26**, with
27 catalog positives and strict shape/minimum/future-version negatives. Normal
runtime Node/demo and full acceptance were not run in this focused task.

Fresh native introspection matches **604 source inputs**, including **173 engine
build inputs**, their committed Git blobs and retained archive members. Effective
debug/test and release build metadata records Rust 1.99.0; test optimization 2,
debug assertions and overflow checks remain enabled. New executable pins are
separate from the preserved earlier 1.99.1 binaries. The fresh release simulator
build identity is `032cbba4a087ebe5622fe29a543916755132d9ec145131caaf6b14c95571e844`;
executable SHA256 is `19aa47dae97a7166789e73f28581c04a2b35bc2a92cb72251b69af57619045b5`.

Fresh final/repeat/preflight coverage JSON and TSV compare byte-for-byte: exactly
**1,985/32,116, 17 additions and zero losses**. All 79 prior references remain
unchanged. The parent independently audited all 549 retained artifacts
(2,700,381,346 bytes), reconstructed build identities, checked native archive and
binary pins, verified all 23 source records against the complete hashed archive,
matched all 17 definition provenance bindings and recomputed the exact legal-ID
delta. Its initial audit log parser assumed TAP; the corrected read-only parser
handles Node's actual reporter. No engine tests were repeated for that parser fix.

Read-only focused proof:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/d2cd-focused-rust1990-39e010d-20261104T010100Z/`

SHA256 inventory: `666bd612c40fb8b5ee723d5df80fb1e86773a90404a214352ee654887664033a`.
Seal SHA256: `2ab158425b5581d24e38498b802d99f4740b68fc6a46d91c673af79748723f49`.
Current native archive SHA256: `192b0deb03f3b7bab3ec990bb8e12473d8df264374ab2dda4dbc63b7c1d6020f`.
Parent audit is retained at
`generated/final-engine-verification/resumed-20261103/d2c-d-pinned-parent-audit.json`.
Harness-only failures and the version-only probe preceding lock acquisition are
preserved; all builds, tests and heavy commands were serialized. This is focused
validation and a permission/SHA256 seal, not full acceptance, an immutable-bit
seal, an offline rebuild or a P7 result.

## Earlier development verification and retained attempts

The earlier Rust 1.99.1 development records contain **102 distinct passing cases**
with the qualifications below; the pinned completion above establishes the new
current-source focused pass:

| Command (all Cargo execution under the shared `cargo.lock`) | Passing cases |
| --- | ---: |
| `cargo test --locked -p mtg-compiler --lib d2c -- --test-threads=1` | 13 |
| Compiler selectors `strict_mirror_reports_specific_nodes_keywords_and_fields`, `all_committed_catalogs_are_fresh` | 1 + 1 |
| Contracts `every_capability_has_a_minimum_ir_version_witness` | 1 |
| `cargo test --locked -p mtg-core --lib d2cd_ -- --test-threads=1` plus the exact corrected pump witness below | 17 + 1 |
| Core `engine::cast_trigger_tests::` | 22 |
| Core `engine::step_trigger_tests::` | 30 |
| Core `engine::trigger_tests::` | 7 |
| Core `--test triggers_adversarial` | 8 |
| Simulator `d2c_summary_uses_stable_keys_for_data_bearing_trigger_conditions` | 1 |

The expanded attack command had **17 passes and one fixture failure**, not an
all-green 18-test run. Its attacking-set test initially injected two legendary
Yargle copies under one controller; the legend rule correctly demanded a choice.
Replacing those two fixture rows with exact nonlegendary Storm Crows changes no
semantic assertion. The exact corrected selector
`engine::attack_trigger_tests::d2cd_admitted_named_sources_use_existing_resolution_start_attacking_set`
then passed once (`--exact`). Every one of the 17 admitted whole sources now has
an execution witness, including named-source binding, targeted pump and the
existing resolution-start attacking set. The earlier three-compiler/fifteen-core
D2c-d development run also passed, but is not added again to the unique count.

`cargo fmt --all -- --check` passed after that fixture correction. The focused
prebuilt-CLI Node helper passes cast/attack minimum-IR and strict unknown/boolean
shape guards, future IR25 rejection and all **27** old/new catalog positives.
It calls no demonstration or profile matrix. Dependency setup used
`npm ci --ignore-scripts --workspace=@mtg-ecosystem/game-engine --include-workspace-root=false`;
tracked package and lockfiles are unchanged.

Warnings-denied all-target Clippy stopped on `manual_range_patterns` in the
D2c-b minimum-version test's `22 | 23 | 24`. The equivalent `22..=24` is committed.
At that original handoff, Clippy rerun, fresh native CLI build/introspection and
final repeated coverage were deferred at the parent's a7 gate priority request. The queued
native build never started. PID 276400 had exited at this failure by the time the
priority message was processed; Cargo ownership was released, with no child
killed or orphaned. No later Cargo/runtime/heavy command was started.

The only source difference since the passing focused checks is that equivalent
D2c-b **test-only** lint correction. `source-identity-proof.json` reconstructs the
exact optimized-test build ID
`3701e5d704a2a8505857f84c46187a752b529eaeefea1437d31162d2dc1bb0ec`
from the pre-correction bytes and effective build metadata; all contracts/core/
compiler production bytes are unchanged. It inventories 604 native retained
inputs, of which 173 enter the engine build identity. Effective rustc recorded
by those builds is 1.99.1 (Arch Linux), with test opt-level 2, debug assertions
and overflow checks. This is a **source-only reconstruction**, not a fresh native
executable/source-closure pass or offline rebuild. The existing preflight CLI is
not represented as a current-source native build.

The compiler-only preflight coverage and complete admission/byte audits passed.
The coverage classifications were made by the prebuilt development CLI; the
compiler production change from that build is formatting only. A repeat from a
fresh current-source CLI was deferred then; that earlier result alone made no
final/repeated equality claim. Every positive and negative fixture passed the focused
compiler tests, and all committed catalogs passed exact freshness checks.

Retained failures also include the initial extraction metadata's newline hash
convention; initial test compilation using private pending-witness fields and an
invalid Exile shape; and an ignored audit script's mistaken AST hash boundary
(the archive hashes an array of face ability arrays). Corrections preserve exact
source/AST/fixture bytes and all semantic assertions. `attempt-inventory.json`
records each failure/correction and the ownership release; original Cargo and
catalog logs remain intact. The audit script failure was captured in the tool
transcript and is identified as such, not represented as a retained command log.

Integration overlaps are the shared contract test's future probes 24→25, its new
schema helper/test, and the D2c-c schema test's `triggers`→`trigger` fixture path
fix (the same defect reported by the parent gate). No edits were made in
`sim/match_games.rs`, `sim/main.rs`, `sim/match_games/interactions.rs` or
`scripts/check-engine.ts`; the receipt optimization was not stacked here.

No `npm run check:engine`, full workspace tests/demo, SDK acceptance, all twelve
profile batches/216 replays or ignored acceptance diagnostics are run here.
There are no PRs, pushes, merges, rebases, subagents or signing-config edits.

Retained ignored evidence roots in this assigned worktree:

- `generated/final-engine-verification/d2c-d/`: exact focused commands/logs,
  source/IR/provenance and native identities, pinned rules, failures and byte proof.
- `generated/cards-as-data-coverage/d2c-d/`: immutable copied D2c-c baseline,
  classifications, exact ID delta and complete new-admission source audit.

No external evidence seal or offline rebuild is claimed. The parent must review
source before assigning expensive combined acceptance to a final freeze.
