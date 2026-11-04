# D2c-e: exact-source combat damage trigger candidate

**Focused validation passed; final parent source review and combined acceptance
remain pending.** Validated source freeze `3ecbda316dfca44f973253fbb164652c76ceb14c` / tree `4e13ff9ba13f53758ff715a9a375106b10051332` is an
older-f8-based candidate without the later e9/lifetime changes. It is not current
main or combined acceptance. The source-only/unexecuted wording in the historical
sections below records the initial handoff, before this focused follow-up.

Focused result: exactly **3 compiler + 20 core new tests in both debug and
release**, plus minimum-capability witnesses (1 each), **17 compiler, 138 core,
60 adversarial and 1 regular dense capacity regression** in debug. All selected
executions have zero ignored tests and exact nonzero discovered selectors.
Normal/test configurations, warnings-denied affected-crate all-targets Clippy and
fmt passed. Cheap Node preflight passed **27/27**, with the new case and separate
helper using all seven actual source-lowered definitions; all **27 existing
catalog positives** remain checked. No full engine/demo/workspace/profile/SDK/
216-game/explicit-profile-replay/P7 gate or offline rebuild ran.

Fresh compiler coverage is **1,992/32,116 (6.20251588%), +7/-0**, zero legal-invalid,
with byte-identical final/repeat output. Every actual legal addition is one of
the seven complete source/runtime witnesses listed below. The complete source,
all abilities/AST/bindings, IR and raw/source/AST/binding hashes were audited for
each; all nine original negatives remain Unsupported. All **406 old fixture
files**, including **79 named references and 173 historical hashes**, are byte-
identical to accepted `394301d` evidence (seal `d2f62df82ff48ace935f5c2aa66b4c62cb1d430ebbfdb86bfb37f8602c0d37a0`). This is source equality, not a historical workload rerun.

Corrections `7990b51`, `6a4f8e8` and `3ecbda3` repair one private accessor, the stale
coverage IR label, two unreachable test fixtures and the synthetic Node fixture.
The original failed typecheck/build, 18/20 core run and 26/27 Node preflight are
retained, along with setup/audit failures. No runtime algorithm, semantic rejection,
resource ceiling, budget assertion or expected IR is weakened. The original
independent source approval does not transfer to these changed bytes.
The resolving-forgery structural assertion and actual legacy 2,048/2,049 typed
capacity/atomic case pass; the existing dense typed capacity expectation passes.
Private direct-event fixtures remain distinct from accepted-input replay proofs.

External read-only evidence after completion:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/d2ce-focused-269cd818-20261104`. `report.md`, `phase-summary.json`, exact
`*.selectors.json`, `source-closure-proof.json`, `original-16-dispositions.json`,
`coverage-delta.json`, native/full source archives and `evidence-seal.json` retain
complete counts, failures, provenance, binaries and timing/host/lock scope.
Release CLI SHA256 `9bd91098eddfffd477b455868aa09fdb52ecd483d3ee31708891f16055305afb`,
37,164,064 bytes; native build ID `4f948f6cff57a714f1dc21469bdbf2d2a39fcd0e1646d92e766b12a6d5ca5f90` binds
627 native files. Actual pinned compiler build metadata,
18 binaries and 83 toolchain file identities are retained.
Recorded measured phases including failures total 1348.894063s wall,
2047.341962s user, 31.439723s system; separate lock wait
150.343580s. Maximum single-process RSS 4,026,847,232 bytes is
not process-group peak. Cargo jobs 2, libtest serial, shared Cargo lock; no
measured-concurrency or quiet-performance claim. Completion records explicitly
confirm owned Cargo/native process and lock release and the clean committed tree.

## Historical source-only handoff

Assigned parent: `f8d737c027faee3b7e2b24a7d3485aeb7197f34e`, tree
`bf4d542e9309056cc573d536234e415aee9d5903`.
Initial source commit: `d7f4011bfe355855c8f5a49bf0010862e588b445`, tree
`92bf866635655e76f066b94adca8112971192222`.
The initial handoff commit changed this report only. The parent follow-up below
changed two source files and was uncompiled and unexecuted at that handoff.

## Scope and decisions

IR25 adds `DamageTriggerV1` and `TriggerCondition::DamageDealt` with separate
source (`DamageTriggerSource::Source {}`), kind (`TriggerDamageKind::Combat`)
and recipient (`DamageTriggerRecipient::Player`) dimensions. Each dimension
admits only that finite member. Old enum variants and JSON representations are
unchanged. Trusted snapshot/bundle versions remain 19/20 and public
projection/context/sidecar remain 16/16/15. Existing catalogs, profiles, expected
IR/digests and history references are unchanged on disk; runtime equality has
not been checked here. Hosting/search whitelists are unchanged and exclude the
new capability.

The compiler requires exact source, explicit `combat: true`, and `to: "any"`.
The parser's AST comment at `packages/oracle-parser/src/ast.ts` and actual
`parser/triggers.ts` implementation were read directly: this `any` means printed
**a player**, not an arbitrary damageable target. Named references still require
existing archive-bound source canonicalization before the strict mirror decodes
them. No card identity or name selects runtime behavior.

Omitted/false/noncombat kinds, opponent or object recipients, filtered sources,
unknown fields, grouping/history/quantity qualifiers, event quantities, optional
programs, unsupported conditions and unsupported other abilities reject the
complete source. Existing mandatory trigger programs are reused. Damage triggers
bind neither event-player recipients nor event quantities. No effects, optional
payments, selection operations, parser changes or hosting/search capabilities
are introduced. The reporter's current catalog version advances to 25 so a later
parent-owned coverage run can validate new IR; that edit is not a measurement.

## Actual occurrence, multiplicity and continuations

Collection consumes `DamageCommitted.actual`, after positive final damage is
committed and before lethal SBAs and APNAP placement. It matches the original
pre-commit `DamageTransactionId`, its frozen kind, and each `DamageId` against
that exact transaction's member, including full source facts, controller,
recipient and final amount. Member lookup uses the existing contiguous ID index
and checks its identity. No aggregate life result, current combat flag, fresh
source incarnation or trace is occurrence evidence. Fully prevented/zero damage
produces no witness; partial prevention and doubling retain the final amount.

The explicit multiplicity choice is **enforcement**, not inferred grouping:
one `(source incarnation, player)` member per combat transaction. The ordinary
producer was read directly: each declared attacker has one defender and emits
one player edge; trample also emits separate blocker edges. Preparation rejects
duplicate player edges before consuming IDs or clearing assignment choices;
transaction validation enforces the same invariant on restoration and commit.
Different sources damaging one player remain distinct. First/double strike use
distinct transactions and real batch occurrences. No uniqueness restriction is
added to noncombat transactions or ordered noncombat events.

The genuine legacy `CombatDamage` / `DamageDealt` leaf remains supported. Its
positive typed source/recipient/amount vector supplies actual member evidence;
the immutable source fact is captured from its pre-event game. Its original
vector index is the member ID, under the existing real batch occurrence; no new
batch, event counter or transaction ID is invented. The witness records
`transaction: null` for that path. The legacy leaf also enforces combat
source/player uniqueness and the existing shared damage capacity before writes,
so its member IDs satisfy the witness domain. This is an explicit strengthening
of legacy malformed-vector validation, not a lineage gate. Ordinary combat
continues to use its transactional producer. `NoncombatDamageDealt` never
selects this trigger.

`TriggerEventWitness::DamageDealt` freezes kind, optional transaction identity
and the complete actual damage member. Damage-time source facts stay immutable;
existing separately retained invocation source LKI can refresh when that source
later leaves. Collection observes the battlefield before lethal SBA removal.
An observer absent at damage time cannot trigger. Ownership, frozen damage-source
controller, frozen invocation controller and damaged player retain distinct
fields and meanings; source ownership never chooses the effect recipient.
Structural validation does not equate the invocation controller with the frozen
damage-source controller, or require current combat, source or recipient
liveness. Existing controller-departure cancellation still applies.

Pending, placed and resolving invocations use the shared typed witness checks.
Structural guards check the condition, battlefield creature fact, source identity,
owner/controller domains, positive exact quantity, bounded member ID and
transaction identity domain. Accepted-input replay is the actual provenance
proof: the draft tests include coherently rehashed lies for member, amount,
source, recipient and transaction representation that can pass structural guards.
Noncombat kind lies fail the structural scope guard. Source departure, bounce
and reentry do not rewrite or rebind the historical witness.

Existing staged event-group and advance transactions own rollback. New matching
and capacity faults occur inside them. Draft fault cases compare the complete
state, trace and replay journal (normalizing only the terminal fault frame and
fault annotation), including life, marked damage, physical Commander damage,
IDs, pending entries and retained charges. This is source reasoning and written
assertions, not executed rollback evidence.

## Exact source pins

Read-only parent design:
`/home/josh/Code/mtg-engine-plan/generated/final-engine-verification/resumed-20261103/d2ce-source-design.md`,
SHA256 `3daac4b92e53dd20a961ed4588b093d4feaed8553e41957f2877c789ce3d1569`.
The design was treated as a proposal; actual rules, archive and producer/restore
code were read independently.

Full retained archive:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl`,
SHA256 `d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
Underlying source corpus SHA256:
`f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738`.
Parser revision `327b3360ea8e5cffb75b136223e75a348e903cbe`, parser source SHA256
`316c9deaba977533329b903e9d9ba48b6d02057651702c05efa460fb651fbd3a`.

Pinned official rules:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-9debc71c69aa5310-1Ju2o8/rules-20260925.txt`,
SHA256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Read CR 120.2–9, 510.2–4, 603.2c/g, 603.3a/b, 603.10,
615.6, 117.5, 704.3–4, 113.7a and 800.4a directly. Implementation comments cite
the pinned official rules URL.

All 16 bounded rows were copied byte-for-byte, retaining the entire source,
all abilities, AST lines and diagnostic binding. Source-only checks recomputed
source/AST/binding hashes, compared each fixture with its exact archive line,
verified the complete archive hash, and checked frozen Commander legality,
normal layout and complete successful face/line AST inventories. These are
source checks, not compiler admissions.

Per-record raw/source/AST SHA256 and archive lines are in
[`provenance.json`](../packages/game-engine/tests/fixtures/cards-data-d2ce/provenance.json).
The fixture-local [`accepted-archives.json`](../packages/game-engine/tests/fixtures/cards-data-d2ce/accepted-archives.json)
pins the sixteen raw record hashes to the verified archive origin; old registries
are unchanged. Both metadata files retain their established strict shapes.
The copied manifest's raw file SHA256 is
`326b63c9285b38a8b610d6609434fb116a0dfe80708fef76608d5b3eeab80b76`;
its accepted trimmed-manifest SHA256 is
`994bc48e8d8791696330f63338bc32ee0bd17c8c7e9901956ebd35c171f9315a`.

| Archive line | Complete source | Intended test role (unexecuted) | Oracle ID / fixture basename |
| ---: | --- | --- | --- |
| 4538 | Drider | positive candidate | `1db76752-31ed-4a16-986a-d0852ba7d812` |
| 6875 | Demon of Loathing | complete-card negative | `2e55fc03-779c-4a71-beb7-aa0ade738c24` |
| 7477 | Kutzil, Malamet Exemplar | complete-card negative | `3233af43-7826-4895-b67e-03c6102c2cd5` |
| 7911 | Fear of Failed Tests | complete-card negative | `34fd461b-a913-4a60-9477-ce2bd5c6fc5a` |
| 9081 | Precinct Captain | positive candidate | `3c75234b-e367-46da-8454-be4315f9b9de` |
| 10885 | Neurok Commando | complete-card negative | `48936751-429b-4c40-88f5-72acde6278cc` |
| 12306 | Nine-Tail White Fox | positive candidate | `51f4eb1f-abf2-425a-b78e-cafac177da89` |
| 13027 | Dimir Cutpurse | complete-card negative | `56c1a548-c80c-432b-bf92-d4ef6117b298` |
| 14102 | Thieving Otter | complete-card negative | `5dee663f-9e2e-4503-8086-fc51664fdf29` |
| 14941 | Scroll Thief | positive candidate | `637c5583-4683-4ae4-8b4e-f5da42a772c7` |
| 19442 | Soulknife Spy | positive candidate | `813c0654-7e3b-435b-b7a5-148f0c36a4be` |
| 21636 | Stealer of Secrets | positive candidate | `8f9535f2-f00c-40d3-9997-d1c0ece15f9f` |
| 27940 | Ohran Frostfang | complete-card negative | `b99ada26-9a61-4175-9fb8-15a106960220` |
| 28052 | Research Thief | complete-card negative | `ba5055ce-6809-47a4-bee6-fb97a5b60f8a` |
| 28189 | Jeskai Shrinekeeper | positive candidate | `bb50b4f4-f0da-4283-821b-6d74bd20752f` |
| 28579 | Locke Cole | complete-card negative | `bdcb7aed-3595-4c7e-b9da-543e92de919a` |

The four plain sources draw one. Drider retains reach and creates its exact
2/1 black Spider with reach and menace; Precinct Captain retains first strike
and creates its exact 1/1 white Soldier; Jeskai Shrinekeeper retains flying and
haste and gains one life before drawing one. These inventories came from the
complete pinned source/AST, not model memory.

Negative boundaries are Research Thief's filtered artifact-creature source;
Thieving Otter's omitted combat kind/opponent recipient; Fear of Failed Tests'
event quantity; Locke Cole's triggered discard; Dimir Cutpurse's event-recipient
discard; Demon of Loathing's damaged-player sacrifice; Neurok Commando's optional
draw; Kutzil's casting prohibition plus grouped conditional trigger; and Ohran
Frostfang's Snow type line, ability grant and filtered trigger. Kutzil's original
binding diagnostic remains `fullyBound: false`; it was not repaired or suppressed.
No first-rejection coverage extrapolation or whole-corpus admission claim is made.

## Draft tests, source checks and remaining review

Written but **not run**: three compiler cases, nineteen core cases, one Node
schema case/helper, and the expanded capability minimum-version witness. They
cover all seven complete source-to-IR-to-runtime candidates and nine negatives;
strict booleans/unknown fields/dimensions, min IR25/future IR26; unsupported
program/recipient/condition shapes and named-source binding; all three traces;
positive/zero/full and partial prevention/doubling; multi-source damage to one
player and finite shield allocation; first/double strike; lethal trample split;
SBA/APNAP/Stifle; pending/placed/resolving and replacement-choice restore/replay;
no recollection, independent branches, source reentry and source/owner/controller/
recipient departures; coherent forgeries; legacy/noncombat paths; transaction,
batch and trigger ID, stack-capacity and retained-history rollback; and explicit
hosting/search exclusion. Direct private fixture events are labeled separately
from accepted-input replay witnesses.

The core **test-only** dev-dependency on `mtg-compiler` lets its private tests
compile real complete rows without hand-authoring expected IR. Production core
has no compiler dependency/import. `Cargo.lock` receives only that local test
edge; no package version or external dependency changes. Parent source review
should assess this test arrangement and the legacy shared-capacity guard.
No new public catalog/expected IR was fabricated. Existing D2c-d implementation
and fixtures remain intact; its only test edit keeps IR25 compatibility and moves
the future-version rejection to IR26. Other old future probes receive the same
necessary update. No optimizer-owned `main.rs`, `match_games.rs`,
`interactions.rs` or `replay.rs` is changed.

Performed source-only checks: exact archive/record/hash/metadata audit described
above; TOML parsing; `git diff --check`; and direct pinned rustfmt formatting and
`--check` on owned Rust files, with `skip_children=true`. The formatter is
`/home/josh/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/usr/bin/rustfmt`,
SHA256 `a5d35f2958b2194e3647beecaa10e7bd0b7c846fe3319bc22df357709808fde3`.
The first direct invocation failed to load its adjacent `librustc_driver`;
supplying that installation's `usr/lib` through `LD_LIBRARY_PATH` fixed tool
startup. Formatting then completed, and the final direct check passed. This
proves formatting/parser acceptance only, not Rust typechecking or compilation.

No Cargo command (including version), build, Clippy, Rust/Node runtime test,
SDK check, demonstration, schema export, coverage, batch, replay, ignored probe,
retention or performance run was executed. No heavy/Cargo lock or other
worktree target/cache was accessed. There were no subagents, parser edits, PRs,
pushes, merges or implementation-owned acceptance.

Deferred until explicit parent runtime release, under parent lock ownership:

- Typecheck/build both normal and test configurations with the pinned toolchain
  and locked dependencies; warnings-denied affected-crate Clippy.
- Compiler `d2ce_`, strict mirror diagnostics, complete negative classifications,
  existing D2c compiler cases and catalog freshness; contracts
  `every_capability_has_a_minimum_ir_version_witness` and executable schema guards.
- Core `d2ce_` (all nineteen cases), then affected damage/effects/combat keywords,
  cast/attack/step/ordinary/adversarial trigger and lifetime regressions; focused
  release equivalents and actual restore/replay/rollback assertions.
- Node schema/helper/minimum/future probes from a fresh current-source CLI;
  strict provenance origin loading and each whole source's actual lowered IR.
- Parent source review, a separately named final integration freeze, and ONE
  centrally owned combined full gate with the required engine, debug/release,
  normal Node/demo, SDK/strict TypeScript, profile/batch/explicit replay and
  historical/reference workload. Retention and other release requirements stay
  with that owner; no successful earlier gate is reused for changed source.
- Fresh coverage and exact legal-ID delta only if assigned by the parent, with
  every rejected tail and old-catalog/reference comparison retained. No predicted
  coverage count, compile pass, runtime pass, P7 result or acceptance is supplied
  by this source-only candidate.

## Changed paths

Source candidate (relative to the assigned parent):

```text
Cargo.lock
packages/game-engine/crates/compiler/src/coverage.rs
packages/game-engine/crates/compiler/src/d2cb_tests.rs
packages/game-engine/crates/compiler/src/d2cc_tests.rs
packages/game-engine/crates/compiler/src/d2cd_tests.rs
packages/game-engine/crates/compiler/src/d2ce_tests.rs
packages/game-engine/crates/compiler/src/lib.rs
packages/game-engine/crates/contracts/src/lib.rs
packages/game-engine/crates/core/Cargo.toml
packages/game-engine/crates/core/src/engine.rs
packages/game-engine/crates/core/src/engine/damage_trigger_tests.rs
packages/game-engine/crates/core/src/engine/effects.rs
packages/game-engine/crates/core/src/engine/events.rs
packages/game-engine/crates/core/src/engine/triggers.rs
packages/game-engine/tests/attack-trigger-contracts.ts
packages/game-engine/tests/cast-trigger-contracts.ts
packages/game-engine/tests/contracts.test.ts
packages/game-engine/tests/damage-trigger-contracts.ts
packages/game-engine/tests/fixtures/cards-data-d2ce/1db76752-31ed-4a16-986a-d0852ba7d812.json
packages/game-engine/tests/fixtures/cards-data-d2ce/2e55fc03-779c-4a71-beb7-aa0ade738c24.json
packages/game-engine/tests/fixtures/cards-data-d2ce/3233af43-7826-4895-b67e-03c6102c2cd5.json
packages/game-engine/tests/fixtures/cards-data-d2ce/34fd461b-a913-4a60-9477-ce2bd5c6fc5a.json
packages/game-engine/tests/fixtures/cards-data-d2ce/3c75234b-e367-46da-8454-be4315f9b9de.json
packages/game-engine/tests/fixtures/cards-data-d2ce/48936751-429b-4c40-88f5-72acde6278cc.json
packages/game-engine/tests/fixtures/cards-data-d2ce/51f4eb1f-abf2-425a-b78e-cafac177da89.json
packages/game-engine/tests/fixtures/cards-data-d2ce/56c1a548-c80c-432b-bf92-d4ef6117b298.json
packages/game-engine/tests/fixtures/cards-data-d2ce/5dee663f-9e2e-4503-8086-fc51664fdf29.json
packages/game-engine/tests/fixtures/cards-data-d2ce/637c5583-4683-4ae4-8b4e-f5da42a772c7.json
packages/game-engine/tests/fixtures/cards-data-d2ce/813c0654-7e3b-435b-b7a5-148f0c36a4be.json
packages/game-engine/tests/fixtures/cards-data-d2ce/8f9535f2-f00c-40d3-9997-d1c0ece15f9f.json
packages/game-engine/tests/fixtures/cards-data-d2ce/accepted-archives.json
packages/game-engine/tests/fixtures/cards-data-d2ce/archive-manifest-current.json
packages/game-engine/tests/fixtures/cards-data-d2ce/b99ada26-9a61-4175-9fb8-15a106960220.json
packages/game-engine/tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json
packages/game-engine/tests/fixtures/cards-data-d2ce/bb50b4f4-f0da-4283-821b-6d74bd20752f.json
packages/game-engine/tests/fixtures/cards-data-d2ce/bdcb7aed-3595-4c7e-b9da-543e92de919a.json
packages/game-engine/tests/fixtures/cards-data-d2ce/provenance.json
```

The subsequent documentation-only commit adds `docs/game-engine-cards-as-data-d2c-e.md`.


## Parent source-review follow-up

Independent source-only review approved initial handoff `1699eb4b` / tree
`9a04fa3fc5a42a518f43d3db8357c9fbdce3e572`, with two P3 test-strength gaps and
no blocking source or dependency-cycle findings. It independently verified all
sixteen complete source rows and archive/rules pins. This approval applies to
that exact initial freeze and establishes no compile, runtime or coverage pass.

Parent follow-up source commit `841227a8b4d7a59ecf50f347251486812764a055`, tree
`4bbc0c0aa0fd31ba688a0971590ad4019facfa59`, adds explicit resolving-frame
validation to the coherently forged snapshots before accepted-input restore must
reject them. A new twentieth core case exercises a valid legacy vector at the
shared damage limit, the over-limit typed `DamageCapacityExceeded` fault, and
complete state/trace/journal preservation except the expected typed fault.

Parent source inspection also found that the new duplicate-player guard in
`prepare_combat_damage` preceded the existing capacity guard in `begin_damage`.
An existing dense witness supplies an oversized duplicate player vector and
expects the typed damage-capacity fault. The follow-up restores capacity-fault
priority with an early count check before duplicate validation and adds that
oversized/duplicate preparation to the new atomic boundary case. This is a static
regression finding and source correction, not an observed test failure or pass.
Production legacy damage behavior and all resource ceilings remain unchanged.

Only the two affected Rust files changed in that follow-up. Direct pinned
rustfmt and diff checks passed; no Cargo, compile, lints or runtime checks ran.
The new assertions, including the original three compiler and now twenty core
cases, still need parent-coordinated focused validation after the active
`394301d` verifier releases exclusive execution. No IR25 code is integrated into
that freeze. The independent initial review is retained by the parent; its
approval is not silently transferred to these subsequent bytes.
