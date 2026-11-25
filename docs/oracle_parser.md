# Oracle text parser

The hand-written TypeScript parser at `packages/oracle-parser` owns normalization,
syntax AST and diagnostic reference analysis. Rust owns supported complete-source
binding/lowering and executable Card IR; see [system architecture](../README.md#system-architecture)
and [catalog admission](game-engine-catalogs.md). Broad grammar expansion is
paused; parsing and reference binding never certify engine execution. This guide owns parser semantics, data/archive
contracts and ingestion; [the package manual](../packages/oracle-parser/AGENTS.md)
owns local development constraints.

## Pipeline and public API

```text
Oracle text → normalize → ability blocks → lex → parse → typed AST
                                                       ↓
                                           reference binding analysis
```

`parseOracleText(text, cardName?, context?)` returns the discriminated
`ParseCardResult` in [ast.ts](../packages/oracle-parser/src/ast.ts). Successful
cards have aggregate `abilities`; failures have only per-block `lines`, retaining
successful blocks and diagnostics. `lines[].text` is normalized, not an original
source span. There is no separate JSON Schema; source types own the payload.

`ok: true` means every block was consumed and represented by supported productions.
It is not semantic certification: structural tests/rules review must catch wrong
ASTs. Unknown syntax never becomes raw executable text.

## Responsibilities and invariants

- Normalize punctuation, reminder text and exact-case self-references while
  preserving newlines; malformed parentheses remain errors. A complete-input
  `CardNameCatalog` supplies card, face and token names; grammar rejects unknown
  or ambiguous literal names.
- Lines ordinarily delimit abilities. Modal headers own all following bullet,
  Spree `+` or weighted `{P}` lines; consume every option or fail the whole block.
  An instruction-only self-replacement paragraph joins its preceding spell block
  (CR 614.15).
- Lexing never drops unknown characters. Parsers construct typed nodes directly,
  never `any` or lossy approximations; returning `null` restores cursor and
  elided-subject context through `Cursor.attempt`.
- Diagnostics come from the original classifier attempts; farthest-failure token
  locations are not original character offsets.

## Data contracts

The [Scryfall adapter](../scripts/lib/scryfall.ts) validates consumed fields while
preserving unrelated fields. [Card objects](https://scryfall.com/docs/api/cards)
and [bulk data](https://scryfall.com/docs/api/bulk-data) distinguish printing `id`
from Oracle `oracle_id`; reversible faces carry Oracle identities. Missing/null
Oracle text differs from present empty text. `oracle_cards` selects representative
printings, not a complete set/printing index.

Corpus calls pass each face's `typeLine` (so permanent text cannot pass as a
one-shot spell), layout and complete `cardNames` catalog. Binding inputs require
`includeSourceProvenance: true`; the validator and archive enable it. Public product/backend
contracts and generated gameplay SDK types remain separate from this AST.

### AST archive

`npm run archive-parser` reads the local cache and writes ignored
`generated/oracle-ast.jsonl`; `--input PATH --output PATH --paper-evidence PATH`
select alternatives (a frozen corpus and its evidence snapshot).
It never downloads or emits executable Card IR. [Archive code](../scripts/archive-parser.ts)
owns exact fields and serialization. The schema-4 manifest declares
`artifactKind: "oracle-ast-archive"`, `executable: false`, corpus byte/canonical-record
hashes, parser source hash/revision, rules edition, versioned scope, frozen paper
evidence identity and acceptance/exclusion/binding/resolution summaries. Source changes
during generation fail publication.

Adjacent content-addressed `corpus.snapshotFile` and `paperEvidence.snapshotFile`
preserve exact original bytes and download identity; decoded `source` is metadata,
not the raw-input authority. Existing snapshots are verified, never overwritten;
missing or corrupt inputs block publication. Scope exclusions are rechecked
against actual paper-printing witnesses, never names or format legality.

Each subsequent line retains one complete input record in original order:

- `source`, `sourceIndex`, `oracleIdentity`, `sourceSha256`: complete Scryfall object,
  every face/characteristic/unconsumed field and provenance.
- `excluded`: original source plus exclusion/duplicate reason, outside acceptance.
- `parsed`: all faces succeeded, full AST/normalized blocks and complete-card AST hash.
- `unsupported`: all available face results/diagnostics, no complete-card AST hash;
  missing/null text remains explicit `missing-oracle-text`, distinct from empty.

In-scope rows carry `resolution`/`resolutionSha256`, the authoritative contract
([resolution.ts](../packages/oracle-parser/src/semantic/resolution.ts)), per
parsed face and block, with block-relative paths:

- `refs`: every reference resolved to the closed `Ref` vocabulary shared with the
  kernel IR (source, controller by role, target slot, attached-to, event object or
  player by trigger, entering object, step result, bound variable, linked pool,
  linked result, announced X), a player derived from one, an operand evaluated in
  place, or `unresolved` with the exact reason. An event player is named by the kernel's
  `EventPlayerRole` where the kernel names that trigger's player
  (`active-player` for a beginning-of-step trigger, `caster`, `activating-player`),
  `declaring-player` for an attack or block declaration cost, and otherwise by its
  trigger's AST discriminant. A linked result names another block of the same face
  (`block`, block-relative `step`): an as-enters choice made by the face's only
  choice of that category, a spell's own additional-cost object, or an earlier
  paragraph of the same spell. References inside a granted or created ability never
  link; every other cross-block reference stays `unresolved`.
- `steps`: each printed step's event under its AST discriminant, or the scope of
  a [structural step](../packages/oracle-parser/src/semantic/step-vocabulary.ts);
  a step the binder did not account for is `unaccounted`.
- `spans`: a certificate tiling the normalized block text in UTF-8 byte offsets,
  naming the AST field each clause produced (folded riders and modal bullets
  included; bullet marks are the only trivia).
- `residuals`: why a block or card is incomplete; empty means complete.

Rows also retain the diagnostic `binding` graph it is derived from, which the
compiler no longer reads; `bindingSummary` counts
reference-bound and fully-bound cards, obligations and engine needs (see
[binding invariants](#semantic-and-binding-invariants)). Neither a parsed row, a
bound graph nor a complete resolution establishes executable support.

The compiler decodes these shapes through `mtg-oracle-ast`, generated from
`ast.ts` and `resolution.ts` by `scripts/generate-rust-ast.ts` (every union by an
explicit discriminant; an inseparable union fails generation). The engine live
guards run `node scripts/generate-rust-ast.ts --check`; regenerate after any
change to those sources.

Publication writes a unique adjacent temporary file, reads every record back and
verifies serialization and raw/source/AST/binding hashes before atomic
replacement. Any failure preserves the prior archive and removes only owned
temporary outputs.

### Executable output boundary

Rust contracts own Card IR schema 1, evolved in place with strict capability
and shape validation; archive schema 4 is independent. `mtg-sim schema` and
`catalog build MANIFEST [--write]` are the reviewed schema/catalog path;
`mtg-sim catalog build-ir ARCHIVE --out DIR` publishes the whole-archive bulk
IR, index and pinned manifest ([bulk admission](game-engine-catalogs.md#bulk-headless-admission)).
Per-set output would additionally need complete set membership and is not
append-only.
Generated `packages/card-data/sets/` is never hand-edited or committed; engine
catalogs remain small reviewed source fixtures.

## Semantic and binding invariants

The AST is experimental and changes without compatibility adapters. Source owns
the exact representation: [ast.ts](../packages/oracle-parser/src/ast.ts) defines
typed constructs (type/color conjunction versus alternatives, trigger grouping,
zones, durations, restrictions and self-replacements), and structural tests pin
each distinction. Unsupported wording fails rather than inventing scope.

The AST keeps symbolic references such as `it`, `that-player`, `x` and keyword
names. [bindings.ts](../packages/oracle-parser/src/semantic/bindings.ts) performs
conservative reference analysis over all parsed faces and source metadata,
recording sources, ability scopes, target selections, event results, reference
edges and unresolved obligations. [keyword-scopes.ts](../packages/oracle-parser/src/semantic/keyword-scopes.ts)
maps each produced keyword to its CR 702 rule, ability kinds and the player its
printed parameters refer to. [step-vocabulary.ts](../packages/oracle-parser/src/semantic/step-vocabulary.ts)
is the one list of structural AST steps (groups, nested abilities, iteration,
replacements, continuous effects); every other printed step is evented at its own
path under its AST `effect` discriminant. Frozen archives keep the event names
they were bound with (`library-scan:*`, `choose-one`). Nested, optional, modal, chosen and repeated
instructions bind under explicit CR-cited scope rules in that source; a bounded
counter division ("among one or two target creatures", never more targets than
counters) binds its target count and `target-division` at announcement (CR 601.2c-d); capture
scopes, ambiguous antecedents and unsupported repetition remain diagnostics.

Results have three levels:

- `referencesBound`: every reference resolves with no diagnostics.
- `engineFeatures`: constructs needing a rules-engine capability rather than a
  binding answer (`event-history`, `damage-prevention`, `characteristic-layers`);
  their references still bind normally.
- `fullyBound`: references bound and no engine feature needed.

Binding requires `includeSourceProvenance: true`. Like `ok: true`, none of these
levels is semantic certification or engine execution support. The
`semantic-binding`, `binding-scope-rules`, `nested-instruction-scopes` and
`keyword-scopes` tests under `packages/oracle-parser/test/` own the cases.

## Ingestion

These root scripts are independent of the deck-builder's ingestion and database.
`npm run ingest` requests the Scryfall bulk manifest with descriptive User-Agent
and Accept headers, selects `oracle_cards` and caches `.scryfall-cache/oracle-cards.json`.
A cache younger than 24 hours is reused only after complete validation. New
downloads stream to a unique adjacent temporary file and publish by rename after
validation; any failure retains the prior corpus.

The reader supports gzip JSONL manifests and historical JSON arrays without
reserializing fields or numeric spellings; invalid UTF-8, truncated gzip or
multiple records per line reject. Keep frozen benchmark inputs unchanged between
comparisons; downloads, reports and snapshots stay ignored and preserved.

Validation and archive require `.scryfall-cache/paper-printing-evidence.json`,
which ingestion does not create. Scope
`paper-printing-evidence-non-silver-non-acorn-v2` requires an eligible nonsilver,
nonacorn paper printing of each Oracle identity; a representative's `digital`
flag alone does not exclude it, and no Un-set is classified wholesale. Token,
double-faced token, emblem, art-series, planar, vanguard and scheme layouts are
excluded and counted separately; all other unsupported cards are failures. Empty
text is valid; missing/null face text fails.

Build paper evidence only from an explicitly downloaded, frozen `default_cards`
JSONL file and bulk manifest:

```sh
sh scripts/engine-env.sh node scripts/build-paper-evidence.ts default-cards.jsonl bulk-manifest.json .scryfall-cache/paper-printing-evidence.json
```

The builder refuses an existing output, records one eligible printing per
identity and hashes the complete download. Missing evidence is an error, never a
smaller-denominator fallback. Historical v1 scope misused the digital flag and
cannot supply comparable v2 percentages.

## Development and verification

Use the repository [.node-version](../.node-version) through
`sh scripts/engine-env.sh COMMAND`. Reuse healthy dependencies; when missing/broken,
follow [conditional engine/parser bootstrap](game-engine-development.md#fresh-worktree),
not root `npm install`. No Java or emitted parser build is required.

From repository root:

```sh
sh scripts/engine-env.sh npm --prefix packages/oracle-parser run check --workspaces=false
```

The gate runs TypeScript and structural/negative Node tests without network or
bulk cache. Fix false successes before extending coverage. Corpus coverage is a
separate check, run only when relevant:

```sh
sh scripts/engine-env.sh npm run validate
sh scripts/engine-env.sh npm run validate -- --json
sh scripts/engine-env.sh npm run archive-parser
```

Validation reads existing inputs without refreshing them. Reports bind exact
corpus, evidence, scope, Git revision and parser source hash (including dirty
changes). Compare only matching corpus/scope and review changed ASTs; reduced
coverage may correct false successes. Before reusing a report or archive, compare
`parser.sourceSha256` to `sourceIdentity()` in `scripts/validate-parser.ts`; never
infer current support from a filename such as `parser-98-*`.

## Source-qualified checkpoint and remaining work

Broad grammar work is paused. The 2026-10-16 checkpoint's 92.27% Commander-card
acceptance is historical, not current execution support. Exact parser source,
corpus/evidence pins, counts and earlier checkpoint 11 audit locators remain in
[the immutable baseline](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/oracle_parser.md#source-qualified-checkpoint-and-remaining-work)
and [original checkpoint](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/oracle-parser-checkpoint.md).
Unique audits remain under `.scryfall-cache/{checkpoint11-review.json,checkpoint11-grammar-gain-audit-coverage.json,archive-checkpoint11-independent-audit.json}`;
no source/evidence was removed. Reused archives identify their checkpoint through
manifest hashes, not their path.

Outstanding work includes independent review of later binding rules, unresolved
reference groups, engine-feature requirements, paused grammar and bulk output.
Wrong binding is worse than an unresolved reference. Rust complete-source
lowering and admission establish execution, not a parser score.
