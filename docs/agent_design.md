# Agent-First Design & AI Integration

> [!NOTE]
> This is a **vision document** spanning all layers of the MTG Ecosystem (Layers 0–3). Layer 0 sections describe what is available today (or soon). Layer 1+ sections describe planned future capabilities and are included for architectural planning.

One of Project Multiverse's core tenets is that **artificial intelligence is a first-class participant**. Whether it's an LLM explaining rules to a beginner, a reinforcement learning (RL) agent playtesting a new set, or a deck-building assistant recommending synergies, the system must expose game data in formats optimized for AI consumption.

---

## 🔮 Layer 0: Agent Interaction with Card IR

The Oracle Text Parser (Layer 0) produces structured Card IR that agents can consume immediately — no NLP or text parsing required on the agent's side.

### What's Available

The parser outputs **per-set JSON files** in `ir/sets/<SET_CODE>.json`. Each file contains every card in that set with fully parsed abilities. See [data_schemas.md](data_schemas.md) for the full schema and [oracle_parser.md](oracle_parser.md) for the parser design.

### How Agents Consume Card IR

**1. Query parsed abilities by kind**

Filter cards by ability type (`keyword`, `triggered`, `activated`, `static`, `spell_effect`) without needing to parse oracle text:

```typescript
// Find all cards in a set with triggered abilities
const set = JSON.parse(fs.readFileSync("ir/sets/MH3.json", "utf-8"));
const triggeredCards = set.cards.filter((card) =>
  card.abilities.some((a) => a.kind === "triggered")
);
```

**2. Search and filter by atomic keywords**

Keywords are stored as atomic values (e.g., `KeywordAbility("Flying")`), making exact-match queries trivial:

```typescript
// Find all creatures with Flying in a set
const flyers = set.cards.filter((card) =>
  card.types.includes("Creature") &&
  card.abilities.some((a) => a.kind === "keyword" && a.keyword === "Flying")
);
```

**3. Cross-set analysis**

Since each set is a separate file, agents can load multiple sets and perform cross-set queries:

```typescript
// Load all sets and find every card that deals damage
const setFiles = glob.sync("ir/sets/*.json");
const allCards = setFiles.flatMap((f) => JSON.parse(fs.readFileSync(f, "utf-8")).cards);
const damageCards = allCards.filter((card) =>
  card.abilities.some((a) =>
    a.effects?.some((e) => e.effect_type === "deal_damage")
  )
);
```

**4. Structured data for embeddings and vector search**

The parsed IR provides clean, structured fields that are far more useful for generating embeddings than raw oracle text. Agents can embed ability structures, type lines, and mana costs independently.

---

## 🤖 AI User Personas & Core Use Cases

```
                    ┌────────────────────────┐
                    │  MTG Ecosystem Layers  │
                    └──────────┬─────────────┘
                               │
       ┌───────────────────────┼───────────────────────┐
       ▼                       ▼                       ▼
┌──────────────┐        ┌──────────────┐        ┌──────────────┐
│ Game Player  │        │ Rules Helper │        │ Deck Builder │
│  (RL / LLM)  │        │   (LLM/RAG)  │        │  (LLM/Embed) │
│ • Play games │        │ • Explain UI │        │ • Synergies  │
│ • Simulation │        │ • Rule logs  │        │ • Legality   │
└──────────────┘        └──────────────┘        └──────────────┘
```

### Layer 1–2: Game-Playing Agent (RL or LLM-driven)
- **Goal**: Play matches, evaluate board positions, and simulate matchups.
- **System Requirements**: 
  - A comprehensive, fast, and structured state representation.
  - A deterministic, enumerable list of valid actions (action space).
  - High-throughput simulation capabilities (headless game engine).

### Layer 2: Rules Advisor Agent (LLM-driven Chatbot)
- **Goal**: Answer players' questions during a game (e.g., *"Why did my creature die when I cast this?"* or *"Can I respond to my opponent's trigger?"*).
- **System Requirements**:
  - Readable rules engine step-logs (execution traces).
  - Explicit linking of cards to the specific sections of the MTG Comprehensive Rules they interact with.
  - Explanability API that outputs rule-evaluation steps (e.g., Layer evaluations, SBA results).

### Layer 0–1: Deck-Building Assistant (LLM & Vector Search)
- **Goal**: Guide players in constructing decks, finding synergies, and ensuring format legality.
- **System Requirements**:
  - Embedded representations of card mechanics and text (vector embeddings).
  - Structural metadata for card tags (e.g., *Ramp, Removal, Win-Condition, Sac-Outlet*).

---

## 🛠️ Key Architectural Integrations for Agents

### Layer 1–2: Semantic Action Space Representation
Instead of an agent needing to parse unstructured screen states to determine legal moves, the API exposes a structured JSON action space:

```json
{
  "active_player_id": "player_1",
  "priority_player_id": "player_1",
  "legal_actions": [
    {
      "action_type": "CAST_SPELL",
      "card_instance_id": "instance_9823",
      "card_name": "Lightning Bolt",
      "cost": { "R": 1 },
      "targets_required": [
        {
          "type": "CREATURE_OR_PLAYER",
          "valid_target_ids": ["instance_1102", "player_2"]
        }
      ]
    },
    {
      "action_type": "PASS_PRIORITY"
    }
  ]
}
```

### Layer 1–2: Explainable Rules Engine (Traceability)
The Rules Engine will output a detailed log/trace of rule execution. For example, when evaluating continuous effects:

```json
{
  "card_evaluated": "Magus of the Moon (instance_554)",
  "layer": 4,
  "effect_applied": "Nonbasic lands are Mountains.",
  "affected_cards": [
    {
      "card_instance_id": "instance_772",
      "name": "Stomping Ground",
      "original_types": ["Land", "Mountain", "Forest"],
      "new_types": ["Land", "Mountain"]
    }
  ]
}
```
An LLM Rules Helper can consume this JSON trace and immediately construct a natural language explanation: *"Stomping Ground is now just a Mountain because Magus of the Moon applies in Layer 4 (type-changing effects), stripping it of its other types."*

### Layer 0–1: Unified Card Embeddings & Tags
To help LLMs identify synergies, every card in Layer 0 includes:
- **Oracle Text**: Clean text representations.
- **Semantic Tagging**: Hand-curated and AI-generated labels indicating role (e.g., `#removal`, `#board-wipe`, `#card-draw`, `#tribal-elf`).
- **Vector Embeddings**: Pre-computed semantic vectors for cards, enabling vector-search synergy engines (e.g., querying for *"cards that synergize with graveyard discard and draw"*).
