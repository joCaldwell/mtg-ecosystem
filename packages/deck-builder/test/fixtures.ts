import type { ScryfallCard } from "../src/ingest.ts";

export const FIXTURES: ScryfallCard[] = [
  {
    oracle_id: "id-llanowar", name: "Llanowar Elves", mana_cost: "{G}", cmc: 1,
    type_line: "Creature — Elf Druid", oracle_text: "{T}: Add {G}.",
    power: "1", toughness: "1", colors: ["G"], color_identity: ["G"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-seedborn", name: "Seedborn Muse", mana_cost: "{3}{G}{G}", cmc: 5,
    type_line: "Creature — Spirit", oracle_text: "Untap all permanents you control during each other player's untap step.",
    power: "2", toughness: "4", colors: ["G"], color_identity: ["G"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-atraxa", name: "Atraxa, Praetors' Voice", mana_cost: "{G}{W}{U}{B}", cmc: 4,
    type_line: "Legendary Creature — Phyrexian Angel Horror",
    oracle_text: "Flying, vigilance, deathtouch, lifelink\nAt the beginning of your end step, proliferate.",
    power: "4", toughness: "4", colors: ["W", "U", "B", "G"], color_identity: ["W", "U", "B", "G"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-witch", name: "Witch Enchanter // Witch-Blessed Meadow", cmc: 4,
    type_line: "Creature — Human Warlock // Land", color_identity: ["W"], layout: "modal_dfc",
    legalities: { commander: "legal" },
    card_faces: [
      {
        name: "Witch Enchanter", mana_cost: "{3}{W}", type_line: "Creature — Human Warlock",
        oracle_text: "When Witch Enchanter enters, destroy up to one target artifact or enchantment.",
        power: "3", toughness: "2", colors: ["W"],
      },
      {
        name: "Witch-Blessed Meadow", mana_cost: "", type_line: "Land",
        oracle_text: "Witch-Blessed Meadow enters tapped.\n{T}: Add {W}.", colors: [],
      },
    ],
  },
  {
    oracle_id: "id-counterspell", name: "Counterspell", mana_cost: "{U}{U}", cmc: 2,
    type_line: "Instant", oracle_text: "Counter target spell.",
    colors: ["U"], color_identity: ["U"], layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-solring", name: "Sol Ring", mana_cost: "{1}", cmc: 1,
    type_line: "Artifact", oracle_text: "{T}: Add {C}{C}.",
    colors: [], color_identity: [], layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-golos", name: "Golos, Tireless Pilgrim", mana_cost: "{5}", cmc: 5,
    type_line: "Legendary Artifact Creature — Scout",
    oracle_text: "When Golos, Tireless Pilgrim enters, search your library for a land card.",
    power: "3", toughness: "5", colors: [], color_identity: ["W", "U", "B", "R", "G"],
    layout: "normal", legalities: { commander: "banned" },
  },
  {
    oracle_id: "id-kenrith", name: "Kenrith, the Returned King", mana_cost: "{4}{W}", cmc: 5,
    type_line: "Legendary Creature — Human Noble",
    oracle_text: "{R}: All creatures gain trample and haste until end of turn.\n{1}{G}: Target player puts a +1/+1 counter on each creature they control.",
    power: "5", toughness: "5", colors: ["W"], color_identity: ["W", "U", "B", "R", "G"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-goyf", name: "Tarmogoyf", mana_cost: "{1}{G}", cmc: 2,
    type_line: "Creature — Lhurgoyf",
    oracle_text: "Tarmogoyf's power is equal to the number of card types among cards in all graveyards and its toughness is equal to that number plus 1.",
    power: "*", toughness: "1+*", colors: ["G"], color_identity: ["G"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-teferi", name: "Teferi, Temporal Archmage", mana_cost: "{4}{U}{U}", cmc: 6,
    type_line: "Legendary Planeswalker — Teferi",
    oracle_text: "+1: Look at the top two cards of your library.\n−10: You get an emblem.\nTeferi, Temporal Archmage can be your commander.",
    loyalty: "5", colors: ["U"], color_identity: ["U"], layout: "normal",
    legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-rats", name: "Typhoid Rats", mana_cost: "{B}", cmc: 1,
    type_line: "Creature — Rat",
    oracle_text: "Deathtouch (Any amount of damage this deals to a creature is enough to destroy it.)",
    power: "1", toughness: "1", colors: ["B"], color_identity: ["B"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-bog", name: "Bojuka Bog", cmc: 0,
    type_line: "Land",
    oracle_text: "Bojuka Bog enters tapped.\nWhen Bojuka Bog enters, exile target player's graveyard.",
    colors: [], color_identity: ["B"], layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-ball", name: "Ball Lightning", mana_cost: "{R}{R}{R}", cmc: 3,
    type_line: "Creature — Elemental",
    oracle_text: "Trample, haste\nAt the beginning of the end step, sacrifice Ball Lightning.",
    power: "6", toughness: "1", colors: ["R"], color_identity: ["R"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-token", name: "Soldier Token", cmc: 0,
    type_line: "Token Creature — Soldier", oracle_text: "",
    power: "1", toughness: "1", colors: ["W"], color_identity: ["W"], layout: "token",
  },
  {
    oracle_id: "id-forest", name: "Forest", cmc: 0,
    type_line: "Basic Land — Forest", oracle_text: "({T}: Add {G}.)",
    colors: [], color_identity: ["G"], layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-relentless", name: "Relentless Rats", mana_cost: "{1}{B}{B}", cmc: 3,
    type_line: "Creature — Rat",
    oracle_text: "Relentless Rats gets +1/+1 for each other creature named Relentless Rats.\nA deck can have any number of cards named Relentless Rats.",
    power: "2", toughness: "2", colors: ["B"], color_identity: ["B"],
    layout: "normal", legalities: { commander: "legal" },
  },
  {
    oracle_id: "id-dwarves", name: "Seven Dwarves", mana_cost: "{R}", cmc: 1,
    type_line: "Creature — Dwarf",
    oracle_text: "Seven Dwarves gets +1/+1 for each other creature named Seven Dwarves.\nA deck can have up to seven cards named Seven Dwarves.",
    power: "2", toughness: "2", colors: ["R"], color_identity: ["R"],
    layout: "normal", legalities: { commander: "legal" },
  },
];
