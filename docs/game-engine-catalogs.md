# Catalogs, profiles and build identity

Fixture paths below are relative to `packages/game-engine/`. Use frozen source
records, not refreshed card facts. [Development](game-engine-development.md#cargo-coordination-and-cache-integration)
owns the pinned environment and per-worktree Cargo lock.

## Catalog source and commands

The executable source is Archive R5. The full archive stays outside Git;
`tests/fixtures/phase1-r5/` holds only the extracted records a manifest, witness
or compiler test uses. `accepted-archives.json` pins the whole file, header,
parser and corpus, and each `provenance.json` row pins its exact card line.
R4 (same corpus, older parser) stays registered without record pins so its
bulk IR can be rebuilt for comparison. The header records the generating Git
revision, so only a byte-identical regenerated archive is accepted. Oracle parser tests own their pre-R4 records under
`packages/oracle-parser/test/fixtures/ramos/`.

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

`catalogs/*.json` are manifests (expected IR lives outside that directory) with
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

`extract` requires each exact case-sensitive name/Oracle selector to resolve
once. It hashes the whole archive before writing, preserves raw lines and
terminators, uses Oracle-ID filenames, extends record pins and appends
line/hash provenance; conflicting selections fail before writes. Unparsed
records keep null AST hashes and cannot compile; excluded envelopes stay
intact. A fresh destination needs provenance metadata with `records: []` and
accepted-archive metadata beside it. Archive identity is per record; top-level
`archive_manifest_sha256` rejects.

`verify` compares every row for that archive with its actual line and exact
fixture bytes; missing/duplicate/out-of-range lines, changed hashes or identity
mismatches reject, and other origins are reported separately. Build inputs embed
manifests, provenance, accepted headers/pins and records, excluding expected IR,
policy copies and evidence. Missing scenario digests fail admission without
preventing diagnostic CLI builds.

## IR and build identity

`mtg_contracts::CURRENT_IR_VERSION` is the sole executable IR authority, fixed at
**1** until an external consumer needs compatibility. It evolves in place under
strict capability/shape validation, with no old decoder, version range,
per-feature schema gate or duplicate historical expected output. Archive,
snapshot, bundle, public and query formats have separate identities;
[evidence](game-engine-evidence.md) retains historical versions.

The engine build identity hashes production engine Rust sources and Cargo
manifests (`mtg_contracts::engine_build_input`), root Cargo manifest/lock,
toolchain and compiler/target/settings. Test-only sources (`tests/`, `examples/`,
`benches/`, `tests.rs`, `*_tests.rs`, `*_tests/`, the `testkit` module and the
dev-only `crates/witness` package), fixtures, generated output and server source
are excluded; catalog/pod identity and `SERVER_BUILD_ID` bind those separately.
Data-only catalog additions do not invalidate unchanged-catalog snapshots.

`mtg-sim` and `mtg-server` compute the identity in `build.rs` through
`crates/build_identity.rs`, watching each identity file individually; `main`
calls `mtg_core::install_build_identity` before any engine use. Core never reads
build-time environment: snapshots, saved work and search artifacts stamp and admit
`mtg_core::build_id()`. A process that installs nothing (tests, examples) uses
`BuildIdentity::TEST`, which never matches a native artifact. Compiler,
simulator, host or test-only edits never rebuild core. Core `build.rs`
only embeds catalog/profile data; typed profile validation runs in
`match_profile_validation_tests`.

Compiler watches each catalog/provenance/archive/record input (including absent
repairable ones) plus the catalog directory; core watches the catalog/profile
directories and each selected data file, including ban policies. Watches stay
narrow so README or test edits do not rebuild. A new production module edits a
watched `mod` declaration; a new `build.rs` without a manifest edit is not detected
([Cargo rerun-if-changed](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)).

`crates/core/src/formats.rs` owns the core wire/persistence versions
(`SNAPSHOT_VERSION`, `OBSERVATION_VERSION`, `WORK_VERSION`, search envelopes).
`formats_tests` pins the Observation schema and Snapshot/SavedWork shape
fingerprints, with samples that include resolving and nested Kernel-IR choices.
Tests import the constants; deliberate historical forgeries use literals.

`python3 scripts/engine-structure-check.py` verifies declared direct dependency
boundaries (target/workspace/alias tables and dev-dependencies; core has no
compiler edge), that test-named modules are `#[cfg(test)]` and `testkit` modules
`#[cfg(feature = "testkit")]` with only test crates enabling it or depending on
mtg-witness, and that the Git index holds no known generated paths or
ELF/Wasm/SQLite output. It never alters the index; renamed JSON and transitive
features still need review. Guard tests: `python3 scripts/test-engine-structure.py`
and `python3 scripts/test-engine-cache.py`.

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
mechanics describe each family's behavior.

Control semantics:

- `Seq` follows written order. `If` supports `Compare`, `Exists`, `CostPaid`,
  `Not` and `All`. `ForEach` binds a player or a singleton object.
- `Optional`, `UnlessPays` and `IfPays` take a singleton player expression; both
  payment nodes support life and generic mana. With no live chooser the optional
  effect is skipped; with no live payer only `UnlessPays` performs its node
  (CR 118.12).
- `Choose` binds a pool from a matching public set when both bounds force zero
  or the set's size. `ChooseObject` offers one mandatory public object choice from
  a current controller-relative battlefield set; the saved frame derives
  candidates from current state and consumes an answer only against a matching
  offer (CR 608.2d, 800.4a). `TapTargets` and `RemoveTargets` consume one
  counted battlefield target group and tap/untap, destroy, exile or bounce its
  legal surviving members in one simultaneous event (CR 115.3, 608.2b, 701.26).
- `SacrificeForTappedReturn` binds the declared graveyard group to a mandatory
  creature sacrifice and returns its legal survivors together if it succeeds.
  `Satisfies` can test the printed creature type of a bound graveyard card
  after exile (CR 608.2b, 608.2h).
- `Delayed` and `Reflexive` carry separately checked single-instruction programs;
  the parent can register a one-shot [resolution-created trigger](game-engine-board-mechanics.md#delayed-and-reflexive-triggers)
  at its end.

Controls read fixed amounts, announced X and audited set counts; verbs also read
[resolution-time counts](game-engine-board-mechanics.md#resolution-time-counts). Life
changes, payments and choice bounds need nonnegative amounts. Object sets are
matching battlefield permanents or graveyard cards under audited predicates,
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

Every branch and leaf must pass the checker. Draw and life gain or loss use the
signature evaluator in the resolving task, including under controls. Other
ordinary leaves use the flat executor without task bindings, reading at most
one announced target. Token creation and temporary effects need persistent leaf
provenance, and set damage retains facts tied to the top-level cursor. Optional
target destruction keeps its standalone route. Counter targets one announced stack entry and can read its
spell controller for a fixed generic payment. Regenerate records one source or
announced permanent. Control nodes derive capabilities from their leaves;
continuation-based nodes also require `KernelControlV1`, outside the reviewed
hosting and search sets. A procedure without an audited node and execution path
rejects during lowering; a new mechanic adds a typed operand, capability and
tested rules operation only where the algebra cannot express it.

### Verb signatures

Stage 1 [rework](game-engine-rework.md) contract. Draw, life, damage, permanent
counter placement and sacrifice use `kir::signatures`. `kir::Verb` remains
the action representation on the wire. Damage prevention and commit, counter
placement and graveyard moves retain their transaction authorities.

A slot admits an operand whose class it accepts and that resolves in scope; no
row names a verb and reference pair. Scope is the checker's `Env`
(context, announced targets, triggering event, X and the checked cost); lexical bindings live in checker
scope, not `Env`. Sorts must match. One verb-independent classifier maps a
resolved reference and its typed AST site to a class. Required capabilities are
the union over the verb, its operand classes, distribution and projections.

Each slot declares:

- `field`, `role`: the verb field and its semantic role (subject, recipient, amount, source).
- `sort`, `cardinality`: player, object, set or quantity; singleton, bounded or unbounded.
- `quantity_policy`: nonnegative, positive or signed, and the verb's zero rule.
- `zone`: where an object operand must be when read or acted on.
- Read time and object use: announcement, resolution start, step start, event
  before or after; act on the live incarnation, read the event snapshot, or read
  current-or-last-known.
- `absence_behavior`: skip the member, skip the verb, or reject (CR 608.2b).
- `accepted_constructors`: the operand classes the slot accepts.
- `capability_requirements`: capabilities the slot adds when used.

Last-known information answers a read and never authorizes acting on a new
incarnation (CR 400.7, 608.2h).

A triggered invocation stores an `EventBinding` for the event's player, object
and quantity when supplied. Death reads the common prior battlefield. An
entrant or attacker whose power is read at resolution uses the exact live
incarnation or its departure facts. A later incarnation cannot receive an
action for the original one.

**Distribution** is admitted per verb, separately from operand acceptance:
accepting a player set does not license a simultaneous or divided write. One
member or members in turn: `Single`, `EachSequential`. One event for every
member: `EachSimultaneous`, or `CollectThenSimultaneous` after APNAP choice
collection (CR 101.4). Divided: `DividedAtAnnouncement`, rejected in Stage 1.

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

A second projection of one verb ("life lost this way" from damage, "died this
way") rejects until a `StepRole` is added additively. `Ref::StepResult { step, role }`
keeps its wire form; the checker resolves it to one producer among earlier
siblings of its `Seq`, rejects ambiguity, and requires that step to hold exactly
one `Do` leaf, directly or under `Optional`, `If` or `IfPays`. Nested-body reads
wait for Stage 2. **Absent is not zero.** A producer that did not run yields
`Resolved::Absent` with its reason (illegal target, not chosen, departed player,
no event member or not performed); one that ran and affected nothing yields an
empty result.

A `CostResult` for an activated ability is scoped to a mandatory cost that
sacrifices one selected permanent. It exposes the paid object's last-known
facts, the payer and the number paid. A sacrifice action exposes the actual
number, objects and players through the ordinary step collector.

**Tier preservation.** The 17 previously qualified higher-tier shapes in
`kir::tiers` keep their legacy cells and tiers; every other evaluator-admitted
node is `Cell::Evaluated(verb)` at `Executes`.

### Resolving frame

The `Task` chain keeps lexical paths and scoped bindings in `Frame::Kernel`.
`Task.resume` links to the next scheduled task or to the collector at
`frames[1]`.

- **Step identity.** `ForEach` runs every iteration at `task.child(0)`, so a path
  alone aliases iterations. A dynamic step is
  `StepInstance { stack, instruction, path, iterations }`, adding the iteration
  ordinals `ForEach` pushes.
- **Results at the root** are keyed by `(path, iterations)`: event ids,
  objects (prior, successor, controller before), players and an optional amount.
  A missing entry means not performed; an empty one, ran and changed nothing.
- **Asks and progress.** An ask (optional, pay life, object payment, pick)
  stores chooser, selections and bounds. Candidates are re-derived at offer and
  answer when reproducible from state, else the bounded frozen domain is kept;
  only the pending `decision_id` is answerable. `StepProgress::Sequence` saves
  a draw's players, read-once count, cursor and completed prefix. Part 2 will
  replace `Frame::ForcedSacrifice` with APNAP collection, and E2 adds proposals. `MyriadDefender` stays until
  Stage 3; a migrated task never installs a rewritten leaf program.

`validate_kernel_task` checks the saved steps and projections. Recovery uses `assert_recoverable`
at quanta `1` and `u64::MAX` under Off, Compact, Stream and Detailed:

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
target's block-relative slot must be the one announced for its selection; a
pronoun resolved to a target must name the block's sole announced target.

`resolution::class_of` classifies the migrated operand references from the
generated AST positions. Other references still use `resolution::reference_row`.
Rejected sites report `ref:<role>:<block kind>`. Steps dispatch on the AST `effect` tag to
`compiler/src/rules/verbs/`; operands lower from the generated AST types in
`operands.rs`, and a shape with no IR form rejects under `verb:<effect>:<detail>`
or `operand:<kind>:<shape>`. Composition also rejects "may", "up to" and
"if"/"unless" mismatched against `optional`, `upTo` and a condition, a printed
number absent from the AST, a cost term, an unprinted ability word, and an
evergreen keyword in only one of the keyword list and the AST.

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

**Decoder boundary (Stage 1, unchanged R5).** A migrated lowerer reads only the
generated `mtg-oracle-ast` types (`scripts/generate-rust-ast.ts`) and decodes
the whole containing block, keeping omitted, null, unknown and malformed fields
distinct. Rust accounts for every operative field, qualifier and printed action
on every required face before accepting a card. An unprovable
reading gets a located read, resolve, elaborate, check or admission rejection.
R6 printed-evidence spans are the Stage 2 obligation.

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
compile; `identity --inputs FROZEN_MANIFEST` verifies without compiling. Only the
manifest's pinned content affects output bytes. `MTG_COMPILE_CACHE` names the
reader cache (default this checkout's `generated/`) and `MTG_COMPILE_SIM` a
prebuilt `mtg-sim` (default a locked release build); neither enters identity. Exit
1 names an `input`, `registration`, `reader`, `compiler` or `destination` failure;
exit 2 is usage.

The strict manifest ([R5](../packages/game-engine/tests/fixtures/compile-inputs-phase1-r5.json))
pins Oracle and paper-evidence bytes, canonical records, scope, rules, reader,
archive length/full-file/header hashes and IR/bulk versions. Every pin is mandatory
except `compiler.buildId`, which must equal the binary when present. Unknown keys
reject as unpinned inputs. Source paths resolve relative to the manifest and must
be regular files (hard links, never symlinks). The archive header must be
[registered](#ir-and-build-identity) with these pins. Archive schema 4 (R5) has
only the cached path: the retained archive is verified, never regenerated.

One stage writes the three bulk format-2 files byte-identical to `build-ir` from the
same binary, `outcomes.jsonl` and `compile-manifest.json`. Each outcome row joins
its index row by `oracle_id`, `source_index` and `archive_line` and adds `phase`
(`scope`, `read`, `lower`, `validate`) and `location` (an archive JSON pointer for
read failures, the compiler path for lowering). The compile manifest binds the
path-free logical inputs and their digest, producer, versions, counts and the four
other files' proofs. Publication re-verifies staged files, renames once and syncs
the parent; an occupied destination succeeds only when it already holds exactly
this bundle.

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
