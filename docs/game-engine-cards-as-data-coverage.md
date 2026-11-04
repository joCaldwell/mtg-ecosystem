# Cards-as-data corpus coverage

D2a measures complete-card lowering and IR validation, not Commander deck
admission or complete rules support. Every count uses the record's own
`source.legalities.commander == "legal"` value.

Run from the repository root with the pinned engine toolchain:

```sh
T=/home/josh/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/usr
export PATH=$T/bin:$PATH LD_LIBRARY_PATH=$T/lib
cargo run --locked --release -p mtg-sim -- catalog coverage \
  /home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl \
  --out generated/cards-as-data-coverage/d2a/final
```

For compiler-only iterations, the same implementation is available without
building core/sim:

```sh
cargo run --locked --release -p mtg-compiler --example coverage -- ARCHIVE OUTPUT_DIR
```

The accepted-archives data must pin both the exact manifest and full archive
SHA-256. Unknown archives and changed bytes produce typed errors. No manifest
substitution occurs. The input is read-only. Output defaults to
`generated/cards-as-data-coverage/` if `--out` is omitted.

`summary.json` reports all-record and Commander-legal totals, the legal lowered
count/percentage, legal status counts and descending rejection counts (ties by
reason then status). The top 30 reasons include up to five distinct card names,
sorted alphabetically. `records.tsv` classifies **every** archive record, including
nonlegal cards; its columns are oracle_id, name, status, reason. Backslash, tab,
CR and LF are escaped. Summary counts include only legal records.

Statuses are `lowered_validated`, `unsupported`, `invalid`, and `unparsed`.
Reasons identify an AST discriminator (`trigger:phase`, `effect:gain-abilities`,
`keyword:ward`) or a compiler field path and specific boundary. Field-path
reasons normalize diagnostic punctuation to underscores; serde locations and
expected-variant lists are excluded. These codes describe the first failed
boundary, so fixing one can expose another without admitting a card. Unparsed
means the archive does not contain a complete successful card parse. Unknown
semantics are always rejected for the entire card.

## D2a baseline

Measured on base `317e759` with the committed command, before intake fixes:
38,721 records; 32,116 Commander-legal; **1,719 lowered and validated (5.35247%)**
at IR v20. Legal statuses: 24,694 Unsupported, 3,222 Invalid, 2,481 unparsed.
This reproduces the audit's 1,719 result without substituting a header: D1a
already accepted the current archive through data. Accepted-archive fix delta: 0.

Archive SHA-256:
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`;
parser revision `327b3360ea8e5cffb75b136223e75a348e903cbe`.
Raw baseline: `generated/cards-as-data-coverage/d2a/00-baseline/` (ignored).

## D2a intake measurements

Each output directory contains the full TSV and JSON at that stage. Deltas
are sequential and count newly lowered **and validated** legal records.

| Stage | Lowered | Change | Percent |
| --- | ---: | ---: | ---: |
| Baseline (`00-baseline`) | 1,719 | — | 5.35247% |
| Source normalization (`01-normalization`) | 1,719 | +0 | 5.35247% |
| Multi-face classification (`02-multi-face`) | 1,719 | +0 | 5.35247% |
| Compositional types (`03-type-lines`) | 1,820 | +101 | 5.66696% |
| Strict keyword/node diagnostics (`04-keyword-reader`) | 1,820 | +0 | 5.66696% |
| Inert spell subtypes (`05-spell-subtypes`) | 1,828 | +8 | 5.69187% |
| Accepted archive (already implemented by D1a) | 1,828 | +0 | 5.69187% |

The source adapter now uses case-sensitive Unicode word boundaries, punctuation
canonicalization, full and source-established legendary short names in all
clauses, guarded source pronouns, balanced reminder removal, and complete
modal/dice blocks. Explicit literal-name quotation is canonicalized on both
sides without dropping any name or surrounding text. The resulting comparison
is exact. AST hashes, source hashes, line/aggregate equality and unknown-field
checks remain mandatory for every successful compilation.

Unknown ASTs are rejected before normalization: unsupported station blocks and
self-replacement paragraphs do not become false source-integrity failures.
This changes the first reported boundary, not successful admission. The 2,229
original source-line mismatches and 227 orphan modal-header failures now reach
Unsupported reasons. Legal Invalid falls from 3,222 to 767. There is no coverage
increase because those records still require unsupported semantics.

Multi-face classification (`02-multi-face`): **1,719 (5.35247%), +0**.
All 766 legal records formerly failing single-face required fields now report
Unsupported: transform 341, adventure 147, split 113, modal_dfc 83, prepare 66,
flip 16. Source and binding hashes are checked before layout inspection; absent
single-face fields are never filled with defaults. One legal Invalid remains.

Type-line composition (`03-type-lines`): **1,820 (5.66696%), +101**.
`Basic` and `Legendary` are parsed independently of card types and subtypes.
A canonical execution kind (creature, then land, then artifact, then enchantment)
plus sorted `additional_types` retains all supported simultaneous permanent
types. Empty additional types are omitted, preserving existing IR/digest bytes.
The IR remains v20. Combined types add the corresponding existing capabilities.
Removal's positive/negative type predicates, spell-counter type predicates,
sacrifice filters and enchantment wipes all use complete type membership.
Creature casting, combat and summoning sickness use the canonical creature kind.
Legendary noncreature permanents and inert artifact/enchantment subtypes no longer hit
closed presentation whitelists; their entire AST still has to lower.
Planeswalkers, battles, unsupported card types/supertypes (including Snow and
World), mixed spell/permanent types and land-creatures retain explicit failures.
No new keyword, trigger, snow-cost, world-rule or planeswalker behavior is added.
All 68 compiler tests passed, including exact freshness of every committed
catalog: no expected IR or digest needed regeneration.

Strict keyword/node diagnostics (`04-keyword-reader`): **1,820 (5.66696%), +0**.
Unknown discriminator values and keywords now appear in `CompileResult` itself,
with concrete field paths. Array positions become `[*]` in coverage codes so
moving a clause does not split one rejection family. Serde offsets and expected
variant lists never enter the codes. The mirror still rejects every unknown
node, field, parameter and duplicate key. Hyphenated first-strike/double-strike
aliases map to their existing runtime keywords; JSON field ordering already
works. The archive uses canonical forms for those implemented keywords, so
there is no additional coverage gain. Ward, flash, protection, conditional
hexproof and other unimplemented keyword shapes remain explicit rejections.
All 71 compiler tests passed before the excluded-record reporting correction.

A final type-line review removed the remaining blanket rejection of spell
subtypes (`05-spell-subtypes`): **1,828 (5.69187%), +8**. Subtypes such as Arcane
and Lesson are preserved in IR; they do not introduce an implicit action.
Their entire printed ability list still must lower. The full type-line intake
delta is therefore **+109** (+101 combined/permanent types, +8 spell subtypes).
No subtype-based targeting, splice, learn or alternate casting rule is added.

## D2a final coverage

**1,828 / 32,116 Commander-legal records (5.69187%)**, an increase of **109 cards
and 0.33939 percentage points** over the reproducible baseline. Final legal
statuses: 1,828 lowered/validated, 27,806 Unsupported, 2,482 unparsed, 0 Invalid.
The final reporter also handles archive-excluded rows, which have no AST or
binding. One legal row, `"Name Sticker" Goblin`, is excluded by the archive for
`no-eligible-paper-printing`: it is unparsed with that specific reason. Earlier
stage outputs faithfully retain the initial reporter's Invalid classification
for this row. This reporting correction changes no lowering count.

Top 15 remaining first-failure reasons (including unparsed records):

| Reason | Commander-legal records |
| --- | ---: |
| `status:unparsed` | 2481 |
| `trigger:phase` | 1116 |
| `trigger:attacks` | 897 |
| `effect:gain-abilities` | 866 |
| `trigger:cast` | 789 |
| `static:enters-tapped` | 597 |
| `keyword:flash` | 545 |
| `abilities[*].condition` | 540 |
| `trigger:deals-damage` | 526 |
| `ref:self` | 489 |
| `abilities[*].abilityWord` | 477 |
| `static.scope:unsupported_modifier_scope` | 441 |
| `effects.steps:unsupported_activated_operator_controller_or_cardinality` | 403 |
| `static:conditional` | 403 |
| `multi_face_layout:transform` | 341 |

Full results and examples are retained under
`generated/cards-as-data-coverage/d2a/`; downloaded and generated artifacts stay
ignored. Coverage is a compiler metric. The remaining semantic families above
are future D2 work, and lowering alone does not establish full rules support.

## Verification and evidence qualification

Implementation source freeze: `a0ff576e1a4f0e76a59c0c793a340fd1a4c9666f`.
All 72 compiler tests pass, including every committed catalog's exact expected
IR and digest. No fixture IR/digest regeneration was needed. Core witnesses
cover positive/negative removal filters, sacrifice unions, enchantment/creature
sets, stack filters, casting, summoning sickness and snapshot/replay in all
trace modes. No `sim/src/match_games*` file was changed.

Two release coverage runs took 4.946 and 4.950 seconds and produced byte-identical
JSON and TSV. Each TSV has 38,721 record rows. Comparing legal lowered Oracle
IDs proves all 1,719 baseline cards remain lowered, with 109 additions and zero
regressions. Raw outputs include `final/`, `final-repeat/`,
`reproducibility.json` and `regression-summary.json` beneath
`generated/cards-as-data-coverage/d2a/`.

Final verification phases:

- Formatting and workspace/all-target Clippy: passed.
- `npm run check:engine` Rust phase: **907 passed, 0 failed, 18 existing
  diagnostics ignored**.
- Exact Node phase rerun: **56 passed, 0 failed, 0 skipped**.
- SDK verification against the fresh debug server: **47 passed, 0 failed**, plus
  strict TypeScript and generated schema/declaration checks.
- Release `match-batch commander-foundations-v1`: **18 finished, 0 faults,
  0 truncations, 0 replay failures**.
- Release `match-batch commander-data-admission-v1`: **18 finished, 0 faults,
  0 truncations, 0 replay failures**.

The wrapper invocation completed formatting, Clippy and Rust successfully, then
exited 1 because this fresh worktree lacked `ajv` before the Node contract tests
could load. After local `npm ci --ignore-scripts`, its exact Node command passed:

```sh
node --test scripts/capture-json-output.test.ts scripts/retain-policy-sources.test.ts \
  scripts/retain-server-evidence.test.ts packages/game-engine/tests/contracts.test.ts
```

This is a phase-wise pass qualification: the failed wrapper invocation itself
is not relabelled as passed. No engine source changed between its passing Rust
phase and the successful Node retry. The earlier pre-freeze attempt was stopped
after source-identity checks correctly detected source edits during the run;
its original log and the stale-build diagnostic are preserved separately.
The frozen rebuild passed both source-identity tests. No guard was weakened.

All final phase logs, both complete batch directories, SDK verification and
`verification-summary.json` are retained under
`generated/final-engine-verification/d2a/` (ignored). The release batch build ID
is `76f3950a308cf83a4709191c0ee99a6b591d047c70037e325a780e85d99dce6e`.

## D2b: bound sources, flash and source entry replacements

The [D2b report](game-engine-cards-as-data-d2b.md) records the rules, independent
reviews, source audit, witnesses and exact verification qualifications. Against
an independently rebuilt `a5f59b1` base, sequential measurements give:

| Stage | Legal lowered | Delta |
| --- | ---: | ---: |
| D2a merged base | 1,828 | — |
| Subtype, diagnostic and artifact-land corrections | 1,828 | +0 |
| Bound source references | 1,828 | +0 |
| Flash | 1,882 | +54 |
| Source enters tapped | 1,904 | +22 |
| General ability-word annotations | 1,904 | +0 |

The final result is **1,904 / 32,116 (5.92851%)**, **76 additions and zero
regressions** from D2a. There are zero legal Invalid records. Source binding is a
prerequisite for two of the entry-tapped additions; all five admitted labeled
cards already had supported triggered labels. Separate feature counts therefore
must not be added as independent gains: 56 flash and 22 entry-tapped cards
include two overlaps.

Two final full CLI runs produced byte-identical JSON and TSV, also identical to
the fully enabled diagnostic stage. Raw outputs, the preserved baseline and
feature-mask measurement source are ignored under
`generated/cards-as-data-coverage/d2b/` in the D2b worktree. The report explains
that source-copy measurement and retains its failed preparation attempts.

The largest remaining first boundaries are `trigger:phase` (1,133),
`effect:gain-abilities` (922), `trigger:attacks` (918), `trigger:cast` (808), and
`trigger:deals-damage` (526). These are first-failure counts, rather than counts
of cards that one capability alone would admit. Broader phase/cast/draw trigger
and card-flow work remains the next capability track.


## D2c-a: beginning-of-step triggers

The [D2c-a report](game-engine-cards-as-data-d2c-a.md) describes the exact IR v21
subset, typed event witnesses, source audit, rejected forms and verification.
The final compiler result is **1,923 / 32,116 legal records (5.98767%)**:
**19 additions, zero losses**, and **0.05916 percentage points** over D2b's 1,904.
Legal statuses are 1,923 lowered/validated, 27,711 Unsupported, 2,482 unparsed,
and zero Invalid. All 38,721 archive records are classified in the TSV.

The capability composes upkeep/draw/end steps with controller/any-player/opponent
turn predicates and existing mandatory programs. It does not admit broader phase
forms, intervening conditions, optional effects, event-player recipients or
multi-player draw. A source-backed runtime witness exposed Master of the Feast's
unsupported each-opponent draw; its whole card now rejects before admission.
The initial 1,924 result and that failed runtime attempt remain preserved rather
than being treated as final coverage.

Two final full CLI runs produce byte-identical JSON and TSV, also identical to
the post-guard measurement. Raw results are retained under
`generated/cards-as-data-coverage/d2c-a/`. The baseline TSV is copied from the
retained D2b `resumed-final` output. These are compiler metrics, not deck admission
or a claim that all nineteen cards were played in legal profile batches.

Top fifteen remaining first-failure reasons:

| Reason | Legal records |
| --- | ---: |
| `status:unparsed` | 2,481 |
| `effect:gain-abilities` | 953 |
| `trigger:attacks` | 919 |
| `trigger:cast` | 817 |
| `trigger:deals-damage` | 526 |
| `static:conditional` | 482 |
| `static.scope:unsupported_modifier_scope` | 461 |
| `effects.steps:unsupported_activated_operator_controller_or_cardinality` | 418 |
| `static:enters-with-counters` | 350 |
| `mana:choice` | 346 |
| `effect:reveal` | 341 |
| `multi_face_layout:transform` | 341 |
| `abilities[*].effect.condition` | 329 |
| `abilities[*].effects[*].steps[*]:data_did_not_match_any_variant_of_untagged_enum_sourcecounterrecipient` | 328 |
| `effect:tap` | 317 |

Recognizing phase ASTs exposes later unsupported operations and conditions.
These are first-failure counts, not independent predicted capability gains.


## D2c-b: frozen event players and source status

[The D2c-b report](game-engine-cards-as-data-d2c-b.md) records the IR v22 scope,
rule-backed current/LKI semantics, complete source audits and verification.
The final result is **1,929/32,116 legal records (6.00635%)**, **six additions and
zero losses** from D2c-a's 1,923. The increase is **0.01868 percentage points**.
Legal statuses: 1,929 lowered/validated, 27,705 Unsupported, 2,482 unparsed,
zero Invalid. The TSV classifies all 38,721 source records.

The six additions are Dictate of Kruphix, Font of Mythos, Howling Mine, Kami of
the Crescent Moon, Nim Abomination, and Seizan, Perverter of Truth. Source status
and event-player recipients are independent capabilities; Howling Mine composes
both. Existing flash and ordered single-player draw/life execution are reused.
Optional groups/payments, multiplayer draw, other trigger families and library
selection remain excluded. Master of the Feast still rejects as a whole card.

Two final CLI JSON/TSV outputs are byte-identical, also equal to the frozen-source
preflight. Exact legal-ID additions/losses, complete archived source/AST audits
and extracted/validated IR for all six are retained at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/d2c-b-02f0d4df9234-20261103/coverage/` and the adjacent source-audit artifacts.
These are compiler counts; the twelve unchanged batches do not play new cards.

Top fifteen remaining first reasons (not independent admission predictions):

| Reason | Legal records |
| --- | ---: |
| `status:unparsed` | 2,481 |
| `effect:gain-abilities` | 953 |
| `trigger:attacks` | 919 |
| `trigger:cast` | 817 |
| `trigger:deals-damage` | 526 |
| `static:conditional` | 483 |
| `static.scope:unsupported_modifier_scope` | 461 |
| `effects.steps:unsupported_activated_operator_controller_or_cardinality` | 418 |
| `static:enters-with-counters` | 350 |
| `mana:choice` | 346 |
| `effect:reveal` | 342 |
| `multi_face_layout:transform` | 341 |
| `abilities[*].effects[*].steps[*]:data_did_not_match_any_variant_of_untagged_enum_sourcecounterrecipient` | 331 |
| `abilities[*].effect.condition` | 329 |
| `effect:tap` | 317 |


## D2c-c: verified ordinary cast triggers

**Source review, combined acceptance at `a7f5571` and parent evidence audit passed.**
Fresh pinned compiler coverage measures **1,968/32,116 (6.12779%)**, **39 additions and zero
losses** from the exact D2c-b legal lowered-ID set. Increase: **0.12143 percentage
points**. All 38,721 archive rows are classified; legal statuses are 1,968
lowered/validated, 27,666 unsupported, 2,482 unparsed and zero invalid.

IR23 separates caster relations from compositional positive/negative full type
membership and AND/OR predicates. It reuses mandatory programs, with a frozen
real successful-cast witness. Colors, zones, ownership, targets-source, MV/X,
ordinals, turn limits, self-cast stack functioning, cast-or-copy and optionality
remain rejected. Hosting/search exclude the capability. See
[the D2c-c report](game-engine-cards-as-data-d2c-c.md) for rules, exact source
identities, focused checks and persisted-version implications.

The 39 audited additions are:

Arasta of the Endless Web, Argothian Enchantress, Beast Whisperer, Blessed Spirits, Boar-q-pine, Chandra's Emberling, Citanul Druid, Contemplation, Dawnhart Geist, Deeproot Champion, Dragonlair Spider, Ebon Drake, Electrostatic Field, Electrostatic Infantry, Enchantress's Presence, Erebor Flamesmith, Firebrand Archer, Generous Visitor, Guttersnipe, Kambal, Consul of Allocation, Kessig Flamebreather, Managorger Hydra, Medusa, Inhuman Queen, Murmuring Mystic, Pyre Hound, Pyroceratops, Rite of the Dragoncaller, Satyr Enchanter, Sigil of the Empty Throne, Spellgorger Weird, Sprite Dragon, Student of Ojutai, Sunscorch Regent, Sythis, Harvest's Hand, Talrand, Sky Summoner, Tura Kennerüd, Skyknight, Vedalken Archmage, Whirlwind of Thought, Young Pyromancer.

Every complete source/AST and extracted validated IR is retained under
`generated/final-engine-verification/d2c-c/source-admissions/`; exact ID-by-ID
comparison and all archive classifications are under
`generated/cards-as-data-coverage/d2c-c/`. These are compiler metrics, not a claim
that the new cards played in unchanged legal profile batches. No full gate was
authorized for this implementing agent.


## D2c-d accepted declared-attack increment

Frozen `394301d` passed combined acceptance and independent parent artifact,
source and complete-output audit. Coverage is **1,985/32,116 (6.18072%)**, exactly
**17 additions and zero losses** from accepted a7. Final/repeat records and summary
are byte-identical to reviewed D2c-d; all 23 exactsource fixtures and 17 complete
source/AST/binding/IR admissions were checked against the pinned archive. All 79
prior reference files and 173 history hashes remain unchanged. See [the D2c-d
report](game-engine-cards-as-data-d2c-d.md) for exact identities and retained
proof. These are compiler metrics; the new cards were not added to the unchanged
216-game acceptance workload. IR25 and subsequent optimizer candidates are excluded.
