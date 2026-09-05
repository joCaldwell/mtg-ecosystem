# Parser data contracts and proposed Card IR

## Implemented input

The parser entry point accepts Oracle text and an optional card name:
`parseOracleText(text, cardName?)`. Bulk ingestion uses Scryfall's `oracle_cards`
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

## Proposed output: versioned Card IR

**Proposal only.** `scripts/build-ir.ts` fails explicitly. The previous schema
examples used retired discriminator names and unresolved JSON Schema references;
they were not an implemented contract and have been removed.

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
