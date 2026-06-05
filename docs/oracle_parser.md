# Oracle Text Parser — Layer 0 Design

This document covers the design of the Oracle Text Parser: the system that reads raw MTG card data (primarily oracle text) and compiles it into a structured, machine-readable intermediate representation (IR).

---

## 🎯 Goal

Take any MTG card's oracle text — e.g.:

> *"When Mulldrifter enters, draw two cards. Evoke {2}{U}"*

...and produce a typed AST that a rules engine (or an AI agent) can consume programmatically:

```typescript
const mulldrifter: CardIR = {
  name: "Mulldrifter",
  abilities: [
    {
      kind: "triggered",
      trigger: { type: "enters_battlefield", source: "self" },
      effect: { type: "draw_cards", player: "controller", count: 2 }
    },
    {
      kind: "keyword",
      keyword: "Evoke",
      cost: { generic: 2, blue: 1 }
    }
  ]
};
```

---

## 🏗️ Pipeline Overview

```
┌──────────────────┐     ┌──────────────────────────────────┐     ┌──────────────────┐
│  Raw Card Data   │────▶│  ANTLR-Generated Parser (TS)     │────▶│  Card IR (JSON)  │
│  (Scryfall JSON) │     │  MTGLexer.g4 → Lexer             │     │  Per-set files   │
│                  │     │  MTGParser.g4 → Parser → AST     │     │  ir/sets/MH3.json│
└──────────────────┘     │  Visitor → Typed AST Nodes       │     └──────────────────┘
                         └──────────────────────────────────┘
                                        │
                          ┌─────────────┘
                          ▼
                   ┌─────────────┐
                   │  .g4 Grammar │  ← The living spec of MTG's card language
                   │  (ANTLR)     │
                   └─────────────┘
```

### Stage 1: Ingest Raw Data
- Pull card data from **Scryfall bulk data** (the community-standard MTG API).
- Extract oracle text, type line, mana cost, power/toughness, and layout metadata.
- Normalize text: **strip reminder text** (parenthetical), handle split/transform card faces.

### Stage 2: Tokenize (ANTLR Lexer)
- The ANTLR lexer grammar (`MTGLexer.g4`) breaks oracle text into a typed token stream.
- Tokens include: keywords, numbers, mana symbols (`{W}`, `{2}`), card self-references (`~`), and semantic markers.
- Example: `"Destroy target creature."` → `[DESTROY, TARGET, CREATURE_TYPE, PERIOD]`

### Stage 3: Parse (ANTLR Parser → Visitor)
- The ANTLR parser grammar (`MTGParser.g4`) defines the structural rules for abilities, effects, costs, and conditions.
- ANTLR generates a parse tree in TypeScript.
- A custom **Visitor** walks the parse tree and produces strongly-typed AST nodes (TypeScript interfaces/discriminated unions).

### Stage 4: Emit IR (Per-Set JSON)
- The typed AST is serialized into versioned JSON files, **one file per set**.
- Stored in `ir/sets/<SET_CODE>.json` (e.g., `ir/sets/MH3.json`).
- This IR is what the rules engine (Layer 1) will eventually consume.
- A **SQLite index** can be added later for fast cross-set lookups.

---

## 🌳 AST Node Types (Draft Taxonomy)

This is a high-level sketch of the categories of AST nodes. The full grammar will be developed incrementally.

### Ability Types
| Node | Example Oracle Text |
|------|-------------------|
| `ActivatedAbility` | `"{T}: Add {G}."` |
| `TriggeredAbility` | `"When ~ enters, draw a card."` |
| `StaticAbility` | `"Other creatures you control get +1/+1."` |
| `KeywordAbility` | `"Flying"`, `"Trample"`, `"Evoke {2}{U}"` |
| `SpellEffect` | `"Destroy all creatures."` (on instants/sorceries) |

### Effect Types
| Node | Description |
|------|-------------|
| `DealDamage(amount, target)` | `"~ deals 3 damage to any target."` |
| `DrawCards(player, count)` | `"Draw two cards."` |
| `DestroyPermanent(target)` | `"Destroy target creature."` |
| `CreateToken(token_def, count)` | `"Create two 1/1 white Soldier tokens."` |
| `GainLife(player, amount)` | `"You gain 3 life."` |
| `ModifyPT(target, power, toughness)` | `"Target creature gets +3/+3 until end of turn."` |
| `Counter(target)` | `"Counter target spell."` |
| `ReturnToHand(target)` | `"Return target creature to its owner's hand."` |
| `Exile(target)` | `"Exile target permanent."` |
| `Search(zone, filter, action)` | `"Search your library for a basic land card..."` |

### Targeting & Selectors
| Node | Description |
|------|-------------|
| `Target(filter)` | `"target creature"`, `"target player"` |
| `Each(filter)` | `"each creature you control"` |
| `All(filter)` | `"all creatures"` |
| `Self` | `"~"` (self-reference) |
| `Controller` | `"you"` |
| `Opponent` | `"target opponent"` |

### Costs
| Node | Description |
|------|-------------|
| `ManaCost(symbols)` | `{2}{W}{U}` |
| `TapCost` | `{T}` |
| `SacrificeCost(filter)` | `"Sacrifice a creature"` |
| `DiscardCost(filter, count)` | `"Discard a card"` |
| `PayLifeCost(amount)` | `"Pay 2 life"` |

### Conditions & Modifiers
| Node | Description |
|------|-------------|
| `IfCondition(predicate)` | `"If you control a Dragon..."` |
| `Unless(condition)` | `"...unless its controller pays {2}"` |
| `UntilEndOfTurn` | Duration modifier |
| `AsLongAs(condition)` | `"...as long as you control an Island"` |

---

## 🛠️ Why ANTLR → TypeScript?

The parser uses **ANTLR** to define the grammar and generates a **TypeScript** parser runtime.

1. **Grammar IS Documentation**: The `.g4` grammar file is a formal, readable specification of MTG's card language. Anyone can read it to understand what the parser handles — no digging through code.
2. **Existing MTG Grammars**: Community projects like [mtg-grammar](https://github.com/Soothsilver/mtg-grammar) and [Demystify](https://github.com/Zannick/demystify) provide `.g4` grammars to build on.
3. **Battle-Tested**: ANTLR is used by Twitter, Hibernate, and Apache Spark for production language processing. Robust error recovery and mature tooling.
4. **TypeScript Runtime**: The officially-maintained ANTLR TypeScript target gives us:
   - Full npm ecosystem access
   - Easy browser/WASM deployment for future web clients
   - TypeScript's discriminated unions for typed AST nodes
   - Natural JSON serialization for the IR output
5. **Layered Build Path**: Since the upper layers (game state engine, game server, web clients) will also be TypeScript, the parser integrates seamlessly — no cross-language FFI.
6. **IDE Tooling**: ANTLR has IDE plugins for grammar development, railroad diagram visualization, and grammar debugging.

### Future: Python Bindings
Python bindings are planned as a future addition for AI/ML workflows. Options include:
- ANTLR's official Python 3 target (regenerate parser from the same `.g4` grammar)
- WASM compilation of the TypeScript parser for use in Python via Pyodide
- A thin HTTP/CLI wrapper around the TypeScript parser

---

## 📦 Output: Per-Set JSON IR

The compiled card IR is stored as **one JSON file per set** in `ir/sets/`.

Design principles:
- **Self-describing**: Each IR file includes its schema version and set metadata.
- **Deterministic**: The same oracle text always produces the same IR.
- **Diffable**: Git diffs show exactly which cards in which sets changed.
- **Additive**: New sets are added by dropping a new file — no existing files are modified.

### File Layout
```
ir/
├── sets/
│   ├── LEA.json       # Limited Edition Alpha
│   ├── MH3.json       # Modern Horizons 3
│   ├── DSK.json       # Duskmourn
│   └── ...
└── (future: index.db) # Optional SQLite index for cross-set queries
```

### Example: `ir/sets/LEA.json` (excerpt)

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
    }
  ]
}
```

---

## 🧪 Testing Strategy

- **Snapshot Tests**: Parse a curated corpus of cards and snapshot the AST output. Regressions are caught instantly.
- **Coverage by Mechanic**: Track which keywords, abilities, and templated patterns are covered.
- **Round-Trip Tests**: `oracle_text → AST → reconstructed_text` should be semantically equivalent.
- **Scryfall Bulk Validation**: Run the parser against the entire Scryfall database and report coverage stats (% of cards parsed without errors).

---

## ✅ Resolved Decisions

| Question | Decision |
|----------|----------|
| **Parser Tool** | **ANTLR → TypeScript.** Grammar defined in `.g4` files (grammar-as-documentation), parser generated in TypeScript (ecosystem compatibility, web deployment). |
| **IR Storage** | **Per-set JSON files** in `ir/sets/<SET_CODE>.json`. SQLite index left as a future option for fast cross-set queries. |
| **Data Source** | **Scryfall** bulk data is the source of truth for all card data. |
| **Reminder Text** | **Strip before parsing.** The system should have its own understanding of what keywords and rules mean — reminder text is for humans reading physical cards. |
| **Keyword Representation** | **Atomic.** Keywords like `Flying` are stored as `KeywordAbility(Flying)`. The rules engine (Layer 1) will know what each keyword means. |
| **Edge-Case Cards** | **Out of scope for now.** Cards with truly unique mechanics (e.g., *Shahrazad*, un-set cards) are excluded from the initial parser. |
| **Python Bindings** | **Future addition.** Not needed for Milestone 1. Documented as a planned extension for AI/ML workflows. |

For the full decision rationale with pros/cons analysis, see **[decisions.md](decisions.md)**.
