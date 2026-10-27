// Curated card facts verified against the disposable Scryfall-backed cache DB.
// https://scryfall.com/docs/api/cards — preserve per-face costs/text verbatim.
// These focused fixtures intentionally include complete source ability text.
export const MANA_FIXTURES = [
  {
    "oracle_id": "cc187110-1148-4090-bbb8-e205694a39f5",
    "name": "Counterspell",
    "layout": "normal",
    "mana_cost": "{U}{U}",
    "type_line": "Instant",
    "oracle_text": "Counter target spell.",
    "faces_json": null
  },
  {
    "oracle_id": "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6",
    "name": "Forest",
    "layout": "normal",
    "mana_cost": null,
    "type_line": "Basic Land — Forest",
    "oracle_text": "({T}: Add {G}.)",
    "faces_json": null
  },
  {
    "oracle_id": "6ad8011d-3471-4369-9d68-b264cc027487",
    "name": "Sol Ring",
    "layout": "normal",
    "mana_cost": "{1}",
    "type_line": "Artifact",
    "oracle_text": "{T}: Add {C}{C}.",
    "faces_json": null
  },
  {
    "oracle_id": "2fcc69ff-8ab5-4e14-afe3-db892049a872",
    "name": "Boreal Druid",
    "layout": "normal",
    "mana_cost": "{G}",
    "type_line": "Snow Creature — Elf Druid",
    "oracle_text": "{T}: Add {C}.",
    "faces_json": null
  },
  {
    "oracle_id": "027dd013-baa7-4111-b3c9-f4d1414e9c45",
    "name": "Glacial Fortress",
    "layout": "normal",
    "mana_cost": null,
    "type_line": "Land",
    "oracle_text": "This land enters tapped unless you control a Plains or an Island.\n{T}: Add {W} or {U}.",
    "faces_json": null
  },
  {
    "oracle_id": "00625242-9348-4ef4-b975-f2ac82fee21d",
    "name": "Savai Triome",
    "layout": "normal",
    "mana_cost": null,
    "type_line": "Land — Mountain Plains Swamp",
    "oracle_text": "({T}: Add {R}, {W}, or {B}.)\nThis land enters tapped.\nCycling {3} ({3}, Discard this card: Draw a card.)",
    "faces_json": null
  },
  {
    "oracle_id": "d3a0b660-358c-41bd-9cd2-41fbf3491b1a",
    "name": "Birds of Paradise",
    "layout": "normal",
    "mana_cost": "{G}",
    "type_line": "Creature — Bird",
    "oracle_text": "Flying\n{T}: Add one mana of any color.",
    "faces_json": null
  },
  {
    "oracle_id": "0895c9b7-ae7d-4bb3-af17-3b75deb50a25",
    "name": "Command Tower",
    "layout": "normal",
    "mana_cost": null,
    "type_line": "Land",
    "oracle_text": "{T}: Add one mana of any color in your commander's color identity.",
    "faces_json": null
  },
  {
    "oracle_id": "0bc7f093-bef0-4f1a-852c-4b75ebf54838",
    "name": "Arcane Signet",
    "layout": "normal",
    "mana_cost": "{2}",
    "type_line": "Artifact",
    "oracle_text": "{T}: Add one mana of any color in your commander's color identity.",
    "faces_json": null
  },
  {
    "oracle_id": "1436dd81-496e-42a5-b210-fb5b9cdf073f",
    "name": "Selesnya Signet",
    "layout": "normal",
    "mana_cost": "{2}",
    "type_line": "Artifact",
    "oracle_text": "{1}, {T}: Add {G}{W}.",
    "faces_json": null
  },
  {
    "oracle_id": "0222414f-98b5-458a-a0fd-831a66cd8b07",
    "name": "Tainted Isle",
    "layout": "normal",
    "mana_cost": null,
    "type_line": "Land",
    "oracle_text": "{T}: Add {C}.\n{T}: Add {U} or {B}. Activate only if you control a Swamp.",
    "faces_json": null
  },
  {
    "oracle_id": "00225c40-27ff-410f-a591-5c788c6a2bd6",
    "name": "Ruthless Knave",
    "layout": "normal",
    "mana_cost": "{2}{B}",
    "type_line": "Creature — Orc Pirate",
    "oracle_text": "{2}{B}, Sacrifice a creature: Create two Treasure tokens. (They're artifacts with \"{T}, Sacrifice this token: Add one mana of any color.\")\nSacrifice three Treasures: Draw a card.",
    "faces_json": null
  },
  {
    "oracle_id": "cf14d4e5-5965-45ad-97f7-26facf2884b5",
    "name": "Fertile Ground",
    "layout": "normal",
    "mana_cost": "{1}{G}",
    "type_line": "Enchantment — Aura",
    "oracle_text": "Enchant land\nWhenever enchanted land is tapped for mana, its controller adds an additional one mana of any color.",
    "faces_json": null
  },
  {
    "oracle_id": "a75445d3-1303-4bb5-89ad-26ea93fecd48",
    "name": "Evolving Wilds",
    "layout": "normal",
    "mana_cost": null,
    "type_line": "Land",
    "oracle_text": "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
    "faces_json": null
  },
  {
    "oracle_id": "8539f295-5d58-4436-a73a-b9277c4c7795",
    "name": "Rampant Growth",
    "layout": "normal",
    "mana_cost": "{1}{G}",
    "type_line": "Sorcery",
    "oracle_text": "Search your library for a basic land card, put that card onto the battlefield tapped, then shuffle.",
    "faces_json": null
  },
  {
    "oracle_id": "0355249a-8e4e-41db-9cea-1b901faffbe6",
    "name": "Witch Enchanter // Witch-Blessed Meadow",
    "layout": "modal_dfc",
    "mana_cost": "{3}{W} // ",
    "type_line": "Creature — Human Warlock // Land",
    "oracle_text": "Witch Enchanter\nWhen this creature enters, destroy target artifact or enchantment an opponent controls.\n//\nWitch-Blessed Meadow\nAs this land enters, you may pay 3 life. If you don't, it enters tapped.\n{T}: Add {W}.",
    "faces_json": "[{\"name\":\"Witch Enchanter\",\"mana_cost\":\"{3}{W}\",\"type_line\":\"Creature — Human Warlock\",\"oracle_text\":\"When this creature enters, destroy target artifact or enchantment an opponent controls.\",\"power\":\"2\",\"toughness\":\"2\",\"loyalty\":null},{\"name\":\"Witch-Blessed Meadow\",\"mana_cost\":\"\",\"type_line\":\"Land\",\"oracle_text\":\"As this land enters, you may pay 3 life. If you don't, it enters tapped.\\n{T}: Add {W}.\",\"power\":null,\"toughness\":null,\"loyalty\":null}]"
  },
  {
    "oracle_id": "0272ca81-e727-4f4b-b06e-072d70bb5558",
    "name": "Ulvenwald Captive // Ulvenwald Abomination",
    "layout": "transform",
    "mana_cost": "{1}{G} // ",
    "type_line": "Creature — Werewolf Horror // Creature — Eldrazi Werewolf",
    "oracle_text": "Ulvenwald Captive\nDefender\n{T}: Add {G}.\n{5}{G}{G}: Transform this creature.\n//\nUlvenwald Abomination\n{T}: Add {C}{C}.",
    "faces_json": "[{\"name\":\"Ulvenwald Captive\",\"mana_cost\":\"{1}{G}\",\"type_line\":\"Creature — Werewolf Horror\",\"oracle_text\":\"Defender\\n{T}: Add {G}.\\n{5}{G}{G}: Transform this creature.\",\"power\":\"1\",\"toughness\":\"2\",\"loyalty\":null},{\"name\":\"Ulvenwald Abomination\",\"mana_cost\":\"\",\"type_line\":\"Creature — Eldrazi Werewolf\",\"oracle_text\":\"{T}: Add {C}{C}.\",\"power\":\"4\",\"toughness\":\"6\",\"loyalty\":null}]"
  },
  {
    "oracle_id": "461b3f2f-fcee-4160-abfa-061f8b6a784f",
    "name": "Hengegate Pathway // Mistgate Pathway",
    "layout": "modal_dfc",
    "mana_cost": " // ",
    "type_line": "Land // Land",
    "oracle_text": "Hengegate Pathway\n{T}: Add {W}.\n//\nMistgate Pathway\n{T}: Add {U}.",
    "faces_json": "[{\"name\":\"Hengegate Pathway\",\"mana_cost\":\"\",\"type_line\":\"Land\",\"oracle_text\":\"{T}: Add {W}.\",\"power\":null,\"toughness\":null,\"loyalty\":null},{\"name\":\"Mistgate Pathway\",\"mana_cost\":\"\",\"type_line\":\"Land\",\"oracle_text\":\"{T}: Add {U}.\",\"power\":null,\"toughness\":null,\"loyalty\":null}]"
  },
  {
    "oracle_id": "005c181a-05db-4c15-9893-65103cca338e",
    "name": "Toluz, Clever Conductor",
    "layout": "normal",
    "mana_cost": "{W/U}{U}{U/B}",
    "type_line": "Legendary Creature — Human Rogue",
    "oracle_text": "When Toluz enters, it connives. (Draw a card, then discard a card. If you discarded a nonland card, put a +1/+1 counter on this creature.)\nWhenever you discard one or more cards, exile them from your graveyard.\nWhen Toluz dies, put the cards exiled with it into their owner's hand.",
    "faces_json": null
  },
  {
    "oracle_id": "211d1c59-0111-48dd-9ff6-d03e7ff2ad5c",
    "name": "Kin-Tree Severance",
    "layout": "normal",
    "mana_cost": "{2/W}{2/B}{2/G}",
    "type_line": "Instant",
    "oracle_text": "Exile target permanent with mana value 3 or greater.",
    "faces_json": null
  },
  {
    "oracle_id": "021b4745-4a15-462c-a6b0-2dee1dc9d196",
    "name": "Tamiyo, Compleated Sage",
    "layout": "normal",
    "mana_cost": "{2}{G}{G/U/P}{U}",
    "type_line": "Legendary Planeswalker — Tamiyo",
    "oracle_text": "Compleated ({G/U/P} can be paid with {G}, {U}, or 2 life. If life was paid, this planeswalker enters with two fewer loyalty counters.)\n+1: Tap up to one target artifact or creature. It doesn't untap during its controller's next untap step.\n−X: Exile target nonland permanent card with mana value X from your graveyard. Create a token that's a copy of that card.\n−7: Create Tamiyo's Notebook, a legendary colorless Book artifact token with \"Spells you cast cost {2} less to cast\" and \"{T}: Draw a card.\"",
    "faces_json": null
  },
  {
    "oracle_id": "9f6828e3-39d9-45d9-9bf1-0e3737a0321e",
    "name": "Ulalek, Fused Atrocity",
    "layout": "normal",
    "mana_cost": "{C/W}{C/U}{C/B}{C/R}{C/G}",
    "type_line": "Legendary Creature — Eldrazi",
    "oracle_text": "Devoid (This card has no color.)\nWhenever you cast an Eldrazi spell, you may pay {C}{C}. If you do, copy all spells you control, then copy all other activated and triggered abilities you control. You may choose new targets for the copies. (Mana abilities can't be copied.)",
    "faces_json": null
  },
  {
    "oracle_id": "0030578c-afaf-4072-a62b-2510bdceb64a",
    "name": "Lattice Library",
    "layout": "normal",
    "mana_cost": "{X}{G}{G}",
    "type_line": "Enchantment",
    "oracle_text": "This enchantment enters with X study counters on it.\nWhen this enchantment enters and whenever you cast your first spell with {X} in its mana cost each turn, create a 0/0 green and blue Fractal creature token. Put a number of +1/+1 counters on it equal to the number of study counters on this enchantment.",
    "faces_json": null
  },
  {
    "oracle_id": "29b925e9-1e98-471c-9b23-e0372ac86485",
    "name": "Icehide Golem",
    "layout": "normal",
    "mana_cost": "{S}",
    "type_line": "Snow Artifact Creature — Golem",
    "oracle_text": "({S} can be paid with one mana from a snow source.)",
    "faces_json": null
  },
  {
    "oracle_id": "01160cb5-6d83-4317-9522-74083b3a83bb",
    "name": "Eldrazi Confluence",
    "layout": "normal",
    "mana_cost": "{2}{C}{C}",
    "type_line": "Instant",
    "oracle_text": "Choose three. You may choose the same mode more than once.\n• Target creature gets +3/-3 until end of turn.\n• Exile target nonland permanent, then return it to the battlefield tapped under its owner's control.\n• Create a 1/1 colorless Eldrazi Scion creature token with \"Sacrifice this token: Add {C}.\"",
    "faces_json": null
  },
  {
    "oracle_id": "0a69c3dd-52a3-4ef6-b53f-b0780e0dc5f8",
    "name": "Alrund, God of the Cosmos // Hakka, Whispering Raven",
    "layout": "modal_dfc",
    "mana_cost": "{3}{U}{U} // {1}{U}",
    "type_line": "Legendary Creature — God // Legendary Creature — Bird",
    "oracle_text": "Alrund, God of the Cosmos\nAlrund gets +1/+1 for each card in your hand and each foretold card you own in exile.\nAt the beginning of your end step, choose a card type, then reveal the top two cards of your library. Put all cards of the chosen type revealed this way into your hand and the rest on the bottom of your library in any order.\n//\nHakka, Whispering Raven\nFlying\nWhenever Hakka deals combat damage to a player, return it to its owner's hand, then scry 2.",
    "faces_json": "[{\"name\":\"Alrund, God of the Cosmos\",\"mana_cost\":\"{3}{U}{U}\",\"type_line\":\"Legendary Creature — God\",\"oracle_text\":\"Alrund gets +1/+1 for each card in your hand and each foretold card you own in exile.\\nAt the beginning of your end step, choose a card type, then reveal the top two cards of your library. Put all cards of the chosen type revealed this way into your hand and the rest on the bottom of your library in any order.\",\"power\":\"1\",\"toughness\":\"1\",\"loyalty\":null},{\"name\":\"Hakka, Whispering Raven\",\"mana_cost\":\"{1}{U}\",\"type_line\":\"Legendary Creature — Bird\",\"oracle_text\":\"Flying\\nWhenever Hakka deals combat damage to a player, return it to its owner's hand, then scry 2.\",\"power\":\"2\",\"toughness\":\"3\",\"loyalty\":null}]"
  }
] as const;
