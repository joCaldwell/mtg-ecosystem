# D2c-c: compositional ordinary cast triggers

Implementation starts at `9d279986c7fcc10f6d179a165968e71f751ba95f`.
Source freeze: `795dd722075b3edebb1d4d1bb87e1778e1274777`. The subsequent report commit changes Markdown only.
**Independent source review and the final combined acceptance gate are pending.**
Only focused implementation checks and whole-corpus compiler/source audits are
claimed here. The parent owns integration and final acceptance after review.

## Supported semantics

IR23 adds `CastTriggerV1` and `TriggerCondition::SpellCast`. Caster relations are
controller, any player and opponent, independently of the spell predicate and
active player. Source AST `you`, `a-player`, `an-opponent` lower into those
relations. Sources function on the battlefield only, with the existing mandatory
trigger-program vocabulary. Draw/life event-player recipients bind the frozen
caster; ordinary spells/activations and unrelated triggers still reject them.
The independent death-controller binding remains unchanged.

`SpellPredicate` represents any spell, positive/negative type membership, AND and
OR. The structured source ObjectFilter defines fields and ordinary array entries
as conjunctive (`packages/oracle-parser/src/ast.ts`); explicit `anyOf` stays OR.
Runtime uses `Characteristics::has_type`, including artifact/enchantment creatures.
The finite structural bound is 32 predicate nodes and depth 8. The unconstrained
predicate uses an empty struct variant so serde rejects unknown fields there too;
its JSON remains `{"predicate":"any"}`. Colors, color
identity, subtypes/supertypes, ownership/zone/targets/source filters, mana value/X,
ordinals/first/second spell, during-turn qualifiers, limits, self-cast stack
patterns, cast-or-copy, optional payments/groups and unknown fields reject whole
cards. Cast intervening conditions are deliberately outside this first subset.
Whispering Wizard's once-per-turn limit remains rejected. Rhystic Study's
optional draw/unless payment and Master of the Feast's each-opponent draw remain
unsupported; the multiplayer-draw runtime guard is retained. Event-player damage
is not implemented, and its typed-IR guard remains.

`payment_step` collects only after every payment/additional cost succeeds and
the real `Event::SpellCast` is recorded (pinned CR601.2h–i, 603.2g/603.3). Announced,
reversed/unpaid casts, land plays, mana/nonmana activations, entry and resolution
never call it. Copying is still unsupported; neither copy insertion nor cast-or-copy
semantics is added. Collection precedes stabilization/priority. Existing APNAP and
controller ordering places triggers above the original spell, including command
casts/tax and nonactive flash casts. Countering the spell preserves its trigger;
countering the trigger preserves the spell. Source/owner departure preserves a
surviving controller's invocation, controller departure removes it, and actor-only
instructions skip a departed caster while later controller instructions continue.
Exact source incarnations and existing current/LKI source facts stay distinct.

## Occurrence identity and persisted representation

A cast is not a movement or a fabricated batch slot. `TriggerOccurrenceId` has
separate batch and cast domains. Cast IDs refer to the existing real successful
`SpellCast.stack_id`; that stack ID is unique across proposals and invocations.
The typed `CastOccurrence` witness freezes caster, stack ID, cast origin and the
actual spell's full `ObjectBefore` facts. There is no active-player fallback or
invented object subject. Pending, placed and resolving trigger invocations retain
those facts. Structural validation checks identity domains and typed facts;
independent accepted-input replay proves actual occurrence and frozen evidence,
including coherent rehashed lies.

The batch alternative serializes exactly as the old `{batch,path}` structure;
old witness/event encodings are unchanged. Cast witnessing is an additive variant,
with no extra event, counter, global LKI history or snapshot field for catalogs
without cast triggers. IR advances 22 → 23, future probes advance 23 → 24.
Snapshot/bundle stay 19/20 and projection/context/sidecar stay 16/16/15. No old
expected catalog or historical digest is regenerated. The baseline byte proof
retains all 76 old catalog/expected-IR/digest/history files byte-for-byte against the base. Final
runtime equality with the 173 unchanged references remains a parent acceptance
check, not a focused implementer claim.

## Source and coverage audit

The accepted read-only JSONL remains SHA256
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`, parser
`327b3360ea8e5cffb75b136223e75a348e903cbe`. The local pinned 2026-09-25 CR file is
SHA256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Exact rule excerpts are retained in `pinned-rules.json`.

The eight-definition fixture catalog contains Student of Ojutai (line1657),
Sythis (2381), Firebrand Archer (6913), Sigil of the Empty Throne (8945),
Kambal (11048), Pyre Hound (1591), Generous Visitor (969) and Managorger Hydra
(27104). Whispering Wizard (7466) is an exact whole-card negative. Extraction
verifies the complete pinned archive, copies exact record lines and records
provenance. Synthetic tests explicitly label predicate/program/flash mutations;
they are not claims about printed abilities or legal hosted pods.

The exact legal lowered-ID comparison is **1,929 → 1,968 / 32,116 (6.12779%)**,
**39 additions, zero losses**, +0.12143 percentage points. Every archive row is
classified: 1,968 legal lowered/validated, 27,666 unsupported, 2,482 unparsed,
zero invalid; 38,721 total records. Across legal and nonlegal rows the lowered set
is 1,958 → 1,997. Every addition is legal. All 39 complete source records, ability
ASTs, binding/provenance and exact archive identities are retained and audited;
coverage does not infer gains from the former 817 first failures.


The exact additions are:

Arasta of the Endless Web, Argothian Enchantress, Beast Whisperer, Blessed Spirits, Boar-q-pine, Chandra's Emberling, Citanul Druid, Contemplation, Dawnhart Geist, Deeproot Champion, Dragonlair Spider, Ebon Drake, Electrostatic Field, Electrostatic Infantry, Enchantress's Presence, Erebor Flamesmith, Firebrand Archer, Generous Visitor, Guttersnipe, Kambal, Consul of Allocation, Kessig Flamebreather, Managorger Hydra, Medusa, Inhuman Queen, Murmuring Mystic, Pyre Hound, Pyroceratops, Rite of the Dragoncaller, Satyr Enchanter, Sigil of the Empty Throne, Spellgorger Weird, Sprite Dragon, Student of Ojutai, Sunscorch Regent, Sythis, Harvest's Hand, Talrand, Sky Summoner, Tura Kennerüd, Skyknight, Vedalken Archmage, Whirlwind of Thought, Young Pyromancer.

## Focused verification and ownership

The focused implementation checks pass **81 Rust tests**, zero failures:

| Command selector (all Cargo commands use `--locked`) | Passed |
| --- | ---: |
| `cargo test -p mtg-compiler -p mtg-core --lib d2cc_ -- --test-threads=1` | 5 compiler + 22 runtime |
| Compiler `strict_mirror_reports_specific_nodes_keywords_and_fields` | 1 |
| Compiler `d2c_tests`, `d2cb_tests`, `all_committed_catalogs_are_fresh` | 3 + 2 + 1 |
| Contracts `every_capability_has_a_minimum_ir_version_witness` | 1 |
| Core `engine::trigger_tests::` | 7 |
| Core `engine::step_trigger_tests::` (including D2c-b) | 30 |
| Core integration `--test triggers_adversarial` | 8 |
| Simulator `d2c_summary_uses_stable_keys_for_data_bearing_trigger_conditions` | 1 |

`cargo fmt --all -- --check`, warnings-denied all-target Clippy for contracts,
compiler, core and simulator, and the fresh debug simulator build passed. The
focused prebuilt-CLI Node schema helper passes its cast predicates, minimum IR,
unknown-field negatives and all **26** old/new catalog positives. No full Node
demonstration was run. Native introspection verifies all **571 compiled source
files** against current bytes; this is a source/identity check, not an offline
rebuild. Fresh CLI build ID:
`82c5732908aee2076f14a4cc413602aefba54e57a555db685a35084104e7465b`.

Final and repeat coverage JSON/TSV are byte-identical and also equal the preflight.
`final-repeat-proof.json` records exact hashes and IDs. The three explicit corpus
regressions remain Unsupported: Whispering Wizard (`abilities[*].limit`), Rhystic
Study (`abilities[*].effects[*].steps[*].optional`), Master of the Feast
(`definition:unsupported_each_opponent_draw`). The 39 whole sources, AST/binding
provenance and expected IR were additionally matched by a parent **preliminary
uncommitted source audit**; it is not final approval or runtime verification.

Every Cargo compilation/check honored the shared `cargo.lock`. The long lock
waits were deferred checks, not tests or acceptance attempts. The final native
source adjustment after the 27-test cast freeze was a **test-only** borrow in
`sim/match_games/interactions.rs`: `to_value(&d.triggered[0].condition)`. It is
necessary because compositional predicates make the condition non-Copy. That
single affected simulator test, all-target Clippy and the fresh CLI build pass.
`post-focus-source-delta.json` proves that all core/compiler production and test
bytes remain identical to their passing freeze. Unaffected passed runtime phases
were not repeated for this test-only adapter correction.

The optimizer-owned production logic is unchanged. Integration overlaps are:
`contracts.test.ts` future probes 23→24 plus an import/new schema test, and the
single borrowed serialization expression in the existing simulator step test.
The parent must combine these with the optimizer's changes. No edits were made
in `sim/match_games.rs`, `sim/main.rs` or `scripts/check-engine.ts`. The historical
movement adapter in `sim/src/triggers_games.rs` explicitly requires a batch
occurrence while retaining every uniqueness, incarnation and outcome assertion.
The mirror's generic unknown-trigger diagnostic now uses unimplemented attacks;
missing cast actor/predicate fields remain explicit whole-card negatives.

Preserved failures are individually logged in `attempt-inventory.json`: the
initial batch-identity test/adapter compile errors; the Counterspell fixture with
only one Island for UU (corrected by adding the second source-backed Island);
missing Ajv before workspace dependency installation; direct rustfmt missing the
pinned loader path; and the non-Copy simulator test compile failure. No expected
hash or semantic assertion was weakened to obtain a pass.

No `npm run check:engine`, full workspace suite, full demonstration, all-profile
batches/replays, SDK acceptance or end-to-end acceptance was run here. Independent
final source review and the parent-owned combined gate remain pending.

Ignored evidence roots in this assigned worktree:

- `generated/final-engine-verification/d2c-c/`: exact command logs, schema,
  extraction, source/IR identities, pinned rules, baseline byte proof, retained
  failed attempts and source-admission manifests/records.
- `generated/cards-as-data-coverage/d2c-c/`: immutable copied D2c-b baseline,
  current JSON/TSV classifications, exact ID delta and complete new source/AST
  audit.

These artifacts remain in this worktree for review and parent-owned acceptance;
no external evidence seal or offline rebuild is claimed. No parser or other
worktree was edited. No subagent, signing-configuration change, PR or merge is
part of this task.
