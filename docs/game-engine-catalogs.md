# Catalogs, profiles and build identity

Fixture paths below are relative to `packages/game-engine/`. Use frozen source
records, not refreshed card facts. [Development](game-engine-development.md#cargo-coordination-and-cache-integration)
owns the pinned environment/per-worktree Cargo lock for native commands.

## Catalog source and commands

The executable source is Archive R4. The accepted-archive registry pins the whole
file, header, parser and corpus. Each provenance row pins its exact card line.
The full archive stays outside Git. The committed extracts live in
`tests/fixtures/phase1-r4/`.

From repository root (`engine:dev` builds `mtg-sim` under the target lock):

```sh
npm run engine:dev -- build -p mtg-sim
SIM=target/engine/debug/mtg-sim
$SIM catalog build packages/game-engine/tests/fixtures/catalogs/phase1-r4.json [--write]
$SIM catalog extract /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json 'Exact Card Name' oracle-id
$SIM catalog verify /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json
```

Both filesystem and embedded catalog loaders lower every selected record through
composition. A build checks the resulting schema-1 catalog and its digest against
the committed outputs; the `mtg-compiler` unit tests run that check for all five
manifests. `--write` regenerates those outputs. The base, action
and match manifests supply the smoke, scenario and Foundations startup fixtures.
The phase1-r4 and witness-r4 manifests exercise individual compiler and rules
paths. Oracle parser tests own their ten pre-R4 records under
`packages/oracle-parser/test/fixtures/ramos/`.

A failed build prints its typed `CatalogError` as JSON on stderr. Coverage publishes both receipts together and preserves occupied
destinations unless their receipts already match. Manifest fields are `schema_version: 1`,
`provenance`, `records`, `expected_ir`, `expected_digest` and `output_order`:
`canonical` (sorted compact JSON), `sorted` (sorted pretty), or `struct` (typed
field-order pretty). Manifest paths are relative to that manifest; source/archive
paths are relative to provenance. Digest files contain SHA-256 without a newline.
`catalogs/*.json` contains manifests; expected IR lives outside that directory.

Nonland records with empty printed mana costs reject: absent cost is unpayable,
distinct from `{0}` (CR 118.6/118.6a), and the IR cannot yet represent it. Lands
may have empty cost; explicit `{0}` remains supported. Missing/null/malformed
mana-cost fields reject. Optional AST cast/attacker/recursive damage-source
class/type/Boolean filters and attacker-control relations reject explicit null;
omission and valid nonempty filters retain their meanings.

`extract` requires each exact case-sensitive name/Oracle selector to resolve once.
It hashes the whole archive before writing, preserves raw lines/terminators, uses
Oracle-ID filenames, extends record pins and appends line/hash provenance.
Conflicting selections fail before writes. Unparsed records retain null AST hashes
and cannot compile; excluded envelopes remain intact. For a fresh destination,
copy provenance metadata with `records: []` and place accepted-archive metadata
beside it. Archive identity belongs per record; obsolete top-level
`archive_manifest_sha256` rejects.

`verify` compares every row for that archive against its actual line and exact
fixture bytes. Missing/duplicate/out-of-range lines, changed hashes or identity
mismatches reject; other origins are reported separately. Build inputs embed
manifests, provenance, accepted headers/pins and records, excluding expected IR,
policy copies and evidence. Malformed catalog/group data yields `CatalogError`;
missing scenario digests fail admission without preventing diagnostic CLI builds.
Catalog manifests define record membership and order.

## IR and build identity

`mtg_contracts::CURRENT_IR_VERSION` is the sole executable IR authority, fixed at
**1** until an external consumer needs compatibility. Evolve it in place with
strict capability/shape validation; no old decoder, minimum-version range,
per-feature schema gate or duplicate historical expected output. Regenerate active
IR/digests through the compiler. Archive/snapshot/bundle/public/query formats have
separate identities; source-qualified [evidence](game-engine-evidence.md) retains
historical versions and the schema-1 reset proof.

<a id="engine-code-identity"></a>

The engine build identity hashes production engine Rust sources/Cargo
manifests (`mtg_contracts::engine_build_input`), root Cargo manifest/lock,
toolchain and compiler/target/settings. Test-only sources (`tests/`,
`examples/`, `benches/`, `tests.rs`, `*_tests.rs`, `*_tests/`, the `testkit`
harness module and the dev-only `crates/witness` package), catalog/profile
fixtures, generated output and server source are excluded; catalog/pod identity
and `SERVER_BUILD_ID` bind those separately. Retained manifests still pin all
source/data/server bytes. Data-only catalog additions do not invalidate
unchanged-catalog snapshots.

The identity lives at the executable edge. `mtg-sim` and `mtg-server` compute it
in `build.rs` through `crates/build_identity.rs`, watching each identity file
individually, and `main` calls `mtg_core::install_build_identity` before any
engine use. mtg-core never reads build-time environment: snapshots, saved work
and search artifacts stamp and admit `mtg_core::build_id()`. A process that
installs nothing (tests, examples) is fixed to `BuildIdentity::TEST`, which is
not a SHA-256 and never matches a native artifact. Compiler, simulator, host or
test-only edits therefore never rebuild mtg-core. Core `build.rs` only embeds
catalog/profile data, so mtg-contracts is not a build dependency; typed profile
validation runs in `match_profile_validation_tests`.

Compiler watches each catalog/provenance/archive/record input, including absent
repairable inputs, plus the catalog directory. Core watches catalog/profile
directories and each selected data file, including discovered ban policy paths.
Broad fixture-root or source-directory watches would rebuild for README or test
edits. A new production module edits a watched `mod` declaration; a new
`build.rs` without a manifest edit is not detected. See
[Cargo build-script watches](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed).

Core wire/persistence versions (`SNAPSHOT_VERSION`, `OBSERVATION_VERSION`,
`WORK_VERSION` and the search envelopes) have one owner,
`crates/core/src/formats.rs`. `formats_tests` pins the Observation schema and
Snapshot/SavedWork shape fingerprints. Snapshot samples include a resolving
spell choice and a nested Kernel-IR control choice with saved continuations.
Tests import the constants. Deliberate historical forgeries use literal versions.

`python3 scripts/engine-structure-check.py` verifies declared direct dependency
boundaries (including target/workspace/alias tables and dev-dependencies: core
has no compiler edge at all), that test-named modules are `#[cfg(test)]` and
`testkit` modules `#[cfg(feature = "testkit")]` with only test crates enabling
that feature or depending on mtg-witness (so excluding them from build identity
is sound), and the Git index for known
generated paths and recognizable ELF/Wasm/SQLite output. It never alters the index; arbitrary renamed JSON or
transitive feature behavior still requires review. Focused guards are
`python3 scripts/test-engine-structure.py` and `python3 scripts/test-engine-cache.py`.

## Kernel IR

`Instruction::Kernel { node: kir::Node, source_block: u32 }` is the sole
executable instruction in schema 1. The block index identifies the source
of an instruction for the resolving stack object.

| Layer | Contents |
| --- | --- |
| `Ref` | The closed reference vocabulary: source, controller (with its role), target slot, attached-to, event object/player, entering object, step result, bound variable, linked pool, announced X |
| `PlayerExpr`, `SetExpr`, `Amount` | Players, object sets (`Matching { zone, filter: Pred }`) and quantities built from references and predicate atoms |
| `Verb` | One game action over those operands |
| `Node` | `Do`, `Seq`, `If`, `Optional`, `ForEach`, `Choose`, `UnlessPays`, `Delayed`, `Reflexive` |

`kir::check(node, env)` is the one admission decision. It proves each reference
is in scope and of the right kind for the context, targets and event; rejects a
filtered count of a hand or library (CR 400.2); and admits a verb only in an
audited cell (`kir/check/<verb>.rs`, which names the `kir::Cell` it admits), one
the core interpreter (`core/src/engine/kir/exec/<verb>.rs` or the executor it
dispatches to) executes and recovers. Composition calls the checker for each program and derives the required capabilities
from the node and its operands. The published JSON Schema describes the wire
structure. Rust checks target bindings, operand scope and capability closure.
A well-scoped node without a cell is rejected as unaudited. The resolving
frame can resume at an audited control node, with bindings and progress tied
to its address in the admitted program.

Every flat effect is a kernel verb; the audited cells are:

| Verb | Audited operands |
| --- | --- |
| `gain_life`, `lose_life` | Controller (fixed or X), each opponent, one announced player (fixed or X), or the caster/active event player; the standalone drain pair uses a dedicated executor |
| `draw` | Controller, every player, one announced player (fixed or X), or the caster/active event player |
| `damage` | From the source: one announced target (fixed or X for any target or creature), the controller, a mana ability's activator, or creature sets plus optionally each player (fixed, X, creature count or other casts' mana value) |
| `destroy`, `move`, `tap`, `untap` | One announced target by domain (destroy; exile, owner's hand, top of owner's library; tap; untap), an optional target destroy, matching battlefield sets, your lands (untap), one player's graveyard or every graveyard (exile) |
| `mill`, `discard`, `sacrifice` | Controller or one announced player (mill; X for the player); controller, damaged player, announced player (random or X), each player or each opponent (discard); damaged player, target opponent, each player or each opponent sacrifices one creature |
| `create_tokens`, `put_counters` | Positive fixed tokens you create (predefined or creature recipe); positive fixed +1/+1 counters on the source or one announced creature |
| `modify_until` | End-of-turn fixed P/T change of the source, one announced creature or a matching battlefield set; the source's own keyword grant |

The interpreter routes admitted leaves through the shared rules operations.
Board-wide sets evaluate through the battlefield and graveyard queries.

`Seq` follows written order. `If` supports `Compare`, `Exists`, `CostPaid`,
`Not` and `All`. `ForEach` binds a player or a singleton object.
`Optional` and `UnlessPays` take a singleton player expression, and
`UnlessPays` supports life and generic mana. The continuation skips an optional
effect with no live chooser and takes the unpaid branch with no live payer.
`Choose` binds a pool from a matching public set when both bounds force zero
or the size of that set. Interactive object choice remains unaudited because
the saved candidates and chooser are not refreshed on departure (CR 800.4a/g).

Controls read fixed amounts, announced X and counts of audited sets. Life
changes, payments and choice bounds require nonnegative amounts. Object sets
are matching battlefield permanents or cards across graveyards with the
audited set predicates, single source or announced object identities, a bound
singleton, or a bound choice pool. Tapped and untapped status, combat
participation and controller filters require the battlefield in these sets
(CR 110.5d, 506.3–4, 109.4), including in predicate alternatives. Graveyard
card sets exclude tokens (CR 111.6). Player expressions name the invocation's
controller, a bound player, the caster or active event player, or a player
target in a prefix of ordinary player slots. They can also read the recorded
owner of the source or a singleton iteration object. Player loops and life
leaves can read each player or each opponent. Target owners, object controllers
and life totals can require missing or last-known facts the continuation cannot
always read. Object stats and cast-history amounts remain outside the control
cells.

Every branch and leaf needs its own audited cell. Non-life leaves use the flat
executor without task bindings, with a single announced target when they read a
target. A nested draw must draw exactly one fixed card for one player. Counted
draws, including each-player and announced-X draws, require a top-level draw
instruction for their saved attempts and outcome. Token creation and
temporary effects need persistent leaf provenance, and set damage retains
facts tied to the top-level cursor. The specialized drain and optional target
destroy retain their standalone routes. `Delayed` and `Reflexive` remain
unaudited. Control nodes derive capabilities from their leaves, including
target- and program-sensitive requirements. Continuation-based nodes also
require `KernelControlV1`, which is outside the reviewed hosting and search
sets. Composition and profile admission check each complete definition.

A procedure without an audited node and execution path rejects during lowering.
Adding a mechanic requires a typed operand, a capability and a tested rules
operation wherever the current algebra cannot already express it.

## Compositional lowering

Archive R4 supplies the source, parsed abilities and bound resolution graph.
`compose` lowers every printed ability or rejects the complete record. Every
executable program contains KIR nodes. Missing references, unaudited operators,
and unsupported complete procedures such as scry and surveil reject explicitly.

The normalized source lines must equal the parsed blocks. When the parser
groups the same printed lines differently (an ability-word "instead" rider kept
with the block it modifies) the row is unsupported at path `lines`; text the
parse does not account for is invalid.

Lowerers read pronouns and targets by AST shape, so composition also proves
the edges they assume. Every reference operand must have its resolution entry,
including the scalar player relations the archive always resolves (`control`
you/opponent, `owner`/`whose` your, `to` owner). A pronoun (`it`, `that`,
`them`, `rest`) must resolve to a target or a step result, never an evaluated
or derived entry. A step result must name the nearest earlier producer of its
role (a search's selection, a look's objects), or, for `rest` only, the
remainder of its own member of a simultaneous step after an earlier sibling
took from the pool; any other role rejects. A printed target's resolved
block-relative slot must be the slot announced for its selection, and a
pronoun resolved to a target must name the block's sole announced target.

Each resolved reference is looked up in one table keyed by its role and where
it is printed (`resolution::reference_row`): a row returns its lowering or the
card rejects under `ref:<role>:<block kind>`, so a wave opens a role in its row.
Steps dispatch on their AST `effect` tag to `compiler/src/rules/verbs/<verb>.rs`;
operands lower from the generated AST types in `operands.rs` (target, filter,
set, amount, player, destination, duration), and a shape with no IR form rejects
under `verb:<effect>:<detail>` or `operand:<kind>:<shape>`.

Composition also rejects "may", "up to" and "if"/"unless" against `optional`,
`upTo` and a condition; a printed number absent from the block's AST; a cost
term; an ability word the block does not print; and an evergreen keyword present
in only one of the source keyword list and the AST. Hand-built test rows with
placeholder text cannot carry a resolution: `tests::synthetic_compile` lowers them
through the block tables only (`cfg(test)`), and `tests::composed_near_miss`
asserts `composed` with the exact changed operands. `--example composition_gaps ARCHIVE` lists
every blocker of each remaining card: each card-wide binding failure, the first
failure of each printed block and any card-wide failure outside the blocks.
Lowering still rejects at the first; `catalog coverage` publishes the same rows
as `blockers.tsv`, which the [deck scoreboard](game-engine-development.md#measurement-and-deck-work) ranks.
`--example ast_decode ARCHIVE` decodes every face and resolution through the generated types.
`--example inspect ARCHIVE NAME` prints one compile result. New lowerers start IR
structs from `PermanentFilter::any()`, `TargetSpec::new` and `SpellProgram::instructions`
(`contracts/src/builders.rs`), so a new field touches one constructor.

Adding a primitive touches the row that owns it, not every card:

| Primitive | Files |
| --- | --- |
| Cost term, self event, keyword or step shape that lowers to existing IR | One `compose/*.rs` table row and its test; a frozen fixture when it opens a new runtime path |
| Existing instruction, target route or cost in another kind of stack object | One `contracts/src/kir/tiers/executes.rs` row, a witness in `witness/tests/permanents_mana/composition_tests.rs`, and the core fix if recovery assumed a shape |
| New instruction | A [kernel](#kernel-ir) cell in `kir/check/<verb>.rs` with its capability and `kir::Cell` (tier row at `Executes`), the executor in `core/src/engine/kir/exec/<verb>.rs`, the `rules/verbs/<verb>.rs` lowerer and a witness. |

Capabilities, schema, labels and source blocks come from the shared assembly;
no per-card capability, schema branch or family validator is added.

## Match profile data

Strict JSON under `tests/fixtures/profiles/` is discovered at build time and
embedded without core filesystem I/O. Adding data needs no Rust/name selector.
Malformed/duplicate names, invalid completion indices and unsupported requests
fail the build with their input path.

| Field | Contract |
| --- | --- |
| `name` | Unique admission identity, never rule/strategy dispatch |
| `catalog_manifest` | Relative catalog manifest with expected digest/source checks |
| `decks` | Four defaults with commander/cards `{oracle_id,count>0}`; commander is within the 100 |
| `commanders` | Supported Oracle identities as data |
| `commander_policy` | `color_identity: commander_subset`, `eligibility: legendary_creature`, relative dated `banned_list`; unknown policies reject |
| `hosted`, `search` | Every definition must earn the `Hosting` or `Search` [tier](game-engine-mechanics.md#derived-capability-tiers); names/digests grant neither feature |
| `completion` | Deterministic `rotate` and two `homogeneous_seats` deck indices, not mechanics |

Admission checks 100 cards, singleton name, commander count/eligibility,
color identity and basic-land mana (CR 903.3–903.5). Serialized profile, catalog,
selected ban digest and normalized private configuration bind the pod. Hosting
also validates the whole pod and both executables; search validates integrity,
fresh root and its [information/reconstruction contract](game-engine-search.md).

All dated `commander-banned-*.json` files are embedded; each profile selects one.
Their official URLs, retrieval date, HTML/archive hashes and named/offensive
exclusions are authoritative. Lutri's companion-only category remains distinct.
Pool review derives from authenticated source legality/type/text and the selected
list, not per-profile copies. A new dated list requires no Rust edit.

Optional compiler-owned `Definition.source_labels` is descriptive metadata. Its
exact Oracle text, verified normalized blocks and lowerer index bindings follow
[the permitted decision-context contract](game-client.md#core-action-discovery).
Active outputs are regenerated through the compiler, never edited by hand.
Removing only this metadata reproduces the prior mechanical IR; requirements and
reviewed Hosting/Search capability sets do not expand. Catalog/build identities
still change and must be qualified independently.

`Definition::requirements` is exhaustive over enum variants, not new struct fields.
Every IR field addition requires reviewing capability derivation and hosted/search
sets before admitting a catalog. Profiles never bypass shared rules or capacities.

## Bulk headless admission

`mtg-sim catalog build-ir ARCHIVE --out DIR` publishes compact schema-1 IR,
the archive outcome index, and their pinned artifact manifest. Bulk format 2
adds source-derived Commander pairing and color facts. A consumer verifies
the exact producer, manifest pin, payload proofs, and successful definition
joins during load.

Dynamic decks contain 100 cards including one or two designated legendary
creatures. Two designations require ordinary Partner on both or Doctor’s
companion paired with a Time Lord Doctor that has no other creature types.
Pairing can be inspected independently of compilation. Admission additionally
requires executable definitions (the `Executes` tier). The two color
identities combine for deck legality and commander-color mana. Each physical
designation keeps its own command-zone cast and combat damage history
([CR 702.124, 903.3–903.10](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).
Dynamic identities carry headless admission only. Every deck passes the same
tier and trigger-capacity bound; no roster identity grants an exception
(admission domain v4 retired the hand-listed capability set).

`mtg-sim bench-pods --bulk-artifact DIR --manifest-sha SHA --pod-decks JSON`
reads an object with a `decks` array of four seat lists. Each entry contains
`oracle_id`, `quantity`, and `role` (`main` or `commander`). Workers share a
single validated projection. The bulk manifest, policy, projection and
per-case admission digest identify the benchmark. The existing worker options,
sampled replay checks and `--analytics-out` apply to both input modes; analytics
require every deck card to earn `HeadlessQualified` and group decks by sorted
commander designations and physical quantities.

## Single matches and deck work

`mtg-sim match-game PROFILE INDEX --journal-dir ABSOLUTE_DIR` plays one seeded
game and streams its live journal under that directory; `match-replay PROFILE
FILE` and `match-replay-live PROFILE FILE` reject profile mismatches.
[Deck work](game-engine-development.md#measurement-and-deck-work) covers staging,
coverage and paired benches.

Develop reads public IR targets/filters/amounts/costs and permitted observations
for attack/removal/burn/wipe estimates; core validates all proposals. No card,
profile or catalog fingerprint selects strategy. DrawOnly exists for draw/decking
witnesses. Type filters include all `additional_types`: positive type lists are
alternatives, exclusions reject any type, sacrifice subtypes are conjunctive;
land/creature roles retain contracts' canonical-kind invariant
([CR 205.2b](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).
