# Oracle parser checkpoints

## 2026-10-16 — reference binding

After the coverage target was met, work moved to the reference-binding
analyzer (`src/semantic/bindings.ts`): deciding, for every accepted card, what
each printed reference ("it", "that player", "you", X, "the rest") denotes.
Results now have two levels (see [oracle_parser.md](oracle_parser.md)):
**references bound** — every reference resolves — and **fully bound** —
references bound and no rules-engine feature still needed. Constructs that
need event history, damage prevention, or the layer system are recorded as
`engineFeatures` rather than counted as binding failures.

| Measure (Commander-legal accepted cards, 29,634) | Result |
| --- | --- |
| References bound | 23,395 (78.9%) |
| Fully bound | 21,998 (74.2%); 4,919 at the 2026-10-15 checkpoint |
| References bound, awaiting an engine feature | 1,397 |
| References not yet bound | 6,239 |
| All in-scope cards (33,590) | 24,017 references bound; 22,611 fully bound; awaiting event-history 762, damage-prevention 398, characteristic-layers 256 |
| Parsing acceptance | Unchanged: 29,634 / 32,115 Commander-legal (92.27%) |
| Package checks | Typecheck and 2,906 tests passed; none skipped |

Each binding rule cites the Comprehensive Rules (2026-09-25) and is listed in
oracle_parser.md. Every rule was checked by diffing all changed bindings across
the corpus against the previous state; wrong bindings found that way were
fixed, including several that were already "fully bound" before this work
(blockers dealing damage to themselves, cast triggers countering the
permanent instead of the spell, "unless ... pays for each creature you
control" counting the payer's creatures). One sampled independent review
round (465 newly bound cards, 3 defects, all fixed) was run early in the
work; the later rules were reviewed only by the corpus diffs. No card bound
before this work lost its binding.

The AST gained one optional field, `Sentence.conditionPosition: "after"`, for a
trailing "if" clause. The per-card binding format gained `referencesBound` and
`engineFeatures` (kept at schema version 1 while nothing consumes it).

| Input | SHA-256 |
| --- | --- |
| Parser and reporting sources | `316c9deaba977533329b903e9d9ba48b6d02057651702c05efa460fb651fbd3a` |
| Oracle corpus | `f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738` |

### What remains

1. **Independent audit of the binding rules.** Before the compiler relies on
   these bindings, a fresh reviewer should check a sample of cards bound by the
   later rules (zone successors, quantities, announced X, linked "this way"
   events, alternative triggers, "defending player", would-be controllers,
   engine-feature constructs) against their Oracle text.
2. **Remaining reference gaps** (6,239 Commander-legal cards). Counts are cards
   where the group is the only blocker / cards it affects at all:

   | Group | Only blocker / affected |
   | --- | --- |
   | Created replacement effects ("the next time ... instead"), repeatable modes, random results | 199 / 304 |
   | No antecedent found for a pronoun | 192 / 722 |
   | Prohibitions and cast limits ("can't cast", per-player cast counts) | 114 / 158 |
   | Designations, turn ownership, player comparisons (monarch, day/night, "the player with the most life") | 108 / 144 |
   | Relations to attachments, events and pools ("haunted creature", "exiled with this", replacement-event objects) | 107 / 746 |
   | More than one possible antecedent (deliberately not guessed) | 106 / 531 |
   | Copies of spells and abilities, and choosing new targets | 97 / 177 |
   | Look permissions, commander eligibility, untap choices | 96 / 138 |
   | Costs with their own choices (collect evidence, forage, waterbend) | 85 / 119 |
   | Conditions about an earlier step ("if you won the clash", votes, "if you did this way") | 84 / 128 |
   | "If you do" with no single optional step to link to | 79 / 175 |
   | Per-object dynamic values ("where X is its power" for each creature) | 78 / 100 |
   | Linked choices ("the chosen color", "the chosen player") | 72 / 166 |
   | Numbers with no definition | 65 / 158 |
   | Keyword activation history (intrinsic keyword rules) | 65 / 83 |
   | Smaller groups (each under 60): per-player predicates, delayed-trigger timing, quantified members, cardinality mismatches, unresolved "another", grants to non-objects, trigger-time conditions, counter-change triggers, entering-attacking choices, event quantities, attack groups, entry copies | — |

   Each needs its own rule grounded in the Comprehensive Rules; returns are
   diminishing and the risk of a wrong binding rises with specialisation. An
   unresolved reference is always preferred to a guessed one.
3. **Engine features** (1,397 Commander-legal cards whose references are
   bound). These wait on the Rust rules engine providing an event history,
   prevention/redirection shields, and the layer system; the analyzer records
   which feature each card needs.
4. **Parsing tail.** 2,481 Commander-legal cards (7.7%) still fail to parse.
   This hard tail (rules layers, replacement effects, complex targeting) is
   paused pending a stronger model, per the root manual.
5. **Card IR emission.** `npm run build-ir` still fails deliberately. Semantic
   validation and lowering to Card IR belong to the Rust compiler in
   `packages/game-engine/`, which will consume the archive and these bindings.

## 2026-10-15 — 92% of Commander-legal cards

Coverage work resumed on 2026-10-11. The target was later set to 92% of the
in-scope cards that are legal in Commander (Scryfall `legalities.commander ===
"legal"`); cards that are not legal in Commander were left out of the target.
The target is met. As before, acceptance is not semantic correctness.

| Measure | Result |
| --- | --- |
| Commander-legal cards accepted | 29,634 / 32,115 (92.27%) |
| All in-scope cards accepted | 30,347 / 33,590 (90.35%) |
| Accepted ability blocks | 56,905 / 60,600 (93.90%) |
| Package checks | Typecheck and 2,828 tests passed; none skipped |
| Reference analysis | 5,298 cards fully bound; 61,889 of 153,354 obligations unresolved |
| Change from the 2026-10-11 assessment on this corpus | 22,656 → 30,347 cards; 2 deliberately rejected (Boss Uramon, Glorious Dragon-Kin: a leading modifier whose reach over the second noun is not stated) |

Every new accepted line shape was reviewed independently against its card
text, and a deleted-word probe checked that no meaning-bearing word was
consumed without being represented. Defects found were fixed or the cards were
made to fail; conventions kept after review are documented on the AST nodes.

| Input | SHA-256 |
| --- | --- |
| Parser and reporting sources | `fc3b4ec3b8b3ea27e8363225266379c0aebefdf1d5bc243e6e0d141084462c21` |
| Oracle corpus (203,323,272 bytes) | `f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738` |
| Paper-printing evidence (8,216,961 bytes) | `d3a68c4ae14b1349a9d7b9972a19ca8176db06133020b690eb7348d0d972c699` |

`npm run validate` reports the Commander-legal line alongside the full scope.

## 2026-10-10 — checkpoint 11

Work stopped at the user's request after checkpoint 11. The original goal of
98% complete, correct card support is **not achieved**. Further grammar work
has not started beyond this checkpoint.

### Verified result

| Measure | Result |
| --- | --- |
| Completely accepted cards | 22,509 / 33,299 (67.60%) |
| Accepted ability blocks | 47,417 / 60,047 (78.97%) |
| Package checks | Typecheck and 986 tests passed; none skipped |
| Change from checkpoint 10 on the same corpus and scope | 355 newly accepted cards; zero lost cards; zero changes to previously accepted ASTs |
| Whole-card review of those 355 additions | All faces and complete ASTs reviewed; reviewed hashes matched the final report |
| Stored source records | 38,626, including unsupported and excluded records |

These are parser acceptance counts, not a certificate that every accepted card
is ready for a rules engine. Reference binding remains incomplete, semantic
lowering is unfinished, and the archive declares `executable: false`.
Only 4,701 cards currently have all tracked references bound; across the
corpus, 37,522 of 103,064 binding obligations remain unresolved. Even complete
binding is not a substitute for engine validation.
`npm run build-ir` intentionally fails until executable IR can be validated.

### What this work preserves

The parser now represents substantially more costs, keywords, choices, triggers,
conditions, filters, replacement effects, permissions, characteristic changes,
and linked instructions as typed nodes. Structural tests and negative cases
cover distinctions such as actors, owners, optionality, target provenance,
event grouping, durations, and conditional scope. Unsupported constructions
remain failures.

The final checkpoint adds resolving characteristic groups, counter-change
triggers, characteristic relationships, movement-outcome conditions, and
additional cast-history conditions. Independent review caught and corrected
Deicide's conditional shuffle and a binding-analysis fallthrough. Trigger
matching rejects fresh target declarations where targets cannot yet be chosen.

Archive schema 3 stores complete source objects, every face's parse result,
binding analysis, hashes, and exact original corpus and printing-evidence
snapshots. Independent readback compares all records to the frozen input and
report, verifies hashes, counts and binding references, and checks target
provenance. Ingestion and evidence readers reject malformed or invalid UTF-8
input without silently dropping records or replacing a good cache.

The scope is `paper-printing-evidence-non-silver-non-acorn-v2`. It uses actual
eligible paper-printing evidence rather than treating a digital representative
printing as proof that the Oracle identity is digital-only. This correction
added 1,067 identities and removed none; the denominator was not reduced to
improve coverage. Earlier v1 percentages are not directly comparable.

### Reproduce and resume

Use Node 26 and the frozen local inputs; do not refresh ingestion between
comparisons. From the repository root:

```sh
npm run check --workspace=@mtg-ecosystem/oracle-parser
node scripts/validate-parser.ts --json > .scryfall-cache/validation.json
npm run archive-parser
```

Validation and archive generation use these fingerprints:

| Input | SHA-256 |
| --- | --- |
| Parser and reporting sources | `98980a967a70d57008847ce0952edf0cc277af9ad26b8cb8739a555d97082b8d` |
| Oracle corpus (202,839,918 bytes) | `5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60` |
| Paper-printing evidence (8,216,961 bytes) | `d3a68c4ae14b1349a9d7b9972a19ca8176db06133020b690eb7348d0d972c699` |

The source fingerprint includes working source contents, independently of the
Git revision recorded by each run. Rules references use the official
2026-09-25 Comprehensive Rules. Generated artifacts remain local and ignored:

- `.scryfall-cache/parser-98-checkpoint11.json`: final per-card coverage and hashes.
- `.scryfall-cache/checkpoint11-review.json`: checks and review handoff.
- `.scryfall-cache/checkpoint11-grammar-gain-audit-coverage.json`: all 355 additions linked to matching reviewed ASTs.
- `.scryfall-cache/archive-checkpoint11-independent-audit.json`: complete archive readback verification.
- `generated/oracle-ast.jsonl`: source records, ASTs and binding results, with content-addressed input snapshots beside it.

Remaining work includes unsupported grammar, resolving symbolic references,
semantic validation and engine capability checks, and executable IR emission.
Read-only investigations for historic/devotion values and unblocked-attacker
triggers are saved locally; neither was implemented or counted as a success.
Continue with correctness reviews and the same corpus/scope before expanding
coverage. See [the parser design](oracle_parser.md) and
[storage schemas](data_schemas.md) for the current contracts and limitations.
