import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";

// Wave-1 cost-rules constructs. Wordings follow cards verified in the cached
// Scryfall corpus (Sewer Rats, Skinshifter, Nykthos Paragon, Deadly Alliance,
// Monk Class, Sage of the Beyond, Catalyst Stone, Armor Wars, Hardened
// Berserker, the Kamigawa Marches, Arena of Glory, Scaled Nurturer, Delighted
// Halfling, the Alara Heralds, Lurking Evil, Aether Storm, Sphinx's Decree,
// Leather Armor, Luxurious Locomotive), reduced to minimal bodies.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
type Case = [typeLine: string, text: string, abilities: unknown];
function accepts([typeLine, text, abilities]: Case): void {
  const parsed = parseOracleText(text, undefined, { typeLine });
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  // Absent optional fields and explicit undefined are the same AST.
  assert.deepEqual(JSON.parse(JSON.stringify(parsed.abilities)), abilities, text);
}
function rejects(typeLine: string, text: string): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("activation limits and conjoined restrictions (CR 602.5b)", () => {
  it("represents every printed clause", () => {
    accepts(["Creature", "{B}: Draw a card. Activate no more than three times each turn.",
      [{"kind":"activated","costs":[{"cost":"mana","symbols":["B"]}],"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}],"restriction":{"restriction":"activation-limit","maximum":3,"period":"turn"}}]]);
    accepts(["Creature", "{B}: Draw a card. Activate only during your turn, only if you control an artifact, and only once each turn.",
      [{"kind":"activated","costs":[{"cost":"mana","symbols":["B"]}],"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}],"restriction":{"restriction":"all-of","restrictions":[{"restriction":"timing","turn":"your"},{"restriction":"condition","condition":{"condition":"control","who":{"player":"you"},"what":{"ref":"filter","filter":{"types":["artifact"]},"count":{"amount":"fixed","value":1}}}},{"restriction":"once-each-turn"}]}}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Creature", "{B}: Draw a card. Activate no more than twice.");
  });
});

describe("a modal activated ability keeps its restriction tail (CR 602.5, 700.2)", () => {
  it("represents every printed clause", () => {
    accepts(["Creature", "{G}: Choose one. Activate only once each turn.\n• Draw a card.\n• You gain 1 life.",
      [{"kind":"activated","costs":[{"cost":"mana","symbols":["G"]}],"effects":[{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]},{"effects":[{"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}]}]}],"restriction":{"restriction":"once-each-turn"}}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Creature", "{G}: Choose one. Activate only as a creature.\n• Draw a card.\n• You gain 1 life.");
  });
});

describe("\"Do this only once each turn\" limits the action, not the trigger (CR 603.2h)", () => {
  it("represents every printed clause", () => {
    accepts(["Creature", "Whenever you gain life, you may draw a card. Do this only once each turn.",
      [{"kind":"triggered","trigger":{"trigger":"gains-life","who":{"player":"you"}},"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1},"optional":true}]}],"actionLimit":{"maximum":1,"period":"turn"}}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Creature", "{T}: You may draw a card. Do this only once each turn.");
  });
});

describe("party size as a cost multiplier (CR 700.8a)", () => {
  it("represents every printed clause", () => {
    accepts(["Instant", "This spell costs {1} less to cast for each creature in your party.",
      [{"kind":"static","effect":{"static":"cost-modification","scope":{"action":"cast","spells":{"ref":"this","noun":"spell"}},"change":{"direction":"reduce","symbols":["1"]},"multiplier":{"amount":"party-size","who":{"player":"you"}}}}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Instant", "This spell costs {1} less to cast for each creature in an opponent's party.");
  });
});

describe("ordinal and excluded-origin casting scopes (CR 601.2f)", () => {
  it("represents every printed clause", () => {
    accepts(["Creature", "The second spell you cast each turn costs {1} less to cast.",
      [{"kind":"static","effect":{"static":"cost-modification","scope":{"action":"cast","spells":{"ref":"each","filter":{"cls":"spell"}},"caster":{"player":"you"},"occurrence":{"ordinal":2,"period":"turn"}},"change":{"direction":"reduce","symbols":["1"]}}}]]);
    accepts(["Creature", "Spells you cast from anywhere other than your hand cost {2} less to cast.",
      [{"kind":"static","effect":{"static":"cost-modification","scope":{"action":"cast","spells":{"ref":"each","filter":{"cls":"spell"}},"caster":{"player":"you"},"notFrom":{"zone":"hand","owner":"your"}},"change":{"direction":"reduce","symbols":["2"]}}}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Creature", "The first creature spell you cast costs {1} less to cast.");
    rejects("Creature", "The next spell you cast each turn costs {1} less to cast.");
  });
});

describe("keyword-cost scopes (CR 118.8-118.9, 116.2, 702.6a)", () => {
  it("represents every printed clause", () => {
    accepts(["Artifact", "Flashback costs your opponents pay cost {2} more.",
      [{"kind":"static","effect":{"static":"cost-modification","scope":{"action":"pay-cost","cost":"flashback","payer":{"player":"each-opponent"}},"change":{"direction":"increase","symbols":["2"]}}}]]);
    accepts(["Artifact", "All morph costs cost {2} more.",
      [{"kind":"static","effect":{"static":"cost-modification","scope":{"action":"pay-cost","cost":"morph"},"change":{"direction":"increase","symbols":["2"]}}}]]);
    accepts(["Artifact", "Equip costs you pay cost {1} less.",
      [{"kind":"static","effect":{"static":"cost-modification","scope":{"action":"activate","activator":{"player":"you"},"keyword":"equip","abilities":{"ref":"each","filter":{"cls":"ability"}}},"change":{"direction":"reduce","symbols":["1"]}}}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Artifact", "Cycling costs you pay cost {1} less.");
    rejects("Artifact", "All flashback costs you pay cost {2} less.");
  });
});

describe("resolving cost modifications need a lifetime (CR 611.2a)", () => {
  it("represents every printed clause", () => {
    accepts(["Creature", "Whenever this creature attacks, artifact spells you cast this turn cost {1} less to cast.",
      [{"kind":"triggered","trigger":{"trigger":"attacks","what":{"ref":"this","noun":"creature"}},"effects":[{"steps":[{"effect":"cost-modification","scope":{"action":"cast","spells":{"ref":"each","filter":{"types":["artifact"],"cls":"spell"}},"caster":{"player":"you"}},"change":{"direction":"reduce","symbols":["1"]},"duration":{"duration":"this-turn"}}]}]}]]);
    accepts(["Creature", "Whenever this creature attacks, the next spell you cast this turn costs {1} less to cast.",
      [{"kind":"triggered","trigger":{"trigger":"attacks","what":{"ref":"this","noun":"creature"}},"effects":[{"steps":[{"effect":"cost-modification","scope":{"action":"cast","spells":{"ref":"each","filter":{"cls":"spell"}},"caster":{"player":"you"},"occurrence":{"next":true}},"change":{"direction":"reduce","symbols":["1"]},"duration":{"duration":"this-turn"}}]}]}]]);
    accepts(["Sorcery", "Until your next turn, spells your opponents cast cost {1} more to cast.",
      [{"kind":"spell","effects":[{"steps":[{"effect":"cost-modification","scope":{"action":"cast","spells":{"ref":"each","filter":{"cls":"spell"}},"caster":{"player":"each-opponent"}},"change":{"direction":"increase","symbols":["1"]},"duration":{"duration":"your-next-turn"}}]}]}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Creature", "Whenever this creature attacks, spells you cast cost {1} less to cast.");
    rejects("Creature", "Whenever this creature attacks, the first spell you cast this turn costs {1} less to cast.");
  });
});

describe("optional additional cost with its own reduction (CR 118.8b, 601.2b/f)", () => {
  it("represents every printed clause", () => {
    accepts(["Sorcery", "As an additional cost to cast this spell, you may exile any number of red cards from your hand. This spell costs {2} less to cast for each card exiled this way.",
      [{"kind":"additional-cost","costs":[{"cost":"exile","what":{"ref":"filter","filter":{"colors":["red"],"cls":"card","zone":{"zone":"hand","owner":"your"}},"count":{"amount":"chosen","minimum":0}}}],"optional":true,"costModifications":[{"change":{"direction":"reduce","symbols":["2"]},"multiplier":{"amount":"count","of":{"cls":"card","relation":{"relation":"event-result","event":"exile","scope":"this-way"}}}}]}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Sorcery", "As an additional cost to cast this spell, you may exile a card from your hand. This ability costs {2} less to activate.");
  });
});

describe("mana-spending purposes and riders (CR 106.6, 603.7a)", () => {
  it("represents every printed clause", () => {
    accepts(["Creature", "{T}: Add {C}. Spend this mana only to cast an artifact spell or to activate an ability of an artifact source.",
      [{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["C"]},"restriction":{"restriction":"only","uses":[{"use":"cast","filter":{"types":["artifact"],"cls":"spell"}},{"use":"activate","source":{"types":["artifact"],"cls":"source"}}]}}]}]}]]);
    accepts(["Creature", "{T}: Add X mana of any one color, where X is this creature's power. Spend this mana only to cast Elf spells and activate abilities.",
      [{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"any-one-color","amount":{"amount":"x"}},"restriction":{"restriction":"only","uses":[{"use":"cast","filter":{"subtypes":["Elf"],"cls":"spell"}},{"use":"activate"}]}}],"bindings":[{"variable":"x","value":{"amount":"attribute","of":{"ref":"this","noun":"creature"},"attribute":"power"}}]}]}]]);
    accepts(["Land", "{T}: Add {R}{R}. If any of that mana is spent on a creature spell, it gains haste until end of turn.",
      [{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["R","R"]},"riders":[{"rider":"effect","mana":"any-of-that","spentOn":[{"use":"cast","filter":{"types":["creature"],"cls":"spell"}}],"effects":[{"steps":[{"effect":"gain-abilities","what":{"ref":"it"},"abilities":[{"gained":"keyword","keyword":{"keyword":"haste"}}],"duration":{"duration":"end-of-turn"}}]}]}]}]}]}]]);
    accepts(["Land", "{T}: Add {G}. When you spend this mana to cast a Dragon spell, you gain 2 life.",
      [{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]},"riders":[{"rider":"delayed-trigger","spentOn":[{"use":"cast","filter":{"subtypes":["Dragon"],"cls":"spell"}}],"effects":[{"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":2}}]}]}]}]}]}]]);
    accepts(["Land", "{T}: Add {G}. When that mana is spent to cast a Dragon spell, you gain 2 life.",
      [{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]},"riders":[{"rider":"delayed-trigger","spentOn":[{"use":"cast","filter":{"subtypes":["Dragon"],"cls":"spell"}}],"effects":[{"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":2}}]}]}]}]}]}]]);
    accepts(["Land", "{T}: Add {C}. Spend this mana only to cast a legendary spell, and that spell can't be countered.",
      [{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["C"]},"restriction":{"restriction":"only","uses":[{"use":"cast","filter":{"supertypes":["legendary"],"cls":"spell"}}]},"riders":[{"rider":"effect","mana":"that","spentOn":[{"use":"cast","filter":{"supertypes":["legendary"],"cls":"spell"}}],"effects":[{"steps":[{"effect":"cant","what":{"ref":"that","filter":{"cls":"spell"}},"action":"be-countered"}]}]}]}]}]}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Land", "{T}: Add {C}. If that mana is spent on a creature card, it gains haste.");
    rejects("Land", "{T}: Add {C}. If that mana is spent on a creature spell, invented effect.");
  });
});

describe("shared-verb sacrifice costs and halved life payment (CR 118.3, 107.1a)", () => {
  it("represents every printed clause", () => {
    accepts(["Creature", "{T}, Sacrifice a red creature, a green creature, and this creature: Draw a card.",
      [{"kind":"activated","costs":[{"cost":"tap-self"},{"cost":"sacrifice","what":{"ref":"filter","filter":{"colors":["red"],"types":["creature"]},"count":{"amount":"fixed","value":1}}},{"cost":"sacrifice","what":{"ref":"filter","filter":{"colors":["green"],"types":["creature"]},"count":{"amount":"fixed","value":1}}},{"cost":"sacrifice","what":{"ref":"this","noun":"creature"}}],"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}]]);
    accepts(["Creature", "Pay half your life, rounded up: Draw a card.",
      [{"kind":"activated","costs":[{"cost":"pay-life","amount":{"amount":"half","of":{"amount":"life-total","of":{"player":"you"}},"round":"up"}}],"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Creature", "Pay half your life: Draw a card.");
  });
});

describe("passive cast prohibition and next-turn window (CR 101.2, 601.3)", () => {
  it("represents every printed clause", () => {
    accepts(["Enchantment", "Creature spells can't be cast.",
      [{"kind":"static","effect":{"static":"prohibit","prohibition":{"prohibition":"cast","who":{"player":"each-player"},"what":{"ref":"each","filter":{"types":["creature"],"cls":"spell"}}}}}]]);
    accepts(["Sorcery", "Each opponent can't cast instant or sorcery spells during that player's next turn.",
      [{"kind":"spell","effects":[{"steps":[{"effect":"prohibit","prohibition":{"prohibition":"cast","who":{"player":"each-opponent"},"what":{"ref":"each","filter":{"allOf":[{"anyOf":[{"types":["instant"]},{"types":["sorcery"]}]}],"cls":"spell"}},"when":{"during":"next-turn","whose":{"player":"that-player"}}}}]}]}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Enchantment", "Each opponent can't cast spells during that player's next turn.");
  });
});

describe("restriction on a keyword's activated ability (CR 602.5b, 702.6a, 702.122a)", () => {
  it("represents every printed clause", () => {
    accepts(["Artifact — Equipment", "Equip {0}. Activate only once each turn.",
      [{"kind":"keywords","keywords":[{"keyword":"equip","cost":[{"cost":"mana","symbols":["0"]}],"restriction":{"restriction":"once-each-turn"}}]}]]);
    accepts(["Artifact — Vehicle", "Crew 1. Activate only once each turn.",
      [{"kind":"keywords","keywords":[{"keyword":"crew","amount":1,"restriction":{"restriction":"once-each-turn"}}]}]]);
  });
  it("rejects near-miss wording", () => {
    rejects("Creature", "Flying. Activate only once each turn.");
    rejects("Artifact — Equipment", "Equip {1}. Any player may activate this ability.");
  });
});
