# Scryfall Integration

This document describes how the MTG Ecosystem ingests card data from [Scryfall](https://scryfall.com/), the community-standard MTG card database.

---

## 📡 Data Source

Scryfall provides free [Bulk Data](https://scryfall.com/docs/api/bulk-data) exports that contain every card ever printed. We use these bulk exports rather than per-card API calls to avoid rate limiting and ensure we have a complete dataset.

### Bulk Data Endpoints

| Endpoint | Description | Use Case |
|----------|-------------|----------|
| **Oracle Cards** | One entry per unique card name (preferred oracle text) | ✅ Primary source — one canonical entry per card |
| **Unique Artwork** | One entry per unique artwork | Not needed for parsing |
| **Default Cards** | One entry per printing (includes reprints) | Useful for set-level IR output (maps cards to sets) |
| **All Cards** | Every card object including game variants | Too much noise for our needs |

**Primary workflow**: Download the **Default Cards** bulk export to get per-printing data, which lets us group cards by set for our per-set JSON IR output.

---

## 📥 Ingestion Pipeline

```
┌──────────────────────┐     ┌────────────────────┐     ┌──────────────────────┐
│  Scryfall Bulk API   │────▶│  Download & Cache   │────▶│  Normalize & Filter  │
│  (Default Cards)     │     │  (.scryfall-cache/) │     │  (Strip, Clean, Map) │
└──────────────────────┘     └────────────────────┘     └──────────┬───────────┘
                                                                   │
                                                                   ▼
                                                        ┌──────────────────────┐
                                                        │  Per-Card Objects    │
                                                        │  Ready for Parsing   │
                                                        └──────────────────────┘
```

### Step 1: Download Bulk Data
- Fetch the bulk data manifest from `https://api.scryfall.com/bulk-data`
- Download the `default_cards` JSON file (typically ~200MB)
- Cache locally in `.scryfall-cache/` (gitignored) to avoid re-downloading

### Step 2: Filter & Normalize
For each card object in the bulk data:

1. **Filter out irrelevant cards**:
   - Skip tokens, emblems, and art cards (no oracle text to parse)
   - Skip un-set cards and edge cases (out of scope for now)
   - Skip digital-only cards from Alchemy/Arena formats (optional, can be configurable)

2. **Normalize oracle text**:
   - **Strip reminder text**: Remove all parenthetical text — e.g., `"Flying (This creature can't be blocked...)"` → `"Flying"`
   - **Replace self-references**: The `~` character in Scryfall data represents the card's own name
   - **Normalize whitespace**: Collapse multiple spaces, trim line breaks

3. **Extract structured fields**:
   - `name`, `mana_cost`, `cmc`, `type_line`, `oracle_text`
   - `power`, `toughness`, `loyalty`, `defense`
   - `layout` (normal, transform, split, adventure, etc.)
   - `set`, `set_name`, `released_at`
   - For multi-face cards: extract each face's data separately

### Step 3: Group by Set
- Group normalized card objects by their `set` code
- Each group becomes one per-set JSON IR file after parsing

---

## 🗂️ Key Scryfall Fields

These are the Scryfall JSON fields we consume. See the [Scryfall Card Object docs](https://scryfall.com/docs/api/cards) for the full specification.

| Field | Type | Description | Used By |
|-------|------|-------------|---------|
| `id` | string | Scryfall UUID | Card IR identifier |
| `name` | string | Card name | Card IR, display |
| `oracle_text` | string | Rules text | **Parser input** |
| `mana_cost` | string | Cost string, e.g., `{2}{W}{U}` | Parsed into structured cost |
| `cmc` | number | Mana value | Card IR metadata |
| `type_line` | string | Full type line, e.g., `Legendary Creature — Elf Warrior` | Parsed into types/subtypes |
| `colors` | string[] | Card colors | Card IR metadata |
| `color_identity` | string[] | Color identity (for Commander) | Card IR metadata |
| `power` | string | Power (can be `*`) | Card IR metadata |
| `toughness` | string | Toughness (can be `*`) | Card IR metadata |
| `loyalty` | string | Planeswalker loyalty | Card IR metadata |
| `layout` | string | Card layout type | Multi-face handling |
| `card_faces` | object[] | Face data for multi-face cards | Multi-face parsing |
| `set` | string | Set code (e.g., `MH3`) | IR file grouping |
| `set_name` | string | Full set name | IR file metadata |
| `released_at` | string | Release date | IR file metadata |
| `keywords` | string[] | Keywords on the card | Cross-reference for parser validation |

---

## 🔄 Update Strategy

When a new MTG set is released:

1. **Re-download** the Scryfall bulk data (or just the new set's cards)
2. **Run the ingestion pipeline** to produce normalized card objects
3. **Run the parser** on the new cards — the existing ANTLR grammar should handle most new cards
4. **If new mechanics appear**: Update the `.g4` grammar, add visitor logic, add tests
5. **Emit a new per-set JSON file** (e.g., `ir/sets/NEW_SET.json`)
6. Existing set files are unchanged unless Scryfall issues errata

---

## ⚠️ Scryfall API Guidelines

Scryfall is a free service. Please follow their [API Guidelines](https://scryfall.com/docs/api):
- **Rate limit**: Maximum 10 requests per second for the REST API
- **Bulk data**: No rate limit on bulk downloads, but cache locally
- **Attribution**: Scryfall data is provided under their own terms — card images are © Wizards of the Coast
- **User-Agent**: Include a descriptive `User-Agent` header when making API requests
