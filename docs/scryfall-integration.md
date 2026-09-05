# Scryfall integration for oracle-parser

The parser uses Scryfall's `oracle_cards` bulk export. The deck-builder has its
own ingestion pipeline and database; this document describes the root parser
scripts only.

## Input and identity

[Scryfall's bulk documentation](https://scryfall.com/docs/api/bulk-data) defines
Oracle Cards as one object for each Oracle ID. The selected object is a
representative printing; it is not a complete map of all printings to sets.
A future per-set emitter needs printing membership data, such as Default Cards,
in addition to Oracle definitions.

[Card Object fields](https://scryfall.com/docs/api/cards) distinguish `id`
(printing) from `oracle_id` (Oracle identity). Reversible cards carry Oracle IDs
on their faces. The shared adapter in `scripts/lib/scryfall.ts` validates the
fields used by ingestion and reporting while allowing unrelated fields.

## Download and cache

Run `npm run ingest` at the root. It requests the bulk manifest, selects
`oracle_cards`, and caches `.scryfall-cache/oracle-cards.json`.

- A cache younger than 24 hours is reused only after parsing and validating it.
- A replacement streams into a unique temporary file beside the cache.
- Stream/HTTP failures and invalid JSON or card records abort the replacement.
- A complete validated download replaces the cache with a same-directory rename.
- Temporary files are removed after success or failure; the previous corpus
  remains available if a replacement fails.

Requests include descriptive User-Agent and Accept headers. External formats
were checked against the linked Scryfall documentation on 2026-09-05.
Downloaded bulk files, temporary files, and generated reports are ignored.

## Evaluation

`npm run validate` reads the existing cache; it does not fetch newer data.
`npm run validate -- --json` emits a reproducible machine-readable report with
corpus hash, parser revision/source hash, exclusions, full failure examples,
and per-card acceptance/AST hashes.

The scope excludes digital-only cards, silver borders, acorn stamps, and token,
double-faced token, emblem, art-series, planar, vanguard, and scheme layouts.
Other unsupported mechanics count as failures. This scope intentionally does
not equate an entire Un-set with silver/acorn status. The scope identifier is
versioned so denominator changes are visible.

Faces are evaluated separately. Empty text is valid; missing or null face text is a
reported failure. Modal headers and bullets count as one ability block. Text
normalization preserves boundaries and introduces `~` for self-references;
Scryfall does not supply that placeholder.

A report measures acceptance, not correctness. Run the parser's `npm run check`
for structural and negative regression tests. Compare corpus hashes and scopes
before interpreting changes in percentages. For the AST/IR boundary, see
[oracle_parser.md](oracle_parser.md) and [data_schemas.md](data_schemas.md).
