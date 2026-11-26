# Catalogs, profiles and build identity

Fixture paths below are relative to `packages/game-engine/`. Use frozen source
records, not refreshed card facts. [Development](game-engine-development.md#cargo-coordination-and-cache-integration)
owns the pinned environment and per-worktree Cargo lock.

## Catalog source and commands

The executable source is Archive R4. The full archive stays outside Git;
`tests/fixtures/phase1-r4/` holds only the extracted records a manifest, witness
or compiler test uses. `accepted-archives.json` pins the whole file, header,
parser and corpus, and each `provenance.json` row pins its exact card line.
Oracle parser tests own their pre-R4 records under
`packages/oracle-parser/test/fixtures/ramos/`.

From repository root (`engine:dev` builds `mtg-sim` under the target lock):

```sh
npm run engine:dev -- build -p mtg-sim
SIM=target/engine/debug/mtg-sim
$SIM catalog build packages/game-engine/tests/fixtures/catalogs/phase1-r4.json [--write]
$SIM catalog extract /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json 'Exact Card Name' oracle-id
$SIM catalog verify /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json
```

Filesystem and embedded loaders both lower every selected record through
composition. `build` checks the resulting schema-1 catalog and digest against the
committed outputs; `--write` regenerates both together, and the `mtg-compiler`
unit tests check every manifest without writing. The base, action and match
manifests supply the smoke, scenario and Foundations startup fixtures; phase1-r4
and witness-r4 exercise individual compiler and rules paths. A failed build
prints its typed `CatalogError` as JSON on stderr.

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
intact. For a fresh destination, copy
provenance metadata with `records: []` and place accepted-archive metadata beside
it. Archive identity is per record; top-level `archive_manifest_sha256` rejects.

`verify` compares every row for that archive against its actual line and exact
fixture bytes. Missing/duplicate/out-of-range lines, changed hashes or identity
mismatches reject; other origins are reported separately. Build inputs embed
manifests, provenance, accepted headers/pins and records, excluding expected IR,
policy copies and evidence. Missing scenario digests fail admission without
preventing diagnostic CLI builds.

## IR and build identity

`mtg_contracts::CURRENT_IR_VERSION` is the sole executable IR authority, fixed at
**1** until an external consumer needs compatibility. Evolve it in place with
strict capability/shape validation: no old decoder, version range, per-feature
schema gate or duplicate historical expected output. Archive, snapshot, bundle,
public and query formats have separate identities; [evidence](game-engine-evidence.md)
retains historical versions.

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
simulator, host or test-only edits therefore never rebuild core. Core `build.rs`
only embeds catalog/profile data; typed profile validation runs in
`match_profile_validation_tests`.

Compiler watches each catalog/provenance/archive/record input (including absent
repairable ones) plus the catalog directory; core watches the catalog/profile
directories and each selected data file, including ban policies. Broad watches
would rebuild for README or test edits. A new production module edits a watched
`mod` declaration; a new `build.rs` without a manifest edit is not detected
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
feature behavior still need review. Guard tests: `python3 scripts/test-engine-structure.py`
and `python3 scripts/test-engine-cache.py`.

## Kernel IR

`Instruction::Kernel { node: kir::Node, source_block: u32 }` is the sole
executable instruction in schema 1; the block index identifies the source of the
resolving stack object's instruction. Under `contracts/src/kir/`, `refs.rs`
defines the closed reference vocabulary, `PlayerExpr`/`SetExpr`/`Amount` build
operands from references and `pred.rs` atoms, `verbs.rs` defines one game action
per `Verb`, and `nodes.rs` composes them (`Do`, `Seq`, `If`, `Optional`,
`ForEach`, `Choose`, `ChooseObject`, `TapTargets`, `UnlessPays`, `IfPays`,
`Delayed`, `Reflexive`).

`kir::check(node, env)` is the one admission decision. It proves each reference
is in scope and of the right kind for the context, targets and event; rejects a
filtered count of a hand or library (CR 400.2); and admits a verb only in an
audited cell: a `kir/check/*.rs` row naming the `kir::Cell` it admits, which the
core interpreter (`core/src/engine/kir/exec/`) executes and recovers. A
well-scoped node without a cell is rejected as unaudited. Composition calls the
checker for each program and derives required capabilities from the node and its
operands; the JSON Schema describes wire structure only. The resolving frame can
resume at an audited control node, with bindings and progress tied to its
address in the admitted program. The audited cells and their operands are the
check modules; [mechanics](game-engine-mechanics.md) describes the supported
behavior of each family.

Control semantics:

- `Seq` follows written order. `If` supports `Compare`, `Exists`, `CostPaid`,
  `Not` and `All`. `ForEach` binds a player or a singleton object.
- `Optional`, `UnlessPays` and `IfPays` take a singleton player expression; both
  payment nodes support life and generic mana. With no live chooser the optional
  effect is skipped; with no live payer `UnlessPays` performs its node and
  `IfPays` does not (CR 118.12).
- `Choose` binds a pool from a matching public set when both bounds force zero
  or the set's size. `ChooseObject` offers one mandatory public object choice from
  a current controller-relative battlefield set; the saved frame derives
  candidates from current state and requires a matching offer when consuming an
  answer (CR 608.2d, 800.4a). `TapTargets` consumes one grouped optional
  announcement and changes its legal surviving targets simultaneously (CR 115.3,
  608.2b, 701.26).
- `Delayed` and `Reflexive` carry separately checked single-instruction programs;
  the parent can register a one-shot [resolution-created trigger](game-engine-mechanics.md#delayed-and-reflexive-triggers)
  at the end of its program.

Controls read fixed amounts, announced X and counts of audited sets; life
changes, payments and choice bounds require nonnegative amounts. Object sets are
matching battlefield permanents or graveyard cards under audited predicates,
source or announced identities, a bound singleton or a bound pool. Tapped,
combat and controller filters require the battlefield (CR 110.5d, 506.3–4,
109.4), including in predicate alternatives; graveyard sets exclude tokens
(CR 111.6). Player expressions name the controller, a bound player, the caster
or active event player, a player target in a prefix of ordinary player slots,
the recorded owner of the source or a singleton iteration object, or the
controller a filtered target's frame captured (CR 608.2h); loops and life leaves
can read each player or each opponent. Object stats and cast-history amounts
remain outside control cells. Branches gated by an optional-cost condition
cannot supply targets until cost-dependent target declarations are supported
(CR 601.2c).

Every branch and leaf needs its own audited cell. Non-life leaves use the flat
executor without task bindings, with a single announced target when they read
one. A nested draw draws exactly one fixed card for one player; counted draws
need a top-level draw instruction for their saved attempts. Token creation and
temporary effects need persistent leaf provenance, and set damage retains facts
tied to the top-level cursor. The drain pair and optional target destroy keep
standalone routes. Counter targets one announced stack entry and can read its
spell controller for a fixed generic payment. Regenerate records one source or
announced permanent. Control nodes derive capabilities from their leaves;
continuation-based nodes also require `KernelControlV1`, outside the reviewed
hosting and search sets. A procedure without an audited node and execution path
rejects during lowering; a new mechanic adds a typed operand, a capability and a
tested rules operation wherever the algebra cannot already express it.

## Compositional lowering

Archive R4 supplies the source, parsed abilities and bound resolution graph.
`compose` lowers every printed ability to KIR or rejects the complete record;
missing references, unaudited operators and unsupported procedures such as
look-at and reveal reject explicitly. Normalized source lines must equal the
parsed blocks: a different grouping of the same lines (an ability-word "instead"
rider kept with the block it modifies) is unsupported at path `lines`, and
unaccounted text is invalid.

Lowerers read pronouns and targets by AST shape, so composition proves the edges
they assume. Every reference operand needs its resolution entry, including the
scalar player relations the archive always resolves (`control` you/opponent,
`owner`/`whose` your, `to` owner). A pronoun (`it`, `that`, `them`, `rest`) must
resolve to a target or step result, never an evaluated or derived entry. A step
result names the nearest earlier producer of its role (a search's selection or
objects, a look's objects) or, for `rest` only, the remainder of its own member
of a simultaneous step after an earlier sibling took from the pool. A printed
target's block-relative slot must be the one announced for its selection, and a
pronoun resolved to a target must name the block's sole announced target.

Each resolved reference is looked up by role and print site in
`resolution::reference_row`: a row returns its lowering or the card rejects
under `ref:<role>:<block kind>`. Steps dispatch on the AST `effect` tag to
`compiler/src/rules/verbs/`; operands lower from the generated AST types in
`operands.rs`, and a shape with no IR form rejects under `verb:<effect>:<detail>`
or `operand:<kind>:<shape>`. Composition also rejects "may", "up to" and
"if"/"unless" mismatched against `optional`, `upTo` and a condition; a printed
number absent from the AST; a cost term; an unprinted ability word; and an
evergreen keyword present in only one of the keyword list and the AST.

Hand-built test rows cannot carry a resolution: `tests::synthetic_compile` lowers
them through the block tables only (`cfg(test)`), and `tests::composed_near_miss`
asserts `composed` with the exact changed operands. Compiler examples:
`--example composition_gaps ARCHIVE` lists every blocker of each remaining card
(card-wide binding failures, the first failure of each block and other card-wide
failures; lowering still rejects at the first), and `catalog coverage` publishes
the same rows as `blockers.tsv` for the [deck scoreboard](game-engine-development.md#measurement-and-deck-work);
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

Capabilities, schema, labels and source blocks come from the shared assembly; no
per-card capability, schema branch or family validator is added.

## Match profile data

Strict JSON under `tests/fixtures/profiles/` is discovered at build time and
embedded without core filesystem I/O; adding data needs no Rust or name selector.
Malformed/duplicate names, invalid completion indices and unsupported requests
fail the build with their input path.

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
following [the permitted decision-context contract](game-client.md#core-action-discovery).
Removing it reproduces the prior mechanical IR without expanding requirements or
reviewed Hosting/Search sets, but catalog/build identities still change.
Active outputs are regenerated through the compiler, never hand-edited.

`Definition::requirements` is exhaustive over enum variants, not new struct
fields: every IR field addition requires reviewing capability derivation and
hosted/search sets before admitting a catalog. Profiles never bypass shared rules
or capacities.

## Bulk headless admission

`mtg-sim catalog build-ir ARCHIVE --out DIR` publishes compact schema-1 IR, the
archive outcome index and their pinned artifact manifest. Bulk format 2 adds
source-derived Commander pairing and color facts. A consumer verifies the exact
producer, manifest pin, payload proofs and successful definition joins on load.

Dynamic decks contain 100 cards including one or two designated legendary
creatures. Two designations require ordinary Partner on both or Doctor's
companion with a Time Lord Doctor that has no other creature types. Pairing can
be inspected without compilation; admission also requires every definition to
earn `Executes`. The two color identities combine for deck legality and
commander-color mana, and each designation keeps its own command-zone cast and
combat damage history ([CR 702.124, 903.3–903.10](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).
Dynamic identities carry headless admission only; every deck passes the same
tier and trigger-capacity bound with no roster exception.

`mtg-sim bench-pods --bulk-artifact DIR --manifest-sha SHA --pod-decks JSON`
reads an object with a `decks` array of four seat lists whose entries contain
`oracle_id`, `quantity` and `role` (`main` or `commander`). Workers share one
validated projection; the bulk manifest, policy, projection and per-case
admission digest identify the benchmark. Worker options, sampled replay checks
and `--analytics-out` apply to both input modes; analytics require every deck
card to earn `HeadlessQualified` and group decks by sorted commander designations
and physical quantities.

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
