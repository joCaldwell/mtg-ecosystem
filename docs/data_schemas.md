# Parser data contracts and proposed Card IR

## Implemented input

The parser entry point accepts Oracle text, an optional card name, and optional
context: `parseOracleText(text, cardName?, context?)`. Context carries layout,
type line, and `includeSourceProvenance` for stable target identities. Bulk
ingestion uses Scryfall's `oracle_cards`
export. The reporting adapter validates the fields it consumes before use:
`id`, `oracle_id`, `name`, `layout`, `oracle_text`, `card_faces`, `digital`,
`border_color`, and optional `security_stamp`.

`id` identifies a printing; `oracle_id` identifies Oracle identity across
reprints. Reversible cards carry Oracle IDs on their faces. Card and face text
may be absent or null, which is distinct from a present empty string.

These field meanings were checked against the official
[Card Object documentation](https://scryfall.com/docs/api/cards) and
[Bulk Data documentation](https://scryfall.com/docs/api/bulk-data).
See [scryfall-integration.md](scryfall-integration.md) for cache handling.

## Implemented output: TypeScript AST

The authoritative types are in
[ast.ts](../packages/oracle-parser/src/ast.ts). There is no separate implemented
JSON Schema, and no generated per-set Card IR currently exists.

`ParseCardResult` is a discriminated union:

- Success: `{ name, ok: true, lines, abilities }`.
- Failure: `{ name, ok: false, lines }`, with no aggregate abilities.

Each line is either `{ text, ok: true, ability }` or
`{ text, ok: false, error }`. Text is normalized; modal blocks retain newlines.

| Ability `kind` | Required payload |
| --- | --- |
| `keywords` | `keywords: KeywordInstance[]` |
| `activated` | `costs: Cost[]`, `effects: Sentence[]` |
| `loyalty` | `cost: { sign, amount }`, `effects: Sentence[]` |
| `triggered` | `trigger: Trigger`, `effects: Sentence[]` |
| `static` | `effect: StaticEffect` |
| `spell` | `effects: Sentence[]` |
| `additional-cost` | `costs: Cost[]` |

Activated and loyalty abilities can carry typed activation restrictions.
Sentences contain ordered effects, with optional conditions and otherwise
branches. Filter logic, event grouping, zone ownership, and duration scope are
specified in [oracle_parser.md](oracle_parser.md#semantic-conventions).

For example, parsing `Flying` produces:

```json
{
  "name": "",
  "ok": true,
  "lines": [{
    "text": "Flying",
    "ok": true,
    "ability": { "kind": "keywords", "keywords": [{ "keyword": "flying" }] }
  }],
  "abilities": [{ "kind": "keywords", "keywords": [{ "keyword": "flying" }] }]
}
```

The AST is experimental and intentionally permits breaking changes. Success is
supported parsing, not engine capability certification. Symbolic references and
atomic keywords still require semantic interpretation by future consumers.

## Implemented storage: nonexecutable AST archive

`npm run archive-parser` writes `generated/oracle-ast.jsonl` from the current
cached corpus. Optional `--input <path>` and `--output <path>` arguments select
other local files. It does not download data. The generated archive is ignored
by Git and is not executable Card IR.

The first JSON line is a manifest with `artifactKind: "oracle-ast-archive"`,
`schemaVersion: 3`, and `executable: false`. It records the exact corpus byte
hash, canonical source-record hash, parser source hash and Git revision, rules
edition, versioned acceptance scope, frozen paper-printing evidence identity,
and the same card/block acceptance and
exclusion counts used by validation. Generation fails if parser sources change
between starting the parse and finishing the verified write.

`corpus.snapshotFile` names an adjacent, content-addressed raw source snapshot.
Its bytes and SHA-256 exactly match the input corpus, preserving whitespace,
JSON number spellings, and future fields independently of JavaScript's numeric
representation. An existing snapshot is verified and never overwritten; a
corrupt snapshot prevents publication of a replacement archive. The decoded
`source` objects below are convenient metadata, while this snapshot is the
authoritative exact original input.

`paperEvidence.snapshotFile` names a second content-addressed snapshot. It
preserves the exact evidence bytes, their size and SHA-256, and the originating
complete printing download's hash, size, record count, URL, and update time.
Each witness is an actual paper printing of one Oracle identity; names and
format legality never establish eligibility. A digital Oracle representative
can therefore remain intact in `source` while its paper identity is included.
Archive verification rechecks every exclusion against the stored evidence.
Missing, changed, or corrupt evidence prevents publication.

Every following JSON line stores one input record, in original input order:

- `source` is the complete original Scryfall object, including all faces,
  characteristics, metadata, and fields the parser does not consume.
  `sourceIndex`, `oracleIdentity`, and `sourceSha256` retain its provenance.
- `status: "excluded"` stores the scope exclusion or duplicate reason while
  retaining the complete source. Excluded cards never inflate acceptance.
- `status: "parsed"` contains face results with the full structured AST and
  a complete-card AST hash. Every face must have parsed successfully.
- `status: "unsupported"` contains all available per-face results and
  diagnostics. Missing/null face text has explicit `missing-oracle-text`
  status, and there is no complete-card AST hash. The source preserves the
  distinction between missing, null, and empty Oracle text.

Face results retain normalized ability blocks as well as the structured nodes;
targets include parser-minted selection IDs, preserving repeated use of one
target separately from independently stated targets with identical filters.
The original Oracle text remains in `source`. Atomic keyword names and symbolic
references remain symbolic. A parsed archive record is an acceptance result,
not a claim that its mechanics are executable or independently certified.

Every in-scope record also stores `binding` and `bindingSha256`: the separate
reference graph, runtime slots, resolved edges, and unresolved diagnostics.
The manifest's `bindingSummary` counts analyzed cards, cards with all inspected
references bound, reference obligations, and diagnostics. These metrics are
separate from syntax acceptance. Missing target provenance, ambiguous
antecedents, or unsupported scopes cannot become a fully bound result.
Reference completeness is not full semantic validation or engine support.

The writer publishes complete source and evidence snapshots first and uses a unique
temporary file beside the archive destination. Before atomic
replacement it reads every JSON line back, checks exact serialization, and
recomputes raw-snapshot, source-record, successful-AST, and binding hashes. Invalid input, non-finite numbers,
source edits, or verification failures preserve the previous archive. JSONL
allows verification to stream rather than load a second bulk corpus in memory.

## Proposed output: versioned Card IR

The semantic compiler and emitter use **Rust**, as fixed by
the [language decision](runtime-language-decision.md). The source parser/archive
stays TypeScript; Card IR is language-neutral JSON. The initial scenario schema
exists; broader schema and emission design remain implementation work.

The initial scenario catalog is implemented by Rust types in
`packages/game-engine/crates/contracts/`; `mtg-sim schema` exports its generated
JSON Schema and `mtg-sim compile` emits the four-definition fixture catalog.
The [first scenario brief](game-engine-first-scenario.md) defines this narrow
contract and its source adapter. General/per-set bulk emission remains a
**proposal**; `scripts/build-ir.ts` fails explicitly. The previous schema
examples used retired discriminator names and unresolved JSON Schema references;
they were not an implemented contract and have been removed.

The [Layer 1 contract proposal](game-engine-spec.md#3-card-compiler-and-executable-contract)
now specifies compilation stages, executable admission, capability profiles,
source provenance, and strict cross-language interchange. It proposes canonical
Oracle definitions with separate printing/set membership indexes, refining the
earlier per-set packaging direction below. The first scenario uses canonical
Oracle definitions and preserves representative printing identity in provenance;
complete printing/set membership indexes and bulk packaging remain unfinished.

The proposed location is `packages/card-data/sets/<set-code>.json`, containing
versioned set metadata and cards. Before implementing it, decide and test:

1. Separate Oracle identity from printing identity and per-set membership.
   `oracle_cards` contains representative printings, not a complete set index;
   per-set emission requires an additional printing/membership source.
2. Preserve faces, layouts, variable and hybrid mana, defense, and other card
   characteristics without lossy numeric shortcuts.
3. Define binding and semantic validation. Unresolved references and unsupported
   constructs must prevent an executable card from being emitted.
4. Specify schema version, parser source identity, rules version, and corpus
   identity so an artifact can be reproduced.
5. Define stable ordering, validation, update policy, and cross-set reprints.
   Oracle corrections may require rebuilding existing sets; output is not
   inherently append-only.

When implemented, derive or validate the machine-readable schema against the
actual types and test serialization. Generated files stay ignored; small,
curated structural fixtures belong in tests. No downstream layer should build
against the retired example schema.
