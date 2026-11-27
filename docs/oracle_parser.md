# Oracle text parser

The hand-written TypeScript parser at `packages/oracle-parser` owns normalization,
syntax AST and diagnostic reference analysis. Rust owns complete-source
binding/lowering and executable Card IR ([system architecture](../README.md#system-architecture),
[catalog admission](game-engine-catalogs.md)). Broad grammar expansion is paused;
parsing and reference binding never certify engine execution. This guide owns
parser semantics, data/archive contracts and ingestion; [the package manual](../packages/oracle-parser/AGENTS.md)
owns local development constraints.

## Pipeline and public API

```text
Oracle text → normalize → ability blocks → lex → parse → typed AST
                                                       ↓
                                           reference binding analysis
```

`parseOracleText(text, cardName?, context?)` returns the discriminated
`ParseCardResult` in [ast.ts](../packages/oracle-parser/src/ast.ts) (no separate
JSON Schema). Successful cards have aggregate `abilities`; failures have only
per-block `lines`, keeping successful blocks and diagnostics. `lines[].text` is
normalized, not an original source span. `ok: true` means every block was consumed
by supported productions; it is not semantic certification, and unknown syntax
never becomes raw executable text.

`normalizeOracleTextTracked` returns the same physical lines, with their UTF-8
byte ranges and a sparse edit map from decoded Oracle text to normalized lines
joined by LF. Copies are implicit. Edits identify punctuation, whitespace and
reminder removal, self-names, pronouns, and literal names. Removed reminders keep
their original ranges. `projectNormalizedRange` returns original ranges and
generated quote anchors. `mapToOriginal` returns the enclosing original range.
Reserved characters, invalid Unicode, and rewrites the edit map cannot represent
return the legacy lines with an explicit unmappable reason.

`parseOracleTextWithPrintedEvidence` returns the archive-context parse result
and an inert face sidecar. [The wire schema](../packages/oracle-parser/src/printed/schema.ts)
keeps physical token order and grammar indices, committed production spans,
folded clause ranges, and lexical facts. Target indices count introductions
within the enclosing ability. Removed reminders have original byte ranges
and may supply original-only citations. Hulls locate containers: a hull's
span is exactly the union of its child extents and the attributed clause
runs it lists in `clauses`, disjoint from child extents, so Rust recomputes it. Unlocated nodes flag missing proof. Aliases point to the same printed
operand when coordination shares one subject, duration, or destination.
Binding and resolution consume the existing parse result.

Implicit evidence identifies an unstated singular target
(`implied-singular-target`), an omitted imperative actor
(`imperative-controller`), or an inferred zone (`default-zone`).
An omitted actor after a printed "you" in the same sentence, and each member
of a "you may" group, aliases that printed "you". When the AST drops that
printed subject (Cruel Ultimatum), the elided actor falls back to
`implicit: imperative-controller`.
Printed pronouns, zone names, and amounts retain their own token spans.
When a later clause changes a recorded node, the parser folds that clause's
tokens into the claim. At finalization the capture compares the node's own
keys, scalar values and primitive array elements with its last recorded snapshot. An uncovered change
increments the mutation audit count and leaves only a hull of child or
folded spans, or an unlocated node when those spans are absent.

Wire version 1 omits `generatedQuotes`. Replay derives generated enclosure
anchors from literal edits whose original slices are unquoted after
punctuation canonicalization. A quoted slice must parse as one JSON string
literal. A literal token's original range remains the
whole edit hull. Lexical and parse failures retain their diagnostic and range.

## Responsibilities and invariants

- Normalize punctuation, reminder text and exact-case self-references, preserving
  newlines; malformed parentheses remain errors. A complete-input `CardNameCatalog`
  supplies card, face and token names; grammar rejects unknown or ambiguous names.
- Lines ordinarily delimit abilities. Modal headers own all following bullet,
  Spree `+` or weighted `{P}` lines; consume every option or fail the block. An
  instruction-only self-replacement paragraph joins its preceding spell block
  (CR 614.15).
- Lexing never drops unknown characters. Parsers build typed nodes, never `any`
  or lossy approximations; a `null` result restores cursor and elided-subject
  context through `Cursor.attempt`.
- Diagnostics come from the original classifier attempts; farthest-failure token
  locations are not original character offsets.

## Data contracts

The [Scryfall adapter](../scripts/lib/scryfall.ts) validates consumed fields and
preserves the rest. [Card objects](https://scryfall.com/docs/api/cards) and
[bulk data](https://scryfall.com/docs/api/bulk-data) distinguish printing `id`
from `oracle_id`; reversible faces carry Oracle identities. Missing/null Oracle
text differs from present empty text. `oracle_cards` selects representative
printings, not a complete set/printing index.

Corpus calls pass each face's `typeLine` (so permanent text cannot pass as a
one-shot spell), layout and complete `cardNames` catalog. Binding requires
`includeSourceProvenance: true`, which the validator and archive enable. Product,
backend and generated SDK contracts stay separate from this AST.

### AST archive

`npm run archive-parser` reads the local cache and writes ignored
`generated/oracle-ast.jsonl`; `--input PATH --output PATH --paper-evidence PATH`
select a frozen corpus and its evidence snapshot. R5 reuses the exact R4 corpus
and paper-printing evidence. The engine pins both archive identities for
[comparison and provenance](game-engine-catalogs.md#catalog-source-and-commands). It never downloads or emits
executable Card IR. [Archive code](../scripts/archive-parser.ts) owns exact fields;
the schema-4 manifest declares `executable: false` and pins corpus, parser source,
rules edition, scope and paper-evidence identities with summary counts. Source
changes during generation fail publication.

Adjacent content-addressed `corpus.snapshotFile` and `paperEvidence.snapshotFile`
preserve exact original bytes and download identity; decoded `source` is
metadata, not the raw-input authority. Existing snapshots are verified, never
overwritten; missing or corrupt inputs block publication. Scope exclusions are
rechecked against actual paper-printing witnesses, never names or legality.
Publication writes a unique adjacent temporary file and reads back every record,
verifying serialization and raw/source/AST/binding hashes before atomic
replacement; any failure keeps the prior archive and removes only owned
temporaries.

Each subsequent line keeps one complete input record in original order, with
`source`, `sourceIndex`, `oracleIdentity` and `sourceSha256` (the complete
Scryfall object and provenance), and one status:

- `excluded`: exclusion/duplicate reason, outside acceptance.
- `parsed`: all faces succeeded; full AST/normalized blocks and complete-card AST hash.
- `unsupported`: all face results/diagnostics, no complete-card hash; missing/null
  text stays explicit `missing-oracle-text`, distinct from empty.

Schema 5 is the R6 same-corpus extension. Excluded rows retain exact schema-4
bytes. In-scope rows append `printed` and the SHA-256 of its raw JSON subvalue.
The header pins the R5 baseline, rules bytes, printed schema, vocabulary and
generator, plus the name catalog and ordered evidence digest. The latter
hashes source index, Oracle identity and evidence hash (or `excluded`) separated
by tabs and terminated by LF, in source order.

Rust verifies the sidecar before classifying a card. It replays typed edits
from source Oracle text to legacy normalized lines, checks UTF-8 token spans
and quote order, and verifies node locations, fact ownership, spelling, target
scope and linked field values. These integrity failures abort archive intake.
The inert reader passes the legacy parse to lowering. Family coverage and
evidence consumption are separate obligations for migrated lowering.

In-scope rows carry `resolution`/`resolutionSha256`, the compiler's authoritative
contract ([resolution.ts](../packages/oracle-parser/src/semantic/resolution.ts)),
per parsed face and block with block-relative paths:

- `refs`: each reference resolved to the closed `Ref` vocabulary shared with
  Kernel IR (including event players and linked results), or `unresolved` with
  the exact reason. A linked result may name another block of the same face only
  for the cases `resolution.ts` admits; references inside granted or created
  abilities never link, and other cross-block references stay `unresolved`.
- `steps`: each printed step's event or [structural](../packages/oracle-parser/src/semantic/step-vocabulary.ts)
  scope; a step the binder missed is `unaccounted`.
- `spans`: a certificate tiling the normalized block text in UTF-8 byte offsets,
  naming the AST field each clause produced (bullet marks are the only trivia).
- `residuals`: why a block or card is incomplete; empty means complete.

Rows also keep the diagnostic `binding` graph the resolution derives from (the
compiler does not read it) and `bindingSummary` counts (see
[binding invariants](#semantic-and-binding-invariants)). Neither a parsed row, a
bound graph nor a complete resolution establishes executable support.

The compiler decodes these shapes through `mtg-oracle-ast`, generated from
`ast.ts` and `resolution.ts` by `scripts/generate-rust-ast.ts` (every union needs
an explicit discriminant). Engine guards run `node scripts/generate-rust-ast.ts --check`;
regenerate after changing those sources.

The same generator emits the printed types and vocabulary in
`mtg_oracle_ast::printed` and `mtg_oracle_ast::vocab`. While the R6 bridge is
active, its guard pins every legacy generated byte outside the source hash line.
`PrintedCard::decode` and `PrintedFace::decode` take original JSON and report
omitted, null, unknown and malformed values at RFC 6901 pointers. They reject
duplicate keys, enforce unsigned 32-bit wire integers and check tuple arity
and range order. Optional and nullable fields retain their distinct states.
A correctly typed value that differs from its required literal reports `Unknown`.
A4 and B1 must hash the original record bytes. Rust emits Serde tags before sorted fields, and a round trip only proves JSON value equality.

`printedIdentity()` in the generator supplies the schema-5 printed header
version and SHA-256 pins. The schema and vocabulary pins hash compact UTF-8
JSON descriptors with sorted declarations, field names, cases and vocabulary
tables and entries. The generator pin hashes the exact generator source bytes.

### Executable output boundary

Rust owns Card IR schema 1 and its validation; archive schema 4 is independent.
`mtg-sim catalog build-ir ARCHIVE --out DIR` publishes whole-archive bulk IR
([bulk admission](game-engine-catalogs.md#bulk-headless-admission)); per-set output
would need complete set membership. Generated `packages/card-data/sets/` is never
hand-edited or committed; engine catalogs remain small reviewed source fixtures.

## Semantic and binding invariants

The AST is experimental and changes without compatibility adapters;
[ast.ts](../packages/oracle-parser/src/ast.ts) owns the representation and
structural tests pin each distinction. Unsupported wording fails rather than
inventing scope.

The AST keeps symbolic references (`it`, `that-player`, `x`, keyword names).
[bindings.ts](../packages/oracle-parser/src/semantic/bindings.ts) performs
conservative reference analysis over all parsed faces and source metadata,
binding nested, optional, modal, chosen, repeated and divided instructions under
CR-cited scope rules; capture scopes, ambiguous antecedents and unsupported
repetition remain diagnostics. [keyword-scopes.ts](../packages/oracle-parser/src/semantic/keyword-scopes.ts)
maps each keyword to its CR 702 rule; [step-vocabulary.ts](../packages/oracle-parser/src/semantic/step-vocabulary.ts)
is the one list of structural steps. Frozen archives keep the event names they
were bound with.

Results have three levels:

- `referencesBound`: every reference resolves with no diagnostics.
- `engineFeatures`: constructs needing a rules-engine capability rather than a
  binding answer (`event-history`, `damage-prevention`, `characteristic-layers`);
  their references still bind normally.
- `fullyBound`: references bound and no engine feature needed.

None of these levels is semantic certification or execution support. The
`semantic-binding`, `binding-scope-rules`, `nested-instruction-scopes` and
`keyword-scopes` tests under `packages/oracle-parser/test/` own the cases. Wrong
binding is worse than an unresolved reference.

## Ingestion

These root scripts are independent of the deck-builder's ingestion and database.
`npm run ingest` requests the Scryfall bulk manifest with descriptive User-Agent
and Accept headers, selects `oracle_cards` and caches `.scryfall-cache/oracle-cards.json`.
A cache younger than 24 hours is reused only after complete validation. Downloads
stream to a unique adjacent temporary file and publish by rename after
validation; any failure keeps the prior corpus.

The reader supports gzip JSONL manifests and historical JSON arrays without
reserializing fields or numeric spellings; invalid UTF-8, truncated gzip or
multiple records per line reject.

Validation and archive require `.scryfall-cache/paper-printing-evidence.json`,
which ingestion does not create. Scope
`paper-printing-evidence-non-silver-non-acorn-v2` requires an eligible nonsilver,
nonacorn paper printing of each Oracle identity; a representative's `digital`
flag alone does not exclude it, and no Un-set is classified wholesale. Token,
double-faced token, emblem, art-series, planar, vanguard and scheme layouts are
excluded and counted separately; all other unsupported cards are failures. Empty
text is valid; missing/null face text fails. Legacy v1 scope misused the digital
flag and is not comparable.

Build paper evidence only from an explicitly downloaded, frozen `default_cards`
JSONL file and bulk manifest:

```sh
sh scripts/engine-env.sh node scripts/build-paper-evidence.ts default-cards.jsonl bulk-manifest.json .scryfall-cache/paper-printing-evidence.json
```

The builder refuses an existing output, records one eligible printing per
identity and hashes the complete download. Missing evidence is an error, never a
smaller-denominator fallback.

## Development and verification

Run commands from the repository root through `sh scripts/engine-env.sh` (pinned
[.node-version](../.node-version)); bootstrap only per the
[package manual](../packages/oracle-parser/AGENTS.md#focused-workflow). The gate
(TypeScript plus structural/negative Node tests, no network or bulk cache):

```sh
sh scripts/engine-env.sh npm --prefix packages/oracle-parser run check --workspaces=false
```

Corpus coverage is a separate check, run only when relevant:

```sh
sh scripts/engine-env.sh npm run validate
sh scripts/engine-env.sh npm run validate -- --json
sh scripts/engine-env.sh npm run archive-parser
```

Validation never refreshes inputs. Reports bind exact corpus, evidence, scope,
Git revision and parser source hash (including dirty changes). Compare only
matching, unchanged corpus/scope and review changed ASTs; reduced coverage may
correct false successes. Downloads, reports and snapshots stay ignored and
preserved. Before reusing a report or archive, compare `parser.sourceSha256` to
`sourceIdentity()` in `scripts/validate-parser.ts`, never a filename.

## Historical checkpoint

The 2026-10-16 checkpoint's 92.27% acceptance is historical, not execution
support; its pins are in
[the immutable baseline](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/oracle_parser.md#source-qualified-checkpoint-and-remaining-work).
Its audits remain under `.scryfall-cache/{checkpoint11-review.json,checkpoint11-grammar-gain-audit-coverage.json,archive-checkpoint11-independent-audit.json}`;
reused archives identify their checkpoint by manifest hashes, not path. Rust
complete-source lowering and admission establish execution, not a parser score.
