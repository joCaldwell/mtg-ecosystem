# Glossary — MTG & System Terminology

A reference mapping Magic: The Gathering terminology to their system-level equivalents in the MTG Ecosystem. If you're new to MTG or to this project, start here.

> [!TIP]
> Terms marked with **(Layer 0)** are relevant to the current milestone. Terms marked **(Layer 1+)** describe concepts that will be implemented in future milestones but are important for understanding the full design.

---

## Game Concepts

| Term | Category | Description |
|------|----------|-------------|
| **Oracle Text** | Card Data | The official, canonical rules text for a card, maintained and updated by Wizards of the Coast. Oracle text supersedes whatever is physically printed on a card. This is the primary input the parser consumes. |
| **Mana Cost** | Card Data | The cost required to cast a spell, expressed as mana symbols (e.g., `{2}{W}{U}`). Appears in the top-right corner of a card. |
| **Converted Mana Cost (CMC) / Mana Value** | Card Data | The total generic mana equivalent of a card's mana cost. For example, `{2}{W}{U}` has a mana value of 4. "CMC" is the legacy term; "mana value" is the current official name. Both appear in community usage. |
| **Color Identity** | Card Data | The complete set of colors present in a card's mana cost *plus* any colored mana symbols appearing in its rules text. Primarily used for Commander format deck-building legality. |
| **Types** | Card Data | The primary classification of a card. Card types include: Creature, Instant, Sorcery, Enchantment, Artifact, Land, Planeswalker, and Battle. A card can have multiple types (e.g., "Artifact Creature"). |
| **Supertypes** | Card Data | Optional qualifiers that appear before a card's type. Include: Legendary, Basic, Snow, World. Supertypes carry rules meaning (e.g., the Legend Rule, basic land fetch interactions). |
| **Subtypes** | Card Data | Type-specific classifications that appear after the type line dash. Examples: creature subtypes (Elf, Warrior, Dragon), enchantment subtypes (Aura, Saga), land subtypes (Forest, Island), spell subtypes (Arcane, Adventure). |
| **Zones** | Game State | Distinct regions where cards and objects can exist during a game. The seven zones are: **Library** (deck), **Hand**, **Battlefield** (in play), **Graveyard** (discard pile), **Stack** (spells/abilities being cast), **Exile** (removed from game), and **Command** (commanders, emblems). |
| **The Stack** | Game State | A LIFO (last-in, first-out) data structure where spells and abilities are placed when cast or activated. Objects on the stack resolve in reverse order. Players receive priority between each resolution. **(Layer 1+)** |
| **Priority** | Game State | The permission system that determines which player may act at any given moment. A player with priority can cast spells, activate abilities, or take special actions. Priority passes between players, and spells/abilities resolve only when all players pass priority in succession. **(Layer 1+)** |
| **State-Based Actions (SBAs)** | Rules | Game rules that are checked and enforced continuously (technically, whenever a player would receive priority). Examples: a creature with 0 or less toughness is destroyed, a player at 0 or less life loses the game, a player who draws from an empty library loses. SBAs don't use the stack. **(Layer 1+)** |
| **Triggered Abilities** | Abilities | Abilities that fire automatically when a specified condition or event occurs. Identified by the words **"When..."**, **"Whenever..."**, or **"At..."** in oracle text. Example: *"When ~ enters, draw a card."* |
| **Activated Abilities** | Abilities | Abilities that a player chooses to use by paying a cost. Written in **"Cost: Effect"** format, with a colon separating the cost from the effect. Example: *"{T}: Add {G}."* |
| **Static Abilities** | Abilities | Abilities that are continuously active as long as the card is in the appropriate zone (usually the battlefield). They don't trigger or activate — they simply apply. Example: *"Other creatures you control get +1/+1."* |
| **Keyword Abilities** | Abilities | Named shorthand for commonly-used abilities defined in the Comprehensive Rules. Examples: Flying, Trample, Lifelink, Deathtouch, Haste, Vigilance. In this system, keywords are represented atomically as `KeywordAbility(Flying)` — the rules engine (Layer 1) interprets their mechanical meaning. |
| **Continuous Effects & Layer System (Rule 613)** | Rules | The system for evaluating overlapping modifications to game objects. When multiple effects modify the same card, they are applied in a strict layer order (7 layers, from copy effects through power/toughness modifications). Critical for deterministic game state evaluation. **(Layer 1+)** |
| **APNAP Order** | Rules | **A**ctive **P**layer, **N**on-**A**ctive **P**layer — the tiebreaking order used when multiple players must make simultaneous choices or when multiple triggered abilities need to be ordered on the stack. **(Layer 1+)** |
| **Replacement Effects** | Rules | Effects that replace a game event with a different outcome. Identified by **"If [event] would happen, [replacement] instead"** phrasing. They are *not* triggered abilities — they modify the event before it occurs, rather than responding after. Example: *"If a creature you control would die, exile it instead."* |
| **Reminder Text** | Card Data | Parenthetical, italicized text on cards that explains what a keyword or mechanic does. It carries no rules weight and is **stripped before parsing** in this system. Example: *"Flying *(This creature can't be blocked except by creatures with flying or reach.)*"* — the parenthetical is removed. |
| **Self-Reference (~)** | Card Data | In oracle text, the tilde character (`~`) is used as a placeholder for the card's own name. This allows the parser to handle self-references generically without needing to know each card's name. Example: *"When ~ enters the battlefield..."* means *"When [this card] enters the battlefield..."* |

---

## System Concepts

| Term | Category | Description |
|------|----------|-------------|
| **AST (Abstract Syntax Tree)** | Parser | The structured tree representation produced by parsing oracle text. Each node in the AST is a strongly-typed TypeScript object representing a game concept (e.g., `TriggeredAbility`, `DealDamage`, `Target`). The AST is the parser's primary output before serialization. See [Oracle Parser Design](oracle_parser.md) for the full node taxonomy. |
| **IR (Intermediate Representation)** | Storage | The compiled, serialized form of the AST — stored as per-set JSON files in `ir/sets/<SET_CODE>.json`. The IR is what downstream layers (rules engine, agents) consume. It is versioned, deterministic, and diffable. See [Data Schemas](data_schemas.md). |
| **Grammar (.g4)** | Parser | The ANTLR grammar files (`MTGLexer.g4` and `MTGParser.g4`) that formally define the tokenization and structural rules for MTG oracle text. The grammar serves as both executable specification and living documentation. See [Decisions](decisions.md) for why ANTLR was chosen. |
| **Visitor Pattern** | Parser | The ANTLR design pattern used to walk the generated parse tree and produce typed AST nodes. The visitor (`ASTBuilder.ts`) traverses each parse tree node and returns the corresponding AST type. This cleanly separates grammar rules from application logic. |
| **Scryfall** | Data Source | The community-standard MTG card database and API ([scryfall.com](https://scryfall.com)). Used as the single source of truth for card data. The ingestion pipeline pulls Scryfall's bulk data exports (JSON), normalizes them, and feeds them into the parser. See [Scryfall Integration](scryfall-integration.md). |
| **Monorepo / npm Workspaces** | Project | The project is structured as a TypeScript monorepo using npm workspaces. Each major component (oracle-parser, card-data, game-engine) is a separate package under `packages/`. See [Project Structure](project-structure.md). |
| **Snapshot Tests** | Testing | A testing strategy where the parser's AST output for a curated set of cards is recorded as "snapshots." Future test runs compare against these snapshots to detect regressions. |
| **Bulk Validation** | Testing | Running the parser against the entire Scryfall card database and reporting coverage statistics — what percentage of cards parse without errors. Used to track parser completeness. |

---

## Quick Symbol Reference

| Symbol | Name | Meaning |
|--------|------|---------|
| `{W}` | White Mana | One white mana |
| `{U}` | Blue Mana | One blue mana (U avoids confusion with Black) |
| `{B}` | Black Mana | One black mana |
| `{R}` | Red Mana | One red mana |
| `{G}` | Green Mana | One green mana |
| `{C}` | Colorless Mana | One colorless mana (specific, not generic) |
| `{X}` | Variable Mana | A variable amount chosen by the player |
| `{1}`, `{2}`, etc. | Generic Mana | Can be paid with any color or colorless mana |
| `{T}` | Tap Symbol | Tap this permanent (activation cost) |
| `{Q}` | Untap Symbol | Untap this permanent (activation cost) |
| `~` | Self-Reference | Refers to the card's own name in oracle text |

---

*See also: [Architecture](architecture.md) · [Oracle Parser](oracle_parser.md) · [Data Schemas](data_schemas.md) · [Decisions](decisions.md) · [Project Structure](project-structure.md)*
