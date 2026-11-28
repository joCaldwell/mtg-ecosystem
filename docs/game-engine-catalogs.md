# Catalogs, profiles and build identity

Fixture paths below are relative to `packages/game-engine/`. Use frozen source
records, not refreshed card facts. [Development](game-engine-development.md#cargo-coordination-and-cache-integration)
owns the pinned environment and per-worktree Cargo lock.

## Catalog source and commands

Default consumers use Archive R5. The full archives stay outside Git;
`tests/fixtures/phase1-r5/` holds only the extracted records a manifest, witness
or compiler test uses. `accepted-archives.json` pins the whole file, header,
parser and corpus, and each `provenance.json` row pins its exact card line.
R4 (same corpus, older parser) stays registered without record pins so its
bulk IR can be rebuilt for comparison. Exact registered headers choose the
reader: R4/R5 use schema 4 and check the header's parser revision; R6 uses
schema 5, records only parser source identity in the header, and takes its
parser revision (the reviewed producer commit) for bulk v2 from the external
registry. A pending registration cannot authorize intake or provenance. A
`candidate` registration authorizes intake and comparison but no record pins,
so no tracked fixture or default consumer can select it; a consumer explicitly
pointed at a candidate file can load it. Oracle parser tests own their pre-R4
records under `packages/oracle-parser/test/fixtures/ramos/`.

The promoted R6 cut carries printed evidence, retains the R5 row bytes projected
by the [D0 archive check](oracle_parser.md#ast-archive), and has its origin under
`phase1-r6/`. Each R6 fixture keeps the same archive line and adds only
`printed` and `printedSha256` to the R5 row; both archives verify for all
R5-origin fixture identities, and the R5 origin and fixture manifests supply the
default controls.

From repository root (`engine:dev` builds `mtg-sim` under the target lock):

```sh
npm run engine:dev -- build -p mtg-sim
SIM=target/engine/debug/mtg-sim
$SIM catalog build packages/game-engine/tests/fixtures/catalogs/phase1-r5.json [--write]
$SIM catalog extract /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json 'Exact Card Name' oracle-id
$SIM catalog verify /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json
```

Filesystem and embedded loaders both lower every selected record through
composition. `build` checks the schema-1 catalog and digest against the
committed outputs, `--write` regenerates both, and `mtg-compiler` unit tests
check every manifest without writing. Base, action and match manifests supply
the smoke, scenario and Foundations startup fixtures; phase1-r5 and witness-r5
exercise individual compiler and rules paths. A failed build prints its typed
`CatalogError` as JSON on stderr.

`catalogs/*.json` manifests (expected IR lives outside that directory) have
fields `schema_version: 1`, `provenance`, `records`, `expected_ir`,
`expected_digest` and `output_order`: `canonical` (sorted compact JSON),
`sorted` (sorted pretty) or `struct` (typed field-order pretty). Manifests define
record membership and order. Manifest paths are relative to the manifest,
source/archive paths to provenance. Digest files hold SHA-256 without a newline.

Nonland records with empty printed mana costs reject: absent cost is unpayable,
distinct from `{0}` (CR 118.6/118.6a), and the IR cannot yet represent it. Lands
may have empty cost. A transforming card's back face is never cast and requires
an empty cost, recording its front face's mana value instead (CR 202.3b).
Missing/null/malformed mana-cost fields reject. Optional AST filters (cast,
attacker, recursive damage-source class/type/Boolean, attacker-control
relations) reject explicit null; omission and valid nonempty filters keep their
meanings.

`extract` resolves each exact case-sensitive name/Oracle selector once, hashes
the whole archive before writing, preserves raw lines and terminators, uses Oracle-ID filenames,
extends record pins and appends line/hash provenance; conflicting selections fail
before writes, and unparsed records keep null AST hashes and cannot compile, and excluded envelopes stay intact. A
fresh destination needs provenance metadata with `records: []` and
accepted-archive metadata beside it; archive identity is per record, and a top-level `archive_manifest_sha256` rejects. `verify`
compares every row for that archive with its actual line and exact fixture
bytes and rejects missing, duplicate or out-of-range lines, changed hashes and
identity mismatches, reporting other origins separately. Build inputs embed manifests, provenance, accepted
headers/pins and records, excluding expected IR, policy copies and evidence.
Missing scenario digests fail admission without preventing diagnostic CLI builds.

### Promote or re-pin R6

After reviewing the candidate's D0 receipt and producer commit, build
`mtg-sim` from the current checkout and run the promotion script with a new
scratch directory each time:

```sh
sh scripts/engine-env.sh npm run engine:dev -- build -p mtg-sim
sh scripts/promote-archive-cut.sh /path/to/r6-candidate.jsonl /path/to/d0.json \
  --r5 /path/to/oracle-ast-phase1-r5.jsonl --sim target/engine/debug/mtg-sim \
  --producer-revision REVIEWED_COMMIT --out generated/r6-promotion
```

D0 records only the producer's source digest, so supply the reviewed 40-digit
commit. The script reruns D0 against the verified report, re-extracts every R5
identity with `--promotion` (earlier origins must match the binary's embedded
pins), verifies against both full archives, and installs the R6 fixtures,
registry, provenance, frozen inputs and registry test header reference only if
every line number, exact R5 row and the two evidence fields match. R6
provenance follows Oracle ID order after the R5 rows, so re-promotion with the
same inputs reproduces the same tracked bytes; the candidate basename becomes
the frozen compile cache name. For a scratch check of the registered cut, use
`scripts/compare-archive-cut.py fixtures FIXTURES OUT --sim SIM --archives R5 R6`.

`scripts/compare-archive-cut.py d1 BEFORE AFTER` compares bulk payload bytes,
coverage, blockers, four tier decisions from the compiler's
`compare-archive-tiers` example, and native admission witnesses.
`d2 BEFORE AFTER --archives R5 R6` also verifies the exact legacy row prefixes,
substituting only the two definition archive/parser provenance digests
(including back faces), the two independently verified index row digests on
changed rows, and the manifest's archive/parser identities, catalog digest and
payload hashes; producer, payload lengths and all decisions must agree.

### Rust printed evidence

Intake accepts a registered R6 cut only when its printed schema, vocabulary and
generator pins name the compiled decoder; the accepted generators are listed
explicitly (the current one and R6 v4, whose decoder a compiler test proves
identical except its generator line). The intake verifier (`compiler/src/ingest/r6.rs`,
`ingest/printed/evidence.rs`) applies P1–P10 to each applicable R6 row before
its card is assessed; the [parser contract](oracle_parser.md#ast-archive) owns
the printed wire format and evidence bases. P6 rebuilds quote regions from the
original text through verified normalization, lexing and punctuation order.
Every matched quote pair in a successfully read block needs an owner: a quoted
grant's AST body, or the exact `the "legend rule" doesn't apply` and `all "bands with other" abilities`
terms when their located production and complete pair
are in each covering run set. An ancestor's span may include an owned quote with
both marks; terms stay in the enclosing ability's scope. P7 requires a fact and
its owner, and P9 the target token, its fact owner and the linked selection
node, to share a region.

A family that opts in receives a verified `CardEvidence`, queries its face and
block before lowering, and reports a per-card `evidence:requires-r6` (face,
block, obligation) when an R5 input lacks the evidence. `BlockEvidence` locates
AST object nodes and facts in original UTF-8 face bytes through recorded runs
and folds, aliases, implicit nodes with closed reasons, or hulls; an unlocated
object, an unusable alias or a root-assigned fact for uncovered tokens yields a
located `evidence:missing`. P11 requires the resolver to discharge each field
of the block AST with a verified exact link, a production-specific check of the
printed phrase and value, or a closed default at its governing node. P12
requires a checked meaning or structural role for every printed fact and token;
structural roles are verified [punctuation facts](../packages/oracle-parser/src/printed/facts.ts)
at a production boundary (sentence periods, trigger and for-each clause commas,
activation cost list commas, the cost/effect colon), and operators such as signs
and the power/toughness slash need a checked field meaning with a bound fact.
An undischarged field or unconsumed occurrence, such as an omitted qualifier,
rejects the card with an original byte range.

Whole-card source authorities register in order in
[`compose/source_authority.rs`](../packages/game-engine/crates/compiler/src/compose/source_authority.rs);
composition asks the registry before reading any legacy resolution, and two
families claiming one card is a registration fault. Each family's module
rustdoc owns its supported and rejected shapes (currently
[player actions](../packages/game-engine/crates/compiler/src/compose/source_action.rs)).
R6 derives every covered run from the recorded node and fact citations and
records the consumed `printed_sha256` in definition provenance; the exact R5
sentence remains only as the R5 proof. The trace lists each selection by
family, block-relative path and kind, and the occurrence differential
(`scripts/rework-reference-diff.py`) joins it to every frozen binder reference
and projection in the claimed block.

Run the frozen witness and mutation harness against the pinned candidate with
`MTG_R6_VERIFIER_ROWS` set to its path. Verifier-only mutations rehash their
payloads over unchanged Oracle text; detached source probes re-read changed text
through the pinned parser and verify the new AST and evidence or require a
located rejection.

```sh
sh scripts/engine-env.sh npm run engine:dev -- test -p mtg-compiler --lib frozen_family_evidence -- --ignored --nocapture
```

## IR and build identity

`mtg_contracts::CURRENT_IR_VERSION` is the sole executable IR authority, fixed at
**1** until an external consumer needs compatibility. It evolves in place under
strict capability/shape validation, with no old decoder, version range,
per-feature schema gate or duplicate historical expected output. Archive,
snapshot, bundle, public and query formats have separate identities;
[evidence](game-engine-evidence.md) retains historical versions.

The engine build identity (`mtg_contracts::engine_build_input`) hashes
production engine Rust sources and Cargo manifests, the root Cargo
manifest/lock, toolchain and compiler/target/settings. Test-only sources
(`tests/`, `examples/`, `benches/`, `tests.rs`, `*_tests.rs`, `*_tests/`, the
`testkit` module and the dev-only `crates/witness` package), fixtures,
generated output and server source are excluded; catalog/pod identity and
`SERVER_BUILD_ID` bind those separately. Data-only catalog additions do not
invalidate unchanged-catalog snapshots.

`mtg-sim` and `mtg-server` compute the identity in `build.rs` through
`crates/build_identity.rs`; `main` calls `mtg_core::install_build_identity`
before any engine use. Core never reads build-time environment: snapshots, saved
work and search artifacts stamp and admit `mtg_core::build_id()`, and a process
that installs nothing uses `BuildIdentity::TEST`, which never matches a native
artifact. Core `build.rs` only embeds catalog/profile data (typed validation runs
in `match_profile_validation_tests`), so compiler, simulator, host or test-only
edits never rebuild core. Build scripts watch each identity, catalog,
provenance, archive, record, profile and ban-policy input individually
(including absent repairable ones), so README or test edits do not rebuild; a
new production module edits a watched `mod` declaration, and a new `build.rs`
without a manifest edit is not detected ([Cargo rerun-if-changed](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)).

`crates/core/src/formats.rs` owns the core wire/persistence versions
(`SNAPSHOT_VERSION`, `OBSERVATION_VERSION`, `WORK_VERSION`, search envelopes).
`formats_tests` pins the Observation schema and Snapshot/SavedWork shape
fingerprints, with samples that include resolving and nested Kernel-IR choices.
Tests import the constants; deliberate historical forgeries use literals.

`python3 scripts/engine-structure-check.py` verifies declared direct dependency
boundaries (core has no compiler edge), `#[cfg(test)]` test modules and
`#[cfg(feature = "testkit")]` testkit modules enabled only by test crates, and
that the Git index holds no known generated paths or ELF/Wasm/SQLite output; it
never alters the index, and renamed JSON and transitive features still need
review. Guard tests: `python3 scripts/test-engine-structure.py` and
`python3 scripts/test-engine-cache.py`.

## Kernel IR

`Instruction::Kernel { node: kir::Node, source_block: u32 }` is the sole
executable instruction in schema 1; the block index identifies the instruction's
source on the resolving stack object. Under `contracts/src/kir/`, `refs.rs`
defines the closed reference vocabulary, `PlayerExpr`/`SetExpr`/`Amount` build
operands from references and `pred.rs` atoms, `verbs.rs` defines one game action
per `Verb`, and `nodes.rs` composes them (`Do`, `Seq`, `If`, `Optional`,
`ForEach`, `Choose`, `ChooseObject`, `TapTargets`, `RemoveTargets`,
`UnlessPays`, `IfPays`, `Delayed`, `Reflexive`).

`kir::check(node, env)` is the one admission decision. It proves each reference
is in scope and of the right kind for the context, targets and event; rejects a
filtered count of a hand or library (CR 400.2); and admits a verb only in an
audited cell: a `kir/check/*.rs` row naming the `kir::Cell` it admits, which the
core interpreter (`core/src/engine/kir/exec/`) executes and recovers. A
well-scoped node without a cell rejects as unaudited. Composition checks each
program and derives required capabilities from the node and its operands; the
JSON Schema describes wire structure only. The resolving frame resumes at an
audited control node, with bindings and progress tied to its address in the
admitted program. [Core](game-engine-mechanics.md) and [board](game-engine-board-mechanics.md)
mechanics describe each family's behavior, including counted target groups and
sacrifice-for-return.

- `Seq` follows written order. `If` supports `Compare`, `Exists`, `CostPaid`,
  `Not` and `All`; `Satisfies` can test the printed creature type of a
  bound graveyard card after exile (CR 608.2b, 608.2h). `ForEach` binds a
  player or a singleton object.
- `UnlessPays` and `IfPays` take a singleton player expression and support
  life and generic mana. With no live payer only `UnlessPays` performs its
  node (CR 118.12). See [resolution choices and continuations](../packages/game-engine/crates/core/src/engine/kir/control.rs)
  for optional effects, creature-type choices and APNAP draws.
- `Choose` binds a pool from a matching public set when both bounds force zero
  or the set's size. `ChooseObject` offers one mandatory public object choice
  from a current controller-relative battlefield set; the saved frame derives
  candidates from current state and consumes an answer only against a matching
  offer (CR 608.2d, 800.4a).
- `Delayed` and `Reflexive` carry separately checked single-instruction
  programs; the parent can register a one-shot [resolution-created trigger](game-engine-board-mechanics.md#delayed-and-reflexive-triggers)
  at its end.

Controls read fixed amounts, announced X and audited set counts; verbs also read
[resolution-time counts](game-engine-board-mechanics.md#resolution-time-counts).
Life changes, payments and choice bounds need nonnegative amounts. Object sets
are matching battlefield permanents or graveyard cards under audited predicates,
source or announced identities, a bound singleton or a bound pool. Tapped,
combat and controller filters require the battlefield (CR 110.5d, 506.3–4,
109.4), including in predicate alternatives; graveyard sets exclude tokens
(CR 111.6). Player expressions name the controller, a bound player, the caster
or active event player, a player target in a prefix of ordinary player slots,
the recorded owner of the source or a singleton iteration object, or the
controller a filtered target's frame captured (CR 608.2h); loops and life leaves
can read each player or each opponent. Object stats and cast-history amounts
stay outside control cells. Branches gated by an optional-cost condition
supply no targets until cost-dependent target declarations exist (CR 601.2c).

Every branch and leaf must pass the checker. Draw, life gain or loss, damage,
counters, sacrifice, discard, mill and zone changes use the signature evaluator
in the resolving task, including under controls. Other ordinary leaves use the
flat executor without task bindings, reading at most one announced target.
Token creation and temporary effects need persistent leaf provenance, and set
damage retains facts tied to the top-level cursor. Optional target destruction
keeps its standalone route. Counter targets one announced stack entry and can
read its spell controller for a fixed generic payment. Regenerate records one
source or announced permanent. Control nodes derive capabilities from their
leaves; continuation-based nodes also require `KernelControlV1`, outside the
reviewed hosting and search sets. A procedure without an audited node and
execution path rejects during lowering; a new mechanic adds a typed operand,
capability and tested rules operation only where the algebra cannot express it.

### Verb signatures

Draw, life, damage, permanent counter placement, sacrifice, discard, mill,
token creation (`CreateTokens`) and zone changes (`Move`) use `kir::signatures`.
The resolving spell shuffling itself (CR 608.2n) and the resolution object
choice stay outside it. `kir::Verb` remains the wire representation. Damage
prevention and commit, counter placement and graveyard moves retain their
transaction authorities.

A slot admits an operand whose class it accepts and that resolves in scope; no
row names a verb and reference pair. Scope is the checker's `Env` (context,
announced targets, triggering event, X and the checked cost); lexical bindings
live in checker scope. Sorts must match. One verb-independent classifier maps a
resolved reference and its typed AST site to a class. Required capabilities are
the union over the verb, its operand classes, distribution and projections.
Each slot (`kir::signatures`) declares its field and role, sort and
cardinality, quantity policy and zero rule, zone, read time and object use (live
incarnation, event snapshot or current-or-last-known), absence behavior (skip
the member, skip the verb or reject, CR 608.2b), accepted operand classes and
added capabilities. Last-known information answers a read and never authorizes
acting on a new incarnation (CR 400.7, 608.2h).

Battlefield sets and permanent targets share counter-balance predicates over a
specified counter kind or the total of all kinds on the current incarnation
(CR 122.1); announced targets recheck them at resolution (CR 601.2c, 608.2b),
and Observation includes them in public target specifications. Sets and
targets also share color predicates over current colors (CR 105.2, 613.1e).
Counter amounts can read an announced permanent's current or last-known balance
(CR 608.2h). Battlefield
movement and destruction sets can compare mana value against announced X (CR
107.3, 202.3). A quantity may read the greatest current stat in a battlefield
set or count a resolved player's public graveyard cards or hand size (CR 402.3,
404.1, 608.2h); characteristic filters on hand counts reject (CR 400.2). An
illegal target cannot supply a player identity for a zone count or relative
filter (CR 608.2b).

A triggered invocation stores an `EventBinding` for the event's player, object
and quantity when supplied. Death reads the common prior battlefield. An
entrant or attacker whose power is read at resolution uses the exact live
incarnation or its departure facts; a later incarnation cannot receive an
action for the original one. A cast spell's mana value is its stack value with
the announced X the cast witness recorded, which stays its last known value
after the spell leaves (CR 202.3e).

The Rust [pronoun resolver](../packages/game-engine/crates/compiler/src/rules/verbs/antecedent.rs)
owns player and object candidates, their scope and their located rejections.
`scripts/rework-reference-diff.py` compares bound object references against
the frozen archive at sweep time.

[Modal announcement and repeated occurrences](../packages/game-engine/crates/core/src/engine/modal.rs)
own the selection and public projection contract.

The [card result resolver](../packages/game-engine/crates/compiler/src/rules/verbs/card_result.rs)
owns printed links to actual card results. Card predicates read the exact public
successor or its last known characteristics (CR 400.7, 608.2h, 701.9c, 701.17c).
A destination test uses the recorded move. A card hidden in both origin and
destination supplies no characteristics.

**Distribution** is admitted per verb, separately from operand acceptance:
accepting a player set does not license a simultaneous or divided write.
`Single` and `EachSequential` act on one member or members in turn;
`EachSimultaneous`, or `CollectThenSimultaneous` after APNAP choice collection
(CR 101.4), make one event for every member; `DividedAtAnnouncement` rejects.

**Projections.** A step binds only a declared projection of a dominating
performed step; each `(verb, StepRole)` has exactly one: the actual result after
replacement.

| Verb | Quantity | Distribution | `Amount` | `Objects` | `Player` |
| --- | --- | --- | --- | --- | --- |
| `Draw` | ≥ 0; zero makes no attempt | `EachSequential` (CR 121.2c) | draws completed | drawn successors, private | players who drew |
| `GainLife` / `LoseLife` | ≥ 0; zero is no event (CR 119.9–10) | `EachSimultaneous` | life actually gained or lost | none | players whose total changed |
| `Damage` | > 0 | `EachSimultaneous` through the damage transaction | damage dealt, never life lost or prevented | permanents dealt damage | players dealt damage |
| `PutCounters` | > 0 | `EachSimultaneous` | counters actually put | recipients | none |
| `Sacrifice` | > 0, as many as possible (CR 609.3) | `CollectThenSimultaneous` (CR 701.21a) | number sacrificed | permanents sacrificed, whatever the destination | sacrificing players |
| `CreateTokens` | ≥ 0; zero creates nothing (CR 111.1) | `Single`, one creation event | none | none | none |
| `Move` | none | `EachSimultaneous` (CR 608.2c) | objects that reached the destination | their prior and successor incarnations; public facts unless the destination is hidden | none |

A second projection of one verb ("life lost this way" from damage, "died this
way") rejects until a `StepRole` is added additively. `Ref::StepResult { step, role }`
keeps its wire form; the checker resolves it to one producer among earlier
siblings of its `Seq`, rejects ambiguity, and requires that step to hold exactly
one `Do` leaf, directly or under `Optional`, `If` or `IfPays`; nested-body reads
wait for Stage 2. **Absent is not zero.** A producer that did not run yields
`Resolved::Absent` with its reason (illegal target, not chosen, departed player,
no event member or not performed); one that ran and affected nothing yields an
empty result.

A `CostResult` for an activated ability is scoped to a mandatory cost that
sacrifices one selected permanent and exposes the paid object's last-known
facts, the payer and the number paid. A sacrifice action exposes the actual
number, objects and players through the ordinary step collector.

**Tier preservation.** The 17 previously qualified higher-tier shapes in
`kir::tiers` keep their legacy cells and tiers as labels the evaluator performs
(a creature target to its owner's hand or library top among them); every other
evaluator-admitted node is `Cell::Evaluated(verb)` at `Executes`.

**Zone changes** read their members once as the step is performed: the
source's or an announced object's same incarnation in its zone, or a matching
battlefield or public graveyard set (CR 400.7, 404.1, 608.2b,h). The
[move checker](../packages/game-engine/crates/contracts/src/kir/check/move_zone.rs)
owns the admitted domains and destinations. Each goes to its owner's hand, library or
exile (CR 400.3), or enters under the mover's control (CR 110.2a). A commander
bound for a hand or library waits for its owner's command-zone choice in APNAP
order (CR 903.9b); a card entering asks the shared card-entry questions (Aura
host, entry payment, reveal; CR 303.4f, 614.12). Then the members move in one
event through the canonical actuals, and a library shuffle follows in the same
transaction (CR 701.24c).

### Resolving frame

The `Task` chain keeps lexical paths and scoped bindings in `Frame::Kernel`;
`Task.resume` links to the next scheduled task or to the collector at
`frames[1]`. `ForEach` runs every iteration at `task.child(0)`, so a dynamic
step is `StepInstance { stack, instruction, path, iterations }`, adding the
iteration ordinals `ForEach` pushes. Results at the root are keyed by
`(path, iterations)`: event ids, objects (prior, successor, controller before),
players and an optional amount; a missing entry means not performed, an empty
one ran and changed nothing. An ask (optional, pay life, object payment, pick)
stores chooser, selections and bounds; candidates are re-derived at offer and
answer when reproducible from state, else the bounded frozen domain is kept, and
only the pending `decision_id` is answerable. `StepProgress::Sequence` saves a
draw's players, read-once count, cursor and completed prefix.
`Reason::MyriadDefender` stays until Stage 3; a migrated task never installs a
rewritten leaf program.

`validate_kernel_task` checks the saved steps and projections. Recovery uses
`assert_recoverable` at quanta `1` and `u64::MAX` under Off, Compact, Stream and
Detailed:

1. One evaluator step is one rule unit: it evaluates, then commits at most one
   batch or installs one ask. A larger budget takes no fast path.
2. A value that can change between quanta is persisted at first read.
3. The result, its batch and the cursor advance in one `RulesTxn` stage, so a
   suspension after a committed draw cannot draw again; a replay witness covers
   the double draw separate writes would allow.
4. Restore authenticates the address, compatible stage, iteration membership and
   output dominance, requires every event id below `next_batch_id` and
   `done + remaining == total`, and rejects stale answers, forged or
   cross-invocation results and duplicate selections.
5. Retained payload is charged in every [trace mode](game-engine-mechanics.md#runtime-invariants).
   The state digest agrees across all four modes, the verification commitment
   across the three recording modes, and state and trace with replay within one.
6. Candidates order by public facts, then the viewer's handle, never a raw ID.
   A hidden-zone choice is always raised, with Decline. Bindings and results
   never reach an observation; a mid-instruction frame is never a search root.
   Worlds differing only in another seat's hidden cards stay indistinguishable
   across suspension and concession.

## Compositional lowering

Archive R5 supplies the source, parsed abilities and bound resolution graph.
`compose` lowers every printed ability to KIR or rejects the complete record;
missing references, unaudited operators and unmatched library procedures
reject explicitly. Normalized source lines must equal the parsed blocks: a
regrouping of the same lines (an ability-word "instead" rider kept with the
block it modifies) is unsupported at path `lines`; unaccounted text is invalid.

Lowerers read pronouns and targets by AST shape, so composition proves their
assumed edges. Every reference operand needs its resolution entry, including the
scalar player relations the archive always resolves (`control` you/opponent,
`owner`/`whose` your, `to` owner). A pronoun (`it`, `that`, `them`, `rest`) must
resolve to a target or step result, never an evaluated or derived entry. A step
result names the nearest earlier producer of its role (a search's selection or
objects, a look's objects) or, for `rest` only, the remainder of its own member
of a simultaneous step after an earlier sibling took from the pool. A printed
target's block-relative slot must be the one announced for its selection.
Evaluated-verb pronouns, `its controller` and follow-up steps acting on a target
bind through the [antecedent rule](#verb-signatures). Counters on each of
several targets lower to one `PutCounters` over `SetExpr::Targets`, a single
simultaneous placement on the counted occurrence's legal members (CR 608.2f,
603.2c).

A linked entry value must resolve to the matching printed producer on the same
face (CR 607.2d). The checker requires one compatible entry choice before
admitting a linked static scope or mana production. Produced mana binds its
spend permission to the chosen creature type and carries any spend rider as a
separate term. Choice and cost definitions here qualify at `Executes`.

`resolution::class_of` classifies the migrated operand references from the
generated AST positions; other references still use `resolution::reference_row`.
Rejected sites report `ref:<role>:<block kind>`. Steps dispatch on the AST
`effect` tag to `compiler/src/rules/verbs/`; operands lower from the generated
AST types in `operands.rs`, and a shape with no IR form rejects under
`verb:<effect>:<detail>` or `operand:<kind>:<shape>`. Composition also rejects
"may", "up to" and "if"/"unless" mismatched against `optional`, `upTo` and a
condition, a printed number absent from the AST, a cost term, an unprinted
ability word, and an evergreen keyword in only one of the keyword list and AST.

**Decoder boundary.** A migrated lowerer (the evaluated actions, exile,
return-to-hand and move-zone) reads only the generated `mtg-oracle-ast` types
(`scripts/generate-rust-ast.ts`) and decodes the whole containing block, keeping
omitted, null, unknown and malformed fields distinct. Rust accounts for every
operative field, qualifier and printed action on every required face before
accepting a card; an unprovable reading gets a located read, resolve,
elaborate, check or admission rejection. R6 printed-evidence spans are the
Stage 2 obligation.

Hand-built test rows carry no resolution: `tests::synthetic_compile` lowers
them through the block tables only (`cfg(test)`), and `tests::composed_near_miss`
asserts `composed` with the exact changed operands. Compiler examples:
`--example composition_gaps ARCHIVE` lists each remaining card's blockers
(card-wide binding failures, each block's first failure and other card-wide
failures; lowering still rejects at the first), and `catalog coverage` publishes
them as `blockers.tsv` for the [deck scoreboard](game-engine-development.md#measurement-and-deck-work);
`--example ast_decode ARCHIVE` decodes every face and resolution;
`--example inspect ARCHIVE NAME` prints one compile result. New lowerers start IR
structs from `PermanentFilter::any()`, `TargetSpec::new` and
`SpellProgram::instructions` (`contracts/src/builders.rs`).

Adding a primitive touches the row that owns it, not every card:

| Primitive | Files |
| --- | --- |
| Cost term, self event, keyword or step shape that lowers to existing IR | One `compose/*.rs` table row and its test; a frozen fixture when it opens a new runtime path |
| Existing instruction, target route or cost in another kind of stack object | One `contracts/src/kir/tiers/executes.rs` row, a witness in `witness/tests/permanents_mana/composition_tests.rs`, and the core fix if recovery assumed a shape |
| New instruction | A [kernel](#kernel-ir) cell in `kir/check/` with its capability and `kir::Cell` (tier row at `Executes`), its executor under `core/src/engine/kir/exec/`, the `rules/verbs/` lowerer and a witness |

Capabilities, schema, labels and source blocks come from the shared assembly,
never a per-card capability, schema branch or family validator.

## Match profile data

Strict JSON under `tests/fixtures/profiles/` is discovered at build time and
embedded without core filesystem I/O; new data needs no Rust or name selector.
Malformed/duplicate names, invalid completion indices and unsupported requests
fail the build, naming their input path.

| Field | Contract |
| --- | --- |
| `name` | Unique admission identity, never rule/strategy dispatch |
| `catalog_manifest` | Relative catalog manifest with expected digest/source checks |
| `decks` | Four defaults with commander/cards `{oracle_id,count>0}`; commander is within the 100 |
| `commanders` | Supported Oracle identities as data |
| `commander_policy` | `color_identity: commander_subset`, `eligibility: legendary_creature`, relative dated `banned_list`; unknown policies reject |
| `hosted`, `search` | Every definition must earn the `Hosting` or `Search` [tier](game-engine-mechanics.md#derived-capability-tiers); names/digests grant neither |
| `completion` | Deterministic `rotate` and two `homogeneous_seats` deck indices, not mechanics |

Admission checks 100 cards, singleton names, commander count/eligibility, color
identity and basic-land mana (CR 903.3–903.5). Serialized profile, catalog,
selected ban digest and normalized private configuration bind the pod. Hosting
also validates the whole pod and both executables; search validates integrity,
fresh root and its [information/reconstruction contract](game-engine-search.md).

All dated `commander-banned-*.json` files are embedded and each profile selects
one; their official URLs, retrieval date, HTML/archive hashes and named/offensive
exclusions are authoritative. Lutri's companion-only category stays distinct.
Pool review derives from authenticated source legality/type/text and the
selected list. A new dated list requires no Rust edit.

Optional compiler-owned `Definition.source_labels` is descriptive metadata
under [the permitted decision-context contract](game-client.md#core-action-discovery).
Removing it reproduces the prior mechanical IR without changing requirements or
reviewed Hosting/Search sets, though catalog/build identities change. Active
outputs are regenerated through the compiler, never hand-edited.
`Definition::requirements` is exhaustive over enum variants, not new struct
fields, so an IR field addition needs a review of capability derivation and
hosted/search sets before a catalog is admitted. Profiles never bypass shared
rules or capacities.

## Bulk headless admission

`mtg-sim catalog build-ir ARCHIVE --out DIR` publishes compact schema-1 IR, the
archive outcome index and their pinned artifact manifest. Bulk format 2 adds
source-derived Commander pairing and color facts. A consumer verifies the exact
producer, manifest pin, payload proofs and successful definition joins on load.

Dynamic decks contain 100 cards including one or two designated legendary
creatures. Two designations require ordinary Partner on both or Doctor's
companion with a Time Lord Doctor that has no other creature types. Pairing is
inspectable without compilation; admission also requires every definition to
earn `Executes`. The two color identities combine for deck legality and
commander-color mana, and each designation keeps its own command-zone cast and
combat damage history ([CR 702.124, 903.3–903.10](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).
Dynamic identities carry headless admission only, under one tier and
trigger-capacity bound with no roster exception.

`mtg-sim bench-pods --bulk-artifact DIR --manifest-sha SHA --pod-decks JSON`
reads an object with a `decks` array of four seat lists whose entries contain
`oracle_id`, `quantity` and `role` (`main` or `commander`). Workers share one
validated projection; the bulk manifest, policy, projection and per-case
admission digest identify the benchmark. Worker options, sampled replay checks
and `--analytics-out` apply to both input modes; analytics require every card to
earn `HeadlessQualified` and group decks by sorted commander designations and
physical quantities.

### Frozen-input compile

`sh scripts/compile-oracle.sh --inputs FROZEN_MANIFEST --out NEW_DIR` is the root
compile; `identity --inputs FROZEN_MANIFEST` verifies the cached inputs. Only the
manifest's pinned content affects output bytes. `MTG_COMPILE_CACHE` names the
reader cache (default this checkout's `generated/`) and `MTG_COMPILE_SIM` a
prebuilt `mtg-sim` (default a locked release build); neither enters identity.
Exit 1 names an `input`, `registration`, `reader`, `compiler` or `destination`
failure; exit 2 is usage.

The strict input manifest
([R5](../packages/game-engine/tests/fixtures/compile-inputs-phase1-r5.json),
[R6](../packages/game-engine/tests/fixtures/compile-inputs-phase1-r6.json))
pins Oracle and paper-evidence bytes, canonical records, scope, rules, reader,
archive length/full-file/header hashes and IR/bulk versions. Every pin is mandatory
except `compiler.buildId`, which must equal the binary when present; unknown keys
reject as unpinned inputs. Source paths resolve relative to the manifest and must
be regular files or hard links. Input schema 1 uses the retained schema-4 archive.
Input schema 2 requires a promoted schema-5 archive, its exact registered header
file, the printed schema, vocabulary, generator and record digests, and the
pre-scope name catalog digest and count; the R6 header also binds the R5 control
archive hashes for fresh generation. For R6, `--reader auto` (the default)
generates only when the cache entry (named by the archive basename) is absent;
a present entry must match its pins. `--reader cached` requires that entry, and
`--reader fresh` generates from the manifest's exact Oracle, paper evidence,
rules and R5 archive paths with the pinned TypeScript producer in a private
temporary directory. Both paths use the same Rust intake and full-file
verification. Fresh generation passes the pinned reader source and printed
generator digests to the writer, which exits before parsing when the checkout
differs, failing the compile as a `reader` error; after any grammar change the
registered cut is readable in cached mode only, like R5, until a new cut is
registered.

One stage writes the three bulk format-2 files byte-identical to `build-ir` from
the same binary, `outcomes.jsonl` and `compile-manifest.json`. Each outcome row
joins its index row by `oracle_id`, `source_index` and `archive_line` and adds
`phase` (`scope`, `read`, `lower`, `validate`) and `location` (an archive JSON
pointer for read failures, the compiler path for lowering). The compile manifest
binds the path-free logical inputs and their digest, producer, versions, counts
and the other four files' proofs. Each run writes and syncs the five files in a
uniquely named private `.bulk-stage-` sibling, verifies the proofs, and
publishes by one atomic no-replace rename and a parent directory sync. An
occupied destination succeeds only if it holds exactly those five regular files
with identical bytes. Errors after staging and identical retries report the
retained stage's path; [storage](game-engine-storage.md#abandoned-bundle-stages)
owns inspecting and removing abandoned stages.

## Single matches and policy inputs

`mtg-sim match-game PROFILE INDEX --journal-dir ABSOLUTE_DIR` plays one seeded
game and streams its live journal there; `match-replay PROFILE FILE` and
`match-replay-live PROFILE FILE` reject profile mismatches. [Deck work](game-engine-development.md#measurement-and-deck-work)
covers staging, coverage and paired benches; [policy boundaries](game-engine-mechanics.md#admission-and-policy-boundaries)
govern Develop. DrawOnly exists for draw/decking witnesses. Policy type filters
include all `additional_types`: positive type lists are alternatives, exclusions
reject any type, sacrifice subtypes are conjunctive, and land/creature roles keep
the contracts' canonical-kind invariant
([CR 205.2b](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).
