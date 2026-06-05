# Data Schemas & Layouts

This document describes the data formats used by the MTG Ecosystem. It covers the **input** data consumed from Scryfall, the **output** Card IR produced by the Oracle Text Parser (Layer 0), and draft schemas for future layers.

---

## 1. Scryfall Input Format

The Oracle Text Parser ingests raw card data from [Scryfall's bulk data exports](https://scryfall.com/docs/api/bulk-data). Scryfall is the community-standard source of truth for MTG card data.

### Key Fields Used

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Scryfall's unique UUID for this printing |
| `name` | `string` | Card name (both faces joined by `//` for DFCs) |
| `oracle_text` | `string` | Rules text (the primary input to the parser) |
| `mana_cost` | `string` | Mana cost in brace notation, e.g. `{2}{W}{U}` |
| `cmc` | `number` | Converted mana cost / mana value |
| `type_line` | `string` | Full type line, e.g. `Creature — Elemental` |
| `colors` | `array` of `string` | Colors of the card (`W`, `U`, `B`, `R`, `G`) |
| `color_identity` | `array` of `string` | Color identity for Commander legality |
| `power` | `string` | Power (may be `*` or similar) |
| `toughness` | `string` | Toughness (may be `*` or similar) |
| `loyalty` | `string` | Starting loyalty for planeswalkers |
| `layout` | `string` | Card layout (`normal`, `split`, `transform`, `adventure`, etc.) |
| `card_faces` | `array` of `object` | Face data for multi-face cards |
| `set` | `string` | Set code (e.g., `lea`, `mh3`) |
| `set_name` | `string` | Full set name |
| `released_at` | `string` | Release date (ISO 8601) |
| `keywords` | `array` of `string` | Scryfall's extracted keyword list |

### Example: Raw Scryfall Card Object (Abridged)

```json
{
  "id": "e3285e6b-3e79-4d7c-bf96-d920f973b122",
  "name": "Lightning Bolt",
  "mana_cost": "{R}",
  "cmc": 1.0,
  "type_line": "Instant",
  "oracle_text": "Lightning Bolt deals 3 damage to any target.",
  "colors": ["R"],
  "color_identity": ["R"],
  "keywords": [],
  "layout": "normal",
  "set": "lea",
  "set_name": "Limited Edition Alpha",
  "released_at": "1993-08-05"
}
```

> [!NOTE]
> Reminder text (parenthetical explanations) is **stripped before parsing**. The parser should understand mechanics from its grammar, not from reminder text meant for human players.

---

## 2. Card IR Output Format

The Oracle Text Parser produces a **Card IR** (Intermediate Representation) — one JSON file per set, stored in `ir/sets/<SET_CODE>.json`. This is the primary output of Layer 0 and the input for the Rules Engine (Layer 1).

### IR File Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "SetIR",
  "type": "object",
  "properties": {
    "schema_version": { "type": "string", "description": "Semantic version of the IR schema" },
    "set_code": { "type": "string", "description": "Uppercase set code, e.g. LEA" },
    "set_name": { "type": "string", "description": "Full set name" },
    "released_at": { "type": "string", "description": "Release date (ISO 8601)" },
    "cards": {
      "type": "array",
      "items": { "$ref": "#/definitions/CardIR" }
    }
  },
  "required": ["schema_version", "set_code", "set_name", "cards"]
}
```

### CardIR Object Schema

```json
{
  "title": "CardIR",
  "type": "object",
  "properties": {
    "card_id": { "type": "string", "description": "Prefixed Scryfall ID, e.g. scryfall:<uuid>" },
    "name": { "type": "string" },
    "mana_cost": {
      "type": "object",
      "description": "Structured mana cost",
      "properties": {
        "generic": { "type": "integer" },
        "white": { "type": "integer" },
        "blue": { "type": "integer" },
        "black": { "type": "integer" },
        "red": { "type": "integer" },
        "green": { "type": "integer" },
        "colorless": { "type": "integer" }
      }
    },
    "cmc": { "type": "number" },
    "types": { "type": "array", "items": { "type": "string" } },
    "subtypes": { "type": "array", "items": { "type": "string" } },
    "supertypes": { "type": "array", "items": { "type": "string" } },
    "power": { "type": "string" },
    "toughness": { "type": "string" },
    "loyalty": { "type": "string" },
    "abilities": {
      "type": "array",
      "items": { "$ref": "#/definitions/Ability" },
      "description": "Parsed abilities — the core parser output"
    }
  },
  "required": ["card_id", "name", "types", "abilities"]
}
```

### Ability Object Schema

Each ability has a `kind` discriminator:

| `kind` | Description | Key Fields |
|--------|-------------|------------|
| `keyword` | Atomic keyword ability | `keyword`, `cost` (optional) |
| `activated` | Activated ability (`cost: effect`) | `cost`, `effects` |
| `triggered` | Triggered ability (`when/whenever/at`) | `trigger`, `effects` |
| `static` | Static/continuous ability | `effects`, `conditions` |
| `spell_effect` | One-shot effect on instants/sorceries | `effects` |

### Example: `ir/sets/LEA.json` (Excerpt)

```json
{
  "schema_version": "0.1.0",
  "set_code": "LEA",
  "set_name": "Limited Edition Alpha",
  "released_at": "1993-08-05",
  "cards": [
    {
      "card_id": "scryfall:e3285e6b-3e79-4d7c-bf96-d920f973b122",
      "name": "Lightning Bolt",
      "mana_cost": { "generic": 0, "red": 1 },
      "cmc": 1,
      "types": ["Instant"],
      "abilities": [
        {
          "kind": "spell_effect",
          "effects": [
            {
              "effect_type": "deal_damage",
              "amount": 3,
              "target": {
                "selector": "target",
                "filter": { "any_of": ["creature", "player", "planeswalker"] }
              }
            }
          ]
        }
      ]
    },
    {
      "card_id": "scryfall:fb4b6cdb-9020-4a7d-a583-8819e8d1d0e8",
      "name": "Serra Angel",
      "mana_cost": { "generic": 3, "white": 2 },
      "cmc": 5,
      "types": ["Creature"],
      "subtypes": ["Angel"],
      "power": "4",
      "toughness": "4",
      "abilities": [
        { "kind": "keyword", "keyword": "Flying" },
        { "kind": "keyword", "keyword": "Vigilance" }
      ]
    }
  ]
}
```

> [!IMPORTANT]
> The Card IR is the **contract between Layer 0 and Layer 1**. All downstream consumers (rules engine, agents, clients) read this format. Changes to the IR schema must be versioned via `schema_version`.


