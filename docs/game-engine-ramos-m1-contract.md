# Ramos M1: implementation contract

M1a implementation starts from integrated `9c2a4b0c2e1efe5e652bf8ec6255c7856c1ee928`
(main ancestor `bbdba9f5add0bfd0c75c0ec18eb2986581c3e534`), at
2026-11-14 16:11:54 UTC. It owns the complete frozen Ramos source, cast-color
quantity, counter-removal mana cost, per-incarnation once-each-turn use and
payment reversal. The remainder of this document retains the family design;
[M0's single matrix](game-engine-ramos-support.md) separates the source witness
from exact-target/profile/consumer admission. M1b counter replacements were excluded from that checkpoint; the separate
[M1b family below](#bounded-m1b-counter-placement-family) now completes them.
Spell copying/color changes, cascade, multiple mana selectors,
life/damage/hybrid costs and host/SDK/search expansion remain excluded.

## Frozen authority

Read-only archive:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl`.
Full-file SHA-256 (including line terminators):
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
Its header equals `packages/game-engine/tests/fixtures/archive-manifest-current.json`
after trimming the terminator. Accepted manifest identity, hashed with
`trim_end` as in `contracts/src/archives.rs`:
`994bc48e8d8791696330f63338bc32ee0bd17c8c7e9901956ebd35c171f9315a`.
Parser revision: `327b3360ea8e5cffb75b136223e75a348e903cbe`; parser source SHA-256:
`316c9deaba977533329b903e9d9ba48b6d02057651702c05efa460fb651fbd3a`.

The following are exact-name, normal-layout records. Line numbers are one-based,
including the manifest. Record pins use the catalog's trimmed-record convention;
future extraction must preserve the original bytes and provenance, rather than
reserializing these summaries.

| Source | Archive line / Oracle ID | Record SHA-256 |
| --- | --- | --- |
| Ramos, Dragon Engine | 9460 / `3ed41d2d-211b-4013-8562-8c64d54cc43a` | `d7304ce7cd6fc6dc6eb8bb1c338b98d555c7797f6a0a07a1528bc70ba3c0c274` |
| Hardened Scales | 24419 / `a1f3da21-af6d-450e-bf0b-985d158418e6` | `85bce2fae3c64503b8171550f73bbe7d5b76751ebf81597bc324ed7ffbbf3fc0` |
| Corpsejack Menace | 30457 / `ca0cc02b-b106-4eca-9388-d4b48dd3be49` | `3fbecc04dbcbffa0022a5219e7b1f04b8e5d90b0b1a2d2acf28a0122a82c8b4d` |

| Source | Raw source SHA-256 | AST SHA-256 | Binding SHA-256 |
| --- | --- | --- | --- |
| Ramos | `0062c9639e97d8e719819c156c845f7d534b849095cf96ff8bb6c360b712999f` | `b61d955c581e9e821e0b7e493a5af98506532574f2869700bda5484916494fc8` | `f2262acd35cfcb3561ff71151bcb50b5118dd58c7c6d271d474a6e6d93a70211` |
| Scales | `19fe818b645e5db9195ad3c9aa3564e67765dfbcbd0a5c8739f8c4c2ecca537c` | `ce93656930a8d1581a7c52ae71db2db76ced36a0bb278d0036b8dc843aeb5ceb` | `45f53dad214a68fc8cffd5e9aa7dfec63f39ce84a90b2ac8bb693d73c242c467` |
| Corpsejack | `5936c3792a9fe9098db3b73666da3a039a2edc15a6921ff16858aa46a2447845` | `91a2d004a0d0dcc9c325037126784d67d3b5b360fed14dd2cd816746e3650086` | `7cdf870d27db90c2a7b7081080a869e0220686cb24ff0b784b41de6725edd440` |

All three records are parsed. Ramos's diagnostic binding has 7/7 obligations
bound and no diagnostics. Each replacement record has 0/2 bound and four
`unsupported-scope` diagnostics concerning replacement roles and would-be
controller capture. These diagnostic archives are nonexecutable: compiler
`lower_trigger` explicitly uses Rust semantic binding rather than treating the
archive's `fullyBound` flag as executable authority. Replacement implementation
must bind every consumed role correctly; it must neither erase the diagnostics
nor infer that a parser/archive rewrite is automatically required. M0 records
the separate archive-binding and production-lowering outcomes.

Frozen Oracle obligations, paraphrased from those records: Ramos is a colorless
{6} legendary 4/4 artifact creature Dragon with flying and five-color identity;
its controller's spell casts trigger one counter per color of that spell. Its
mana ability removes five of its own +1/+1 counters, produces exactly two each
of W/U/B/R/G, and is restricted to once each turn. Scales is a {G} enchantment
that adds one to each positive +1/+1 placement on a controlled creature.
Corpsejack is a {2}{B}{G} 4/4 Fungus that doubles that placement instead.

Rules authority is the local official **2026-09-25** text:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-9debc71c69aa5310-1Ju2o8/rules-20260925.txt`,
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
The pin is sufficient; no newer rules or live card text were substituted.
Semantic code should cite the [pinned official URL](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
and the specific rules below.

| Obligation | Exact pinned CR citations |
| --- | --- |
| Colors differ from mana paid and color identity; colorless contributes zero | 105.1–105.2c, 107.4a–c, 903.4–903.4a |
| Successful casting creates the event; ordinary trigger waits for priority | 601.2g–i, 603.2, 603.3 |
| Referenced spell information at resolution; departed source independence | 608.2h, 113.7a; exact named source: 201.5 |
| Full payment, legal reversal, retained legal mana activations | 118.3, 601.2h, 602.2, 733.1–733.2 |
| Immediate mana ability, payment windows, no reentrant activation | 605.1a, 605.3a–c |
| No tap-symbol sickness restriction; use restriction survives control change | 602.5a–b |
| New object on zone change; counters disappear; entry placements count | 400.7, 122.1a, 122.2, 122.6 |
| Replacement is prospective, applies once, and uses affected controller's order | 614.1a, 614.4–614.5, 616.1, 616.1e–f |
| Would-be entry characteristics and existing replacement sources | 614.12 |
| Copying a spell alone is not casting it; mana empties normally | 707.10, 106.4 |

## Concrete baseline and proposed interfaces

Paths below are relative to `packages/game-engine/crates/`. These are inspected
source gaps, not freshly executed compiler diagnostics.

| Existing boundary | Reuse / required change |
| --- | --- |
| `core/src/engine/triggers.rs`: `CastOccurrence`, `collect_cast_triggers`; `payments.rs`: payment completion | Already freezes stack ID, caster, origin and stack `ObjectBefore` only after paid casting. Keep this sole event boundary; copies and aborted casts cannot call it. |
| `contracts/src/lib.rs`: `PutPlusOneCounters`; compiler `Count`, `SourceCounterRecipient`, `SourceCost`, `Ability::Activated` | Only fixed positive counter amounts; no product/distinct-color binding, remove-counter cost or activation restriction field. Consume Ramos's complete exact AST, including `ref:self` binding and `restriction`; never ignore extra fields. |
| Compiler `lower_mana_ability`; contracts `Definition::validate` | Both require tap-self mana costs. Fixed production already represents all ten symbols, including duplicates. Extend the admitted cost shape narrowly; do not turn production symbols into a color-choice list or add a choice for Ramos. |
| `core/src/engine/mana_permanents.rs` | Eligibility and departure revalidation unconditionally reject tapped/unready creatures. Make those checks conditional on a tap cost. `reverse_permanent_mana` currently requires a surviving source to be tapped and restores only tap state; counter and use-ledger reversal are missing. |
| `core/src/engine/state.rs`, `turns.rs` | No per-ability turn-use ledger. Reuse monotonic `turn.number`, not controller's turn count, land usage, tap state or commander cast ledger. |
| `core/src/engine/tokens.rs`: `execute_token_instruction`, `apply_token_event` | Placement mutates counters directly and rejects zero. A resolved zero-color trigger must finish as no effect before this positive-event boundary. All actual placement producers need one replacement hook. |
| `core/src/engine/effects.rs`: damage transactions and `EffectKey` | Existing ordering, applied-effect identities and serializable continuations are a pattern, not generic counter support. Counter placement needs its own typed transaction, request and validation. |
| `core/src/engine/visibility.rs`, `action_query.rs`; `engine.rs` restore/validation; `recovery.rs` | Public counters and authenticated queries already exist. New replacement choices and use availability must survive projection/query/recovery; raw IDs and snapshots remain trusted only. |
| `contracts/src/lib.rs`: requirements, validation, exported schema; `core/src/profile_data.rs` | Exact capability closure exists. The hosting allowlist includes ordinary counters/mana; search has a narrower set. New struct fields alone would escape the hosting fence without new requirements; review both consumer sets. |

Recommended executable shape (names are proposals; the semantic boundaries are
requirements):

1. Add a narrow `PutSourcePlusOneCountersForCastColors { source_block }`
   instruction and `CastSpellColorCountersV1` capability, retaining the existing
   fixed-counter instruction. Validate it only in a controller-spell-cast
   trigger with no targets, modes or unbound event reference. Lower exactly the
   archive's `1 * distinct-count(color, that spell)` expression; do not introduce
   an arbitrary AST evaluator. Count distinct W/U/B/R/G, never C, symbol
   multiplicity, mana spent or commander identity.
2. Preserve the frozen cast occurrence as identity/matching evidence, but use
   current spell colors on the expected stack incarnation, otherwise bounded
   spell-departure LKI, when the instruction resolves (608.2h). Cast-time color
   facts coincide with resolution information in the current immutable-color
   pool; this is not a universal event-time quantity rule. Before admitting
   spell-color changes, departure must refresh that witness separately without
   rewriting historical cast facts. Reentry of either spell or Ramos cannot
   rebind it. A Ramos that has left its battlefield incarnation gets no counters.
3. Extend shared `PermanentActivationCosts` with optional positive
   `remove_source_plus_one_counters`; add a typed `OnceEachTurn` restriction to
   `ManaAbility`. A distinct `CounterRemovalManaV1` capability must cover both
   cost and restriction requirements, in addition to ordinary permanent mana.
   Validate the supported family shape: positive source-counter removal, no tap,
   mana or sacrifice cost, once each turn, and nonempty fixed-symbol production.
   Ramos supplies the exact five-counter/ten-symbol source witness; printed
   quantities are parameters, not capability or runtime branches. Retain
   existing validated tap-cost shapes. Reject unknown restrictions/counter
   kinds/recipients and unsupported combinations, even after recomputing closure.
4. Use an internal ability key `(ObjectId, ability kind, index)` and a bounded
   current-turn use set. M1 can retain the existing one-mana-ability-per-source
   action, with index zero; do not key by player, Oracle ID or `source_block`
   alone. Successful activation marks use atomically with five-counter removal
   and ten-mana production. It works while tapped or newly controlled and during
   another player's turn. Control change does not reset use; a new turn or new
   incarnation does. Prune departed incarnations/current-turn history safely.
5. Extend `PermanentManaUndo` with the exact prior counter quantity and prior use
   entry, plus the ability key. Reuse nested LIFO receipts and exact spent/
   produced pool accounting. Validate the full action before mutation; illegal
   input leaves counters, mana, use, passes, journal and IDs unchanged. On an
   illegal enclosing cast, undo requested legal mana activations per 733.1;
   if retained, their counter cost and use remain spent. Reversal restores the
   counter quantity directly, not through a placement event or replacements.
6. Add `ControllerCreaturePlusOneCounterReplacementV1` and a closed static
   replacement operation (`Add { amount }` / `Multiply { factor }`). Bind the replacement source's
   controller and affected creature separately. No card-name dispatch, generic
   replacement language, counter removal, or damage-replacement reuse by alias.
   Typed validation and exported-schema constraints must agree with capability
   derivation. Keep IR schema **1**; preserve exact build/catalog fencing and
   leave host/search sets unchanged.

## Counter event, choice and recovery contract

A placement transaction owns the exact recipient, original positive quantity,
current quantity, cause/occurrence identity, applied static effect keys and an
owned continuation. At each prefix recompute applicability: a replacement source
must function on the battlefield, its controller must control the affected
creature, and that key must not already have applied to this event. One Scales
maps `n` to `n+1`; one Corpsejack maps `n` to `2n`. Distinct instances each apply
once; enumeration order is not a rules decision. Zero creates no placement and
cannot become one through Scales. Costs removing counters never enter this path.

The affected creature's controller chooses one currently applicable effect
under 616.1; with multiple affected players use APNAP. Recompute after each
selection until none remain, then commit the resulting placement once. For
base three, Scales then Corpsejack gives eight; reverse order gives seven.
Do not mutate counters or expose a partially committed quantity while awaiting
the order decision. Budget suspension must preserve the exact prefix. Concession
or source/recipient departure at recorded boundaries requires revalidation;
already applied prefixes are not reapplied, and a removed recipient is never
resurrected. Replacement continuation must also compose with simultaneous event
groups rather than serializing unrelated actual mutations early.

CR 122.6 includes placements on entry. Route a would-be entry placement through
the same quantity processing with its prospective creature/controller context;
an entering Corpsejack's general replacement is not already active on itself
(614.4, 614.12). Current counter producers use ordinary post-entry instructions.
If entry-with-counters producers remain unavailable at a checkpoint, record and
enforce that input boundary explicitly; do not claim their acceptance. M2 must
not introduce an entry shortcut that bypasses the placement hook.

Serialize pending transaction, applied keys, continuation, mana receipts and use
ledger in authoritative rules state, covered by canonical hashes and ordinary
accepted-input replay. Validate positive original quantities, exact arithmetic,
valid source/ability/incarnation keys, no duplicate applications, valid turn
usage and prefix/cause reachability. Rehashed forged snapshots must still fail
replay. Use public object/effect handles and chooser-only replacement requests;
other seats see resulting public counters/use information, never private hands,
libraries, raw occurrence IDs or receipts. Publish enough public use availability
for a player to avoid a spent activation; query validity must use the same guard
without mutating state or minting history. Follow existing independent snapshot,
projection and query-format policies when changing their shapes.

## Implementation ownership and M2 boundary

The preparatory research checkpoint edited **only this document**. M0 then
handed native/shared-code ownership to the M1a integration owner. The table
records the original coordination boundary; M1a changed only the needed shared
modules and dedicated tests, leaving M2 fixtures and profile/consumer sets fixed.

| Future owner | Explicit paths / responsibility |
| --- | --- |
| M1 integration owner | `packages/game-engine/crates/contracts/src/lib.rs`; `compiler/src/lib.rs` and `compiler/src/self_reference.rs` beneath `crates/`: IR, schema/capabilities, complete AST consumption/binding. |
| Same integration owner | `packages/game-engine/crates/core/src/engine.rs`; `core/src/engine/{state,casting,payments,mana_permanents,triggers,tokens,events,effects,turns,choices,visibility,action_query,state_digest,snapshot_codec,recovery}.rs` beneath `crates/`: shared payment/events/state/choices/recovery. Edit only files actually required. |
| Same owner or explicitly partitioned test owner | New `ramos_*` / `counter_replacement_*` test modules in contracts/compiler/core; registration stays with shared-module owner. Source-backed accepted-pod witness under `core/tests/`. |
| M0/catalog integration owner | `packages/game-engine/tests/fixtures/` extraction, provenance, immutable development catalog/profile and expected outputs; coordinate exact additions after M0. No incidental changes to existing pools. |
| M2 owner | Land/ramp source research, isolated source-specific tests and proposed entry/search/payment interfaces. Shared files above require explicit handoff; parser edits additionally require that package's manual. |

The shared payment contract must distinguish **cost** (counter removal, later
life/tap/sacrifice), immediate mana **production**, and any separate trigger
(for example M2's becomes-tapped behavior). Preserve complete payment rollback
and genuine typed events. Ramos needs no life payment, mana choice, land-type
query or library access. M2 can independently prepare exact land/ramp lowering
and tests; it cannot simultaneously rewrite `PermanentActivationCosts`, receipt
layout, mana eligibility or entry handling. Multiple printed mana abilities need
an explicit ability selector/action-query change before M2 lifts the singleton
guard; reserve an internal index now, never select a color by guessing an
ability. Do not broaden sacrificed mana-source admission incidentally.

## Smallest next batches and test obligations

**M1a, after M0:** complete Ramos's full source, the cast-color instruction and
counter-removal mana/use/undo path together, with a separately identified small
development catalog and paid accepted-input witness. A contract/compiler-only
checkpoint may land earlier with source still rejected; never admit flying plus
the cast trigger while dropping the mana clause. Scales/Corpsejack and the exact
100-card target remain unadmitted. **M1b:** complete both replacement sources,
their typed transaction/order decision, replay/projection validation and composed
paid witness. Keep source support, development admission, exact-deck admission
and host/search qualification separate in M0's matrix.

| Check family | Meaningful positive and near-miss assertions |
| --- | --- |
| Frozen compiler/contracts | Exact Ramos lowers all three abilities; each complete replacement lowers its exact AST and roles. Mutate product reference/factor, counter kind/source, restriction, recipient/control scope, replacement add/multiply amount or add an unconsumed clause: reject whole source. Forged IR with the new instruction in an unbound trigger, unsupported cost combination or missing capability rejects even with otherwise valid provenance. |
| Color quantity | Paid colorless, mono-, two- and five-color casts give 0/1/2/5 before replacements. Repeated same-color symbols count once; mana color spent and five-color commander identity cannot change a colorless result. Opponent cast, failed cast, land play and a copied-but-not-cast spell collect no Ramos cast trigger. Actual copy mechanics remain M8: until then test that stack insertion cannot call the paid-cast boundary and keep copy sources rejected. |
| Source/reference lifetime | Counter the original spell before Ramos's trigger resolves: retain its color information. Remove/reenter Ramos: old trigger cannot touch the new incarnation. Spell-color changes require current/LKI tests when that separate input becomes supported; frozen cast facts alone are not that proof. |
| Mana/use/payment | Five counters pay to zero and add exactly W2/U2/B2/R2/G2 with no stack entry or color request; six leave one. Four fails atomically. Newly entered/tapped Ramos works; second activation in the same turn fails even after replenishment or control change. It works again on the next opponent's turn; another Ramos incarnation has independent use. Opponent/stale incarnation inputs fail. |
| Retain versus reverse | During a genuinely unpayable cast, activate Ramos and another nested mana source. Both reversal choices preserve correct pools, counters, use and priority; LIFO undo restores exact preexisting mana entries. Retained activation remains used. A subsequent successful paid spell triggers only after complete spending. No phantom placement, activation/cast trigger or receipt survives undo. |
| Replacements | Base three becomes 8 or 7 by affected-controller choice; base zero stays zero; distinct duplicate providers apply once each. Opponent-controlled creature and departed provider do not qualify; Corpsejack can modify placement on itself while already present. Separate placement occurrences receive separate applications. Include prospective entry tests when that producer is enabled and removal/undo controls in every case. |
| Accepted inputs/recovery | From a legal accepted pod, genuinely pay Ramos's command cast, gain at least five counters through paid casts, activate during payment, and spend the output on another legal spell. M1b adds paid replacement sources and both order branches. Snapshot at trigger placement, cast/mana spending and each order prefix; restore/replay with budget 1 and ordinary budget across trace modes. Test stale/foreign responses, use-ledger/applied-prefix tampering, chooser-only requests and nonmutating action queries. Synthetic state tests supplement, not replace, this witness. |

Choose focused contracts/compiler tests for the new validation/lowering and the
specific core/accepted-input tests above; confirm nonzero selected counts under
the shared Cargo lock once the native owner hands it off. No automatic full
matrix. Host/SDK, search, cascade, copying, spell-color mutation, full target
completion and broader profile/release matrices require separately scoped gates.

## This preparatory checkpoint's verification

Read the required manuals, workflow, current map, plan, relevant invariants and
the source modules identified above. Rehashed the full frozen archive and rules;
verified all three raw source/AST/binding hash envelopes and line/aggregate AST
mirrors directly with Python. Those source-integrity checks do not execute the
production compiler or prove new engine semantics.

Doctor returned exit 1: pinned Node/Rust/Cargo and auxiliary tools were present,
but this fresh isolated checkout lacked TypeScript, `@types/node`, Ajv and the
two npm workspace links; no upstream was configured yet. No installation was
needed for this document-only batch. No Cargo/build/native tests, production
audit, schema generation or fixtures were run or changed; M0 retains native
ownership. Final document checks cover whitespace, local links, authority pins
and the single-owned-path diff. Existing ignored evidence/caches are preserved.

## Bounded M1a implementation

The complete frozen Ramos record lowers flying, the controller spell-cast
trigger and the full restricted mana clause together. `ref:self` continues
through the existing archive-bound source canonicalizer. The color product is
consumed only as the exact counter expression; it does not add a general count
interpreter. `CastSpellColorCountersV1` and `CounterRemovalManaV1` fence these
semantics separately; the latter covers both the counter cost and restriction.
IR schema stays 1. Singleton mana-source actions remain unchanged, while the
internal key reserves `(incarnation, Mana, index 0, current turn number)`.

At resolution the cast witness identifies the exact stack spell. Current colors
are read while that incarnation remains there; otherwise its frozen immutable
colors are the bounded departure-LKI witness under CR 608.2h. No admitted IR
instruction changes spell colors. Such an extension must add separately refreshed
departure information and tests before admission; historical cast facts must not
be rewritten. A colorless spell creates no placement event. Source departure and
reentry cannot redirect an old trigger to the new permanent.

The no-tap mana path revalidates controller, incarnation, counters and use before
payment. Counter removal is a cost within the staged atomic rule step, never a
placement. Its receipt records exact prior counters/use and the existing nested
spent/produced/absent-pool facts. LIFO undo restores those facts directly and
retained activation keeps its use spent. Turn transition clears use; zone movement
prunes that incarnation. Control changes leave use intact. The current-turn set
is bounded by live objects, not lifetime activations. Public rows expose optional
`mana_ability_used`; historical sources omit it. Snapshot format is 20;
observation/search-context and query work formats are 17. Exact build/catalog
identity and accepted-input replay still authenticate recovery.

The dedicated core witness uses a private 9-definition catalog (10 with the
nested Signet) and constructs an established midgame with four
legal 100-card singleton/color-identity Commander decks and ordinary zones. It
starts with lands already developed and six known cards in the controller's hand;
it is not an opening-hand simulation, registered development profile, admitted
pod, or the revision-17 target. After `Game` construction it uses only accepted
inputs: paid command cast, five distinct paid green spells, five resolved
counters, immediate Ramos activation during another real cast's mana window,
then exact spending and completion. Snapshot/replay, budgeted nonmutating query
recovery and other-seat privacy are checked across Off/Compact/Detailed traces.
Synthetic color/reference/control/payment edges supplement this source witness.
The shared host/search allowlists and every existing profile/fixture stay fixed.

The final family guard follows the workflow's parameterization policy added on
2026-11-14: any positive source +1/+1 counter quantity and nonempty fixed-symbol
output share the same operation/capability. Counter-removal mana still requires
once-each-turn and excludes tap/mana/sacrifice combinations, choice production,
and multiple public ability selectors. Exact Ramos remains five for ten. Typed IR
variation tests use costs 1/3/7 and several fixed outputs; a synthetic runtime
case removes three from four and adds R/G/G while tapped and unready. The frozen
fixture pool supplies no second complete source with this supported combination;
these variations are explicitly synthetic, not additional source admission.
Missing restriction, zero/negative counter costs, empty/choice output and extra
cost combinations remain actual semantic rejection controls.

## M1a focused verification and omissions

The first complete pass ran 328 selected tests: 204 compiler, 77 contracts,
7 Ramos/M2 split-search core, 14 permanent-mana, 3 payment-event, 22 cast-trigger
and 1 old paid-opening search regression. Sim/server all-target compilation
passed. After the family-parameter guard update, 204 compiler, 78 contracts,
5 M1a core, 3 payment-query and 9 snapshot-codec tests passed, with sim/server
all-target compilation again passing. The final test-only private-catalog
membership refinement passed the 5 M1a core tests. The compiler suite checks all
committed catalogs remain fresh; no fixture or profile bytes were regenerated.

Raw attempts, exact source patches/new-test hashes, frozen fixture byte proof,
source/AST/binding envelopes and preserved executed binaries are retained under
`generated/ramos/m1a/`; `attempts.json` is the attempt receipt. It retains the
initial missing-field/syntax compile failures, the missing capability-witness
failure (75 pass/1 fail), and the core test-type compile failure. Failed attempts
are not counted as qualification. The first complete and final family pass bind
different build IDs; their binaries are preserved separately. No timestamp repair
or cache deletion was needed. The global lock covered preflight, native work and
binary preservation; it was deliberately yielded to the other coordinator's
queued combined qualification before the successful passes.

The doctor observed missing isolated Node dependencies/workspace links. The
known root npm lock/workspace disagreement blocks bootstrap (`npm EUSAGE` in
prior retained evidence); no installer was retried and no unrelated manifest or
lockfile was modified. Node/source Ajv/SDK gates, Clippy, release/full-runtime/
full-game matrices and exact-deck/host/search qualification are omitted. M1b
Scales/Corpsejack replacements and all excluded mechanic families remain work
for later batches. The paid source witness does not qualify a registered pod.

Final M1a optimized-test build is
`19f603234dd46e8e4f0bac633bcd091e598f678c954ba744214088952fa3e1ee`;
the 9-definition paid witness catalog is
`010399dd4d5ce1fc000e59c1da69c0fd4fb95c3bb4f7435f917cd7c0104af52c`.
The final catalog-only test refinement has byte-identical production Rust to the
preceding family pass (`7399de7bb5c2dc38afbead506fed671f479c233fb29228815d4d641709361a1a`).
That comparison and the final executed binaries' SHA-256 hashes are in the attempt
receipt. Work exceeded the 30-minute checkpoint target, including the coordinated
native reservation; the final handoff records actual elapsed and native timings.
No M1b work began, no main merge or PR was made, and later fresh-main combination
checks belong to the parent.

## Parent integration and exported-schema agreement

Parent source `20ca217d` combines M1a with main
`ac95b1cfb393342218e655435ed03398d7c47fd7` (temporary deathtouch,
compound protection and Collar admission). Only additive test-module registration
conflicts needed resolution. Existing catalog/profile admission remains distinct
from the Ramos private source witness and historical client acceptance.

Review found that the exported cast-color trigger constraint used the wrong
serialized spell-predicate shape. External Ajv validation reproduced rejection
of complete compiled Ramos; an isolated follow-up also found acceptance of a
null counter cost on restricted mana. The correction derives the condition from
the actual Rust enum, requires a positive non-null counter quantity, and treats
ordinary null optional extensions as `None`, consistently with Rust decoding.
The family remains parameterized and no gameplay rule changed.

The corrected schema passed 83 contracts and 207 compiler tests plus an external
Node regression with 22 schema probes on complete production-compiled Ramos,
parameter variations, invalid costs/restrictions/capabilities and ordinary mana.
The same helper is registered in `tests/contracts.test.ts`. Parent used its healthy
existing Node/Ajv installation; the isolated child's dependency omission remains
historical. Full Node/source, SDK, Clippy and release/full matrices remain omitted.

Separate producer-qualified evidence is retained in parent
`generated/ramos/combined-20ca217d*/` and `generated/ramos/schema-final/`.
`schema-final/source-bridge.json` proves byte-identical core/compiler/simulator/
server files and identical contracts code outside `catalog_schema`; copied rules
test binaries keep their original producer identity. Schema checks use the new
copied CLI. Interrupted attempts and zero-match selectors remain recorded and
are not treated as passed tests. This does not qualify the exact deck or host.

## Bounded M1b counter-placement family

M1b starts at `5f3ea51d4f69343f1f887fdb96e288132e43daae`,
2026-11-14 17:11:29 UTC, in its own orchestrator checkout. It completes both
unchanged archived replacement sources above, independently of their retained
0/2 diagnostic binding flags. Strict typed lowering consumes the affected
creature/control predicate, positive threshold, counter kind, any cause, each
occurrence and the entire quantity change. Corpsejack's full creature
characteristics and ordinary permanent program are retained. No source fixture,
AST, binding record, admission catalog, exact revision-17 target or host/search
capability set changes.

`ControllerCreaturePlusOneCounterReplacementV1` is one mechanic-family capability.
Its `CounterPlacementModification` is `Add { amount }` or `Multiply { factor }`,
with arbitrary positive exact parameters. Runtime and validation do not branch
on names, Oracle IDs or printed one/two. Negative/zero parameters, unsupported
source kinds/zones, unknown fields and missing/extra capability closure reject.
The exported schema uses the actual Rust enum encoding and remains schema 1.

`counter_placement.rs` owns a separate typed transaction, not a damage alias.
The exact resolving stack incarnation/cursor/cause identify the original singleton
placement and continuation. An applied key is exact provider incarnation plus
static ability index; each prefix retains its qualification and exact arithmetic.
Every prefix recomputes current creature/controller/provider applicability.
Counter state changes once, at final commit; quantity zero never enters the
positive transaction. The affected current controller chooses the next effect
under CR 616.1. No provider/recipient is resurrected, and an already applied key
never runs again. Concessions invalidate stale queued choices while preserving
accepted input/replay and completed prefixes.

Producer inspection covered all executable placements: fixed target spells,
self-enters target/source instructions, sacrifice-source activations, step/cast
triggers and Ramos's color quantity, all through `execute_token_instruction`.
Existing source witnesses include Battlegrowth, Ironpaw Aspirant, Satyr
Grovedancer, Carrion Feeder, Fretwork Colony, Generous Visitor, Pyre Hound and
Managorger Hydra. Counter-removal costs and receipt undo use their separate
existing authority. Raw placement events cannot bypass the transaction.
There is no admitted entry-with-counters or simultaneous-placement producer;
CR 122.6/614.12 entry contexts and APNAP placement groups remain unavailable.
The explicit aggregate guard rejects that unsupported input, and complete Abzan
Charm remains rejected rather than being silently serialized. Other simultaneous
families retain their existing authority and bounds.

The current authoritative frame changes snapshot 20 to **21**. The independent
public request/response union changes observation/search-context 17 to **18**;
incremental query decision scratch and its replay validation change private work
17 to **18**. No unrelated bundle/sample/IR version or consumer set changes.
A chooser-only order request carries public provider object handles with public
ability indices and the allowed before/after quantities. Raw stack/cause IDs,
provider qualification records, prefixes and receipts remain trusted only.
Queries do not mutate state/trace or mint handles; ordinary visibility refresh
supplies public source-incarnation handles. Existing legacy policies explicitly
refuse this unadmitted window without choosing an order or expanding admission.

The small private source witness pays for complete Ramos, Scales and Corpsejack
from ordinary initial zones and lands, then exercises both orders, repeated
placements and a paid colorless zero control. A separate synthetic color-count
variation witnesses exactly 3→8 and 3→7 with snapshots before input, pending input,
each applied prefix and final commit. Tests cover fixed producers, a genuinely
borrowed Carrion Feeder, owner concession before/after a prefix, recipient
concession, stale/foreign/applied-prefix responses, query save/restore, forged
rehashes and payment removal/retain/undo with both providers active. Explicit
synthetic zone probes cover duplicate providers, source reentry and recipient
reentry; no reachable mid-transaction entry producer is claimed.

Independent read-only production/fixture review reported no actionable findings;
all twelve production hashes remained equal throughout that review. Its manifest
is retained at
`/home/josh/.t3/worktrees/mtg-ecosystem/t3-02328819-365b7a4025a6b944/generated/reviews/m1b-current-frozen-review-manifest.json`.
Later changes only add explicit legacy-policy refusal arms and refine/add tests;
the reviewed contracts/compiler/core replacement bytes remain unchanged.


### M1b focused verification and evidence

The final replacement-runtime pass runs **7/7** dedicated core tests, and the
preserved exact test executable runs **70/70** focused regressions: M1a/M2,
controlled-creature damage, snapshot codec, payment query, Work/spool and related
query recovery. Compiler library tests pass **210/210**. Contracts initially
pass **83/84**, with only the capability vocabulary/witness test missing the new
family registration; that test-only omission is repaired and the affected test
passes **1/1** in attempt 14.
The failed full package result remains retained rather than being relabeled.

The own-source dev CLI is copied under the lock before export. Its compiled
source manifest verifies 872 inputs; BUILD_ID is
`d0a07fd23f214a9b46fccd1e21fa5cbb2551196dffe5dae66ec128d90370f357`,
binary SHA-256 `7740454266c36f94f2dae801c5d32f2b15f87d04551f5a027bf3e4e88f1fc979`.
It uses Rust 1.99.0, x86_64, optimization 0, debug/assertions enabled, no extra
Rust flags and incremental disabled. Tests use the ordinary optimized test
profile with checks enabled; their exact binaries and separate dirty-source
inventories are retained. The CLI producer predates only later test-helper and
capability-witness edits; its retained source snapshot qualifies its exact build.
The production prefix of contracts `lib.rs` is unchanged by that `cfg(test)` repair.

Raw evidence lives in this checkout's `generated/ramos-m1b/`. Each measured
attempt retains command/resource/lock receipts, byte inventories and source
copies; executed test binaries are copied before lock release. Attempt 11 also
retains the own CLI, source manifest, build settings and complete-source schema/IR
exports. Earlier failures preserve the wrong initial manifest invocation,
strict source-filter decoding error, missing private Sol Ring definition,
test lifecycle/visibility assumptions, stale shared-cache contracts diagnostics,
and the capability-witness registration omission. A narrow, byte-preserving
contracts timestamp refresh under lock forces the own source after cross-worktree
cache reuse; stale library hashes/metadata and unchanged before/after source hashes
are recorded. No cache or historical evidence is deleted.

The known isolated Node dependency/EUSAGE condition remains unchanged; no installer
or unrelated manifest/lockfile is touched. The parent runs the builtin-only
`counter-placement-contracts.ts` assertions with existing external Ajv against
these immutable own exports, without importing root engine code or running root
Cargo. Its first attempt reveals only a helper expectation: canonical absent
optional mana is omitted, while null is another supported Option input. The helper
is corrected to the actual canonical omission and the failed gate is preserved.
The full rerun is **30/30** (10 positives, 20 negatives), using Ajv 8.20.0.
The parent verifies all four artifact pins before/after and all 872 compiled
source rows against retained/current matching source bytes. The green receipt is
`/home/josh/.t3/worktrees/mtg-ecosystem/feat-engine-ramos-integ-83ca7e724cff4f47/generated/ramos/m1b-ajv-attempt11-canonical-none/result.json`;
the failed helper attempt is retained beside it.

Omitted: release/full workspace/runtime/profile/completion/retention matrices,
Clippy, full Node parser/source gate, all-target server/SDK/client builds, exact
deck/pod admission, and host/search qualification. Source identity, focused tests
and synthetic kernel witnesses do not grant any of those consumer claims.
