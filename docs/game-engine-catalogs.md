# Catalogs, profiles and build identity

Fixture paths below are relative to `packages/game-engine/`. Use frozen source
records, not refreshed card facts. [Development](game-engine-development.md#cargo-coordination-and-cache-integration)
owns the pinned environment/per-worktree Cargo lock for native commands.

## Catalog source and commands

`tests/fixtures/accepted-archives.json` admits source archives; each
`provenance.json` row pins its raw record and archive manifest. A historical
archive with `file_sha256: null` validates existing fixtures through committed
manifest/record pins but cannot be extracted or claimed as a fresh full-file proof.
The current preserved archive has a full-file pin and remains outside Git.

From repository root, under the shared native lock:

```sh
cargo run --locked -p mtg-sim -- catalog build packages/game-engine/tests/fixtures/catalogs/wipes.json
cargo run --locked -p mtg-sim -- catalog build packages/game-engine/tests/fixtures/catalogs/wipes.json --write
cargo run --locked -p mtg-sim -- catalog extract /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json 'Exact Card Name' oracle-id
cargo run --locked -p mtg-sim -- catalog verify /path/to/accepted.jsonl packages/game-engine/tests/fixtures/provenance.json
```

`build` checks complete supported records, provenance pins, IR bytes and digest.
`--write` regenerates both outputs. `--json-errors` emits a structured diagnostic
on stderr. Coverage publishes both receipts together and preserves occupied
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
`record-groups.json` is historical ordering, never catalog membership authority.

## IR and build identity

`mtg_contracts::CURRENT_IR_VERSION` is the sole executable IR authority, fixed at
**1** until an external consumer needs compatibility. Evolve it in place with
strict capability/shape validation; no old decoder, minimum-version range,
per-feature schema gate or duplicate historical expected output. Regenerate active
IR/digests through the compiler. Archive/snapshot/bundle/public/query formats have
separate identities; source-qualified [evidence](game-engine-evidence.md) retains
historical versions and the schema-1 reset proof.

<a id="engine-code-identity"></a>

`BUILD_ID` hashes engine Rust sources/Cargo manifests, root Cargo manifest/lock,
toolchain and compiler/target/settings. Catalog/profile fixtures, generated output
and server source are excluded; catalog/pod identity and `SERVER_BUILD_ID` bind
those separately. Retained manifests still pin all source/data/server bytes.
Data-only catalog additions do not invalidate unchanged-catalog snapshots.

Compiler watches each catalog/provenance/archive/record input, including absent
repairable inputs, plus the catalog directory. Core watches catalog/profile
directories and each selected data file, including discovered ban policy paths.
Broad fixture-root watches would rebuild catalogs for README edits; broad engine
source/build identity remains intact. Simulator source changes still invalidate
core. See [Cargo build-script watches](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed).

`python3 scripts/engine-structure-check.py` verifies declared direct dependency
boundaries (including target/workspace/alias tables) and the Git index for known
generated paths and recognizable ELF/Wasm/SQLite output. Core imports compiler
only for development tests. It never alters the index; arbitrary renamed JSON or
transitive feature behavior still requires review. Focused guards are
`python3 scripts/test-engine-structure.py` and `python3 scripts/test-engine-cache.py`.

## Compositional lowering

`compiler/src/lib.rs` tries whole-card families first. A record no family
supports falls back to `compiler/src/compose.rs`, which repeats the archive
integrity checks, requires a completely bound reference graph that covers the AST, and lowers each block
through small tables: `compose/costs.rs`, `compose/events.rs`,
`compose/keywords.rs`, `compose/steps.rs` and the shared typed step lowerer.
Unknown kinds, fields, operands and unconsumed references reject. A family
rejection that composition does not recover keeps its original reason, so
coverage reasons stay comparable. `--example composition_gaps ARCHIVE` lists the
table row each remaining card lacks; `--example inspect ARCHIVE NAME` prints one
compile result.

Adding a primitive touches the row that owns it, not every family:

| Primitive | Files |
| --- | --- |
| Cost term, self event, keyword or step shape that lowers to existing IR | One `compose/*.rs` table row and its test; a frozen fixture when it opens a new runtime path |
| Existing instruction, target route or cost in another kind of stack object | One `contracts/src/composition.rs` row, a core witness in `core/src/engine/composition_tests.rs`, and the core fix if recovery assumed a shape |
| New instruction | The `Instruction` variant and its binding arm in `contracts/src/lib.rs`, its `composition.rs` row (the match is exhaustive), a `compose/steps.rs` row, the core executor and a witness |

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
| `hosted`, `search` | Requests validated against reviewed capability sets in `core/src/profile_data.rs`; names/digests grant neither feature |
| `completion` | Deterministic `rotate` and two `homogeneous_seats` indices for cases 16/17, not mechanics |

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
requires executable definitions and reviewed capabilities. The two color
identities combine for deck legality and commander-color mana. Each physical
designation keeps its own command-zone cast and combat damage history
([CR 702.124, 903.3–903.10](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).
Dynamic identities carry headless admission only. The R3 copy-capable pod review
pins both normalized roster rows and complete compiled definitions for one
Cascade seat and three Isamaru control seats. Individual deck inspection can
recognize the reviewed copy deck. Pod admission also checks the exact mix and
copy-aware trigger bound before allocating a game. Changed definitions, quantities,
or combinations require their own review.

`mtg-sim bench-pods --bulk-artifact DIR --manifest-sha SHA --pod-decks JSON`
reads an object with a `decks` array of four seat lists. Each entry contains
`oracle_id`, `quantity`, and `role` (`main` or `commander`). Workers share a
single validated projection. The bulk manifest, policy, projection and
per-case admission digest identify the benchmark. The existing worker options,
sampled replay checks and `--analytics-out` apply to both input modes; analytics
group decks by sorted commander designations and physical quantities.

## Generic completion batches

Reuse a coordinated executable rather than building merely to read a report.
Only run the profile/game needed for the change; examples assume the selected
Cargo target really is `target/`:

```sh
cargo build --release --locked -p mtg-sim
target/release/mtg-sim match-batch commander-wipes-v1 generated/my-wipes-batch
target/release/mtg-sim match-replay commander-wipes-v1 generated/my-wipes-batch/game-00.json
```

`match-batch PROFILE NEW_OUTPUT` runs/replays 18 seeded games from profile rotation
and homogeneous data; output must not exist. `match-game PROFILE INDEX` runs one.
`match-replay PROFILE FILE` and `match-replay-live PROFILE FILE` reject profile
mismatches. Historical reports retain their source/catalog/policy/build qualification.

Develop reads public IR targets/filters/amounts/costs and permitted observations
for attack/removal/burn/wipe estimates; core validates all proposals. No card,
profile or catalog fingerprint selects strategy. DrawOnly exists for draw/decking
witnesses. Type filters include all `additional_types`: positive type lists are
alternatives, exclusions reject any type, sacrifice subtypes are conjunctive;
land/creature roles retain contracts' canonical-kind invariant
([CR 205.2b](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).

`report.json` records per-game counts and aggregate interaction coverage: chosen
invocations versus nonempty committed instruction batches/sites, mode/continuation
counts, trigger collection/placement/resolution and keyword involvement. It lists
unreached instruction sites and unused cards; involvement is not proof a keyword
changed the outcome. [Evidence](game-engine-evidence.md#later-focused-engine-qualification)
owns exact qualification.
