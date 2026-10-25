// Verified against the installed Scryfall card database, 2026-10-15.
// Isolated fixtures for the requested sourcing examples; prices in tests are synthetic.
import type { ScryfallCard } from "../src/ingest.ts";
export const SOURCING_CARDS: ScryfallCard[] = [
  {
    "oracle_id": "b922f057-1c91-43eb-b74a-8a933b1be2d2",
    "name": "Winter Moon",
    "mana_cost": "{2}",
    "cmc": 2,
    "type_line": "Artifact",
    "oracle_text": "Players can't untap more than one nonbasic land during their untap steps.",
    "colors": [],
    "color_identity": [],
    "layout": "normal",
    "legalities": {
      "commander": "legal"
    }
  },
  {
    "oracle_id": "24882fa2-3fe9-4c1b-aa3d-0e6488b9db27",
    "name": "Heroic Intervention",
    "mana_cost": "{1}{G}",
    "cmc": 2,
    "type_line": "Instant",
    "oracle_text": "Permanents you control gain hexproof and indestructible until end of turn.",
    "colors": [
      "G"
    ],
    "color_identity": [
      "G"
    ],
    "layout": "normal",
    "legalities": {
      "commander": "legal"
    }
  },
  {
    "oracle_id": "1313da2f-92bb-4862-9153-373972cc1520",
    "name": "Descent into Avernus",
    "mana_cost": "{2}{R}",
    "cmc": 3,
    "type_line": "Enchantment",
    "oracle_text": "At the beginning of your upkeep, put two descent counters on this enchantment. Then each player creates X Treasure tokens and this enchantment deals X damage to each player, where X is the number of descent counters on this enchantment.",
    "colors": [
      "R"
    ],
    "color_identity": [
      "R"
    ],
    "layout": "normal",
    "legalities": {
      "commander": "legal"
    }
  }
];
