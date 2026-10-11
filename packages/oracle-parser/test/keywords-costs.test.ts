import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Cost, KeywordInstance } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { normalizeOracleText } from "../src/normalize.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCostList } from "../src/parser/costs.ts";
import { parseKeyword } from "../src/parser/keywords.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { buildCardNameCatalog } from "../src/card-names.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const mana = (...symbols: string[]): Cost => ({ cost: "mana", symbols });
function only(text: string): Ability {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return parsed.abilities[0];
}
function keyword(text: string, expected: KeywordInstance): void {
  assert.deepEqual(only(text), { kind: "keywords", keywords: [expected] });
}
function costs(text: string, expected: Cost[]): void {
  assert.deepEqual(only(`${text}: Draw a card.`), {
    kind: "activated", abilityWord: undefined, restriction: undefined, costs: expected,
    effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
  });
}

// Oracle examples checked against the cached oracle-cards.json. Mechanical
// distinctions follow CR 118, 702 and 718, September 25, 2026:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("keyword parameters and identity", () => {
  it("lexes meaningful punctuation and possessives in keyword names", () => {
    for (const [text, name] of [
      ["Doctor’s companion", "doctor's companion"],
      ["For Mirrodin!", "for mirrodin!"],
      ["Start your engines!", "start your engines!"],
    ]) keyword(text, { keyword: name });
    assert.equal(parseOracleText("For Mirrodin").ok, false);
    assert.equal(parseOracleText("Start your engines").ok, false);
  });

  it("retains hexproof qualities and distinct protection scopes", () => {
    keyword("Hexproof from black", { keyword: "hexproof", from: [{ scope: "color", color: "black" }] });
    keyword("Hexproof from each color", { keyword: "hexproof", from: [{ scope: "all-colors" }] });
    keyword("Protection from red and from blue", {
      keyword: "protection", from: [{ scope: "color", color: "red" }, { scope: "color", color: "blue" }],
    });
    keyword("Protection from Vampires, from Werewolves, and from Zombies", {
      keyword: "protection", from: ["Vampire", "Werewolf", "Zombie"].map(subtype =>
        ({ scope: "filter" as const, filter: { subtypes: [subtype] } })),
    });
    keyword("Hexproof from artifacts, creatures, and enchantments", {
      keyword: "hexproof", from: [
        { scope: "filter", filter: { allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }, { types: ["enchantment"] }] }] } },
      ],
    });
    assert.equal(parseOracleText("Protection from red and from").ok, false);
    assert.equal(parseOracleText("Protection from red and green creatures").ok, false);
  });

  it("separates player enchantments from object filters", () => {
    keyword("Enchant player", { keyword: "enchant", enchantPlayer: "player" });
    keyword("Enchant opponent", { keyword: "enchant", enchantPlayer: "opponent" });
    keyword("Enchant creature", { keyword: "enchant", filter: { types: ["creature"] } });
    assert.equal(parseOracleText("Enchant for creature").ok, false);
  });

  it("retains restricted equip targets", () => {
    keyword("Equip creature token {1}", { keyword: "equip", filter: { types: ["creature"], cls: "token" }, cost: [mana("1")] });
    keyword("Equip planeswalker {1}", { keyword: "equip", filter: { types: ["planeswalker"] }, cost: [mana("1")] });
    keyword("Equip Shaman, Warlock, or Wizard {1}", {
      keyword: "equip", filter: { allOf: [{ anyOf: ["Shaman", "Warlock", "Wizard"].map(subtype => ({ subtypes: [subtype] })) }] },
      cost: [mana("1")],
    });
  });

  it("does not conflate independent kicker costs with alternative payments", () => {
    keyword("Kicker {B} and/or {R}", { keyword: "kicker", additionalCosts: [[mana("B")], [mana("R")]] });
    keyword("Cumulative upkeep {W} or {U}", {
      keyword: "cumulative upkeep", cost: [{ cost: "choice", options: [[mana("W")], [mana("U")]] }],
    });
    assert.equal(parseOracleText("Kicker {B} and/or").ok, false);
  });

  it("requires both prototype stats and its alternative cost", () => {
    keyword("Prototype {1}{B} — 1/1", {
      keyword: "prototype", cost: [mana("1", "B")], prototype: { power: fixed(1), toughness: fixed(1) },
    });
    for (const text of ["Prototype {1}{B}", "Prototype {1}{B} — 1/", "Prototype {T} — 1/1"])
      assert.equal(parseOracleText(text).ok, false, text);
  });

  it("retains exact named partners and distinct partner variants", () => {
    const partner = parseOracleText("Partner with Krav, the Unredeemed", undefined, { cardNames: buildCardNameCatalog([
      { name: "Krav, the Unredeemed", layout: "normal" },
    ]) });
    assert.ok(partner.ok);
    assert.deepEqual(partner.abilities, [{ kind: "keywords", keywords: [{ keyword: "partner with", pairedWith: "Krav, the Unredeemed" }] }]);
    keyword("Partner—Friends forever", { keyword: "partner—friends forever" });
    keyword("Partner—Character select", { keyword: "partner—character select" });
    keyword("Partner—Survivors", { keyword: "partner—survivors" });
    assert.equal(parseOracleText("Partner with").ok, false);
    assert.equal(parseOracleText("Partner—Unrecognized phrase").ok, false);
  });

  it("stores the complete splice quality", () => {
    keyword("Splice onto Arcane {1}{U}", { keyword: "splice", filter: { subtypes: ["Arcane"] }, cost: [mana("1", "U")] });
    keyword("Splice onto instant or sorcery {2}{U}", {
      keyword: "splice", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }] }, cost: [mana("2", "U")],
    });
  });

  it("retains the exact predefined gift and rejects undefined variants", () => {
    for (const [text, gift] of [
      ["Gift a card", "card"], ["Gift a tapped Fish", "tapped-fish"],
      ["Gift a Treasure", "treasure"], ["Gift a Food", "food"],
      ["Gift an Octopus", "octopus"], ["Gift an extra turn", "extra-turn"],
    ] as const) keyword(text, { keyword: "gift", gift });
    assert.equal(parseOracleText("Gift a Fish").ok, false);
    assert.equal(parseOracleText("Gift a Clue").ok, false);
  });

  it("retains landwalk and typecycling qualifiers without inventing keywords", () => {
    keyword("Snow forestwalk", { keyword: "landwalk", filter: { types: ["land"], subtypes: ["Forest"], supertypes: ["snow"] } });
    keyword("Nonbasic landwalk", { keyword: "landwalk", filter: { types: ["land"], nonSupertypes: ["basic"] } });
    keyword("Desertwalk", { keyword: "landwalk", filter: { types: ["land"], subtypes: ["Desert"] } });
    keyword("Basic landcycling {1}", { keyword: "typecycling", filter: { types: ["land"], supertypes: ["basic"] }, cost: [mana("1")] });
    keyword("Basic swampcycling {2}", { keyword: "typecycling", filter: { subtypes: ["Swamp"], supertypes: ["basic"] }, cost: [mana("2")] });
    keyword("Artifactcycling {2}", { keyword: "typecycling", filter: { types: ["artifact"] }, cost: [mana("2")] });
    assert.equal(parseOracleText("Inventedcycling {2}").ok, false);
  });

  it("preserves optional-number and restricted-sacrifice keyword meanings", () => {
    keyword("Vanishing", { keyword: "vanishing" });
    keyword("Vanishing 3", { keyword: "vanishing", amount: 3 });
    keyword("Devour artifact 1", { keyword: "devour", amount: 1, filter: { types: ["artifact"] } });
    keyword("Champion a Goblin or Shaman", { keyword: "champion", filter: { allOf: [{ anyOf: [{ subtypes: ["Goblin"] }, { subtypes: ["Shaman"] }] }] } });
    keyword("Goblin offering", { keyword: "offering", filter: { subtypes: ["Goblin"] } });
  });

  it("supports audited current keyword shapes and rejects actions as bare abilities", () => {
    for (const name of ["umbra armor", "living metal", "ravenous", "demonstrate", "station", "increment", "paradigm", "storied"])
      keyword(name, { keyword: name });
    for (const name of ["mutate", "cleave", "escalate", "freerunning", "harmonize", "warp", "mayhem", "web-slinging", "sneak", "aura swap", "transfigure"])
      keyword(`${name} {2}`, { keyword: name, cost: [mana("2")] });
    for (const name of ["mobilize", "firebending", "teamwork"])
      keyword(`${name} 2`, { keyword: name, amount: 2 });
    keyword("Mayhem", { keyword: "mayhem" });
    for (const text of ["Amass", "Channel {2}", "Boast {2}"])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("cost payment distinctions", () => {
  it("retains counted energy payments, including variable and repeated-symbol costs", () => {
    costs("Pay six {E}", [{ cost: "energy", amount: fixed(6) }]);
    costs("Pay fifty {E}", [{ cost: "energy", amount: fixed(50) }]);
    costs("Pay X {E}", [{ cost: "energy", amount: { amount: "x" } }]);
    costs("{E}{E}{E}", [{ cost: "energy", amount: fixed(3) }]);
    assert.equal(parseOracleText("Pay six {T}: Draw a card.").ok, false);
    assert.equal(parseOracleText("Pay six {E}{E}: Draw a card.").ok, false);
  });

  it("preserves exert, milling, and untapping objects as distinct payment actions", () => {
    costs("{T}, Mill a card", [{ cost: "tap-self" }, { cost: "mill", amount: fixed(1) }]);
    costs("Mill four cards", [{ cost: "mill", amount: fixed(4) }]);
    costs("{T}, Exert this creature", [{ cost: "tap-self" }, { cost: "exert", what: { ref: "this", noun: "creature" } }]);
    costs("Untap two tapped creatures you control", [{ cost: "untap-objects", what: {
      ref: "filter", count: fixed(2), filter: { types: ["creature"], status: ["tapped"], control: "you" },
    } }]);
    for (const text of ["Mill four", "Untap", "Exert"]) assert.equal(parseOracleText(`${text}: Draw a card.`).ok, false);
  });

  it("distinguishes removal of all counters, a chosen number, and a total distributed across objects", () => {
    costs("Remove all charge counters from this artifact", [{ cost: "remove-counters", counter: { type: "charge" }, count: "all", from: { ref: "this", noun: "artifact" } }]);
    costs("Remove any number of storage counters from this land", [{ cost: "remove-counters", counter: { type: "storage" }, count: { amount: "chosen", minimum: 0 }, from: { ref: "this", noun: "land" } }]);
    costs("Remove one or more +1/+1 counters from among creatures you control", [{ cost: "remove-counters", counter: { type: "+1/+1" }, count: { amount: "chosen", minimum: 1 }, from: { ref: "each", filter: { types: ["creature"], control: "you" } }, distributed: true }]);
    costs("Remove two +1/+1 counters from among creatures you control", [{ cost: "remove-counters", counter: { type: "+1/+1" }, count: fixed(2), from: { ref: "each", filter: { types: ["creature"], control: "you" } }, distributed: true }]);
    assert.equal(parseOracleText("Remove one or three charge counters from this artifact: Draw a card.").ok, false);
  });
  it("retains keyword cost semantics instead of classifying them as spell instructions", () => {
    keyword("Ward—Sacrifice a creature.", { keyword: "ward", cost: [{ cost: "sacrifice", what: { ref: "filter", filter: { types: ["creature"] }, count: fixed(1) } }] });
    keyword("Equip—Pay {3} or discard a card.", { keyword: "equip", cost: [{ cost: "choice", options: [
      [mana("3")], [{ cost: "discard", what: { ref: "filter", filter: { cls: "card" }, count: fixed(1) } }],
    ] }] });
  });

  it("preserves return destination ownership", () => {
    costs("Return a creature you control to your hand", [{ cost: "return", what: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: fixed(1) }, to: "hand", owner: "your" }]);
    costs("Return a creature you control to its owner's hand", [{ cost: "return", what: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: fixed(1) }, to: "hand", owner: "owner" }]);
  });

  it("retains explicit source zones for concrete objects and a single other sacrifice", () => {
    costs("Exile this card from your graveyard", [{ cost: "exile", what: { ref: "this", noun: "card" }, from: { zone: "graveyard", owner: "your" } }]);
    costs("Reveal this card from your hand", [{ cost: "reveal", what: { ref: "this", noun: "card" }, from: { zone: "hand", owner: "your" } }]);
    costs("Return this card from your graveyard to your hand", [{ cost: "return", what: { ref: "this", noun: "card" }, from: { zone: "graveyard", owner: "your" }, to: "hand", owner: "your" }]);
    costs("Sacrifice another creature", [{ cost: "sacrifice", what: { ref: "filter", filter: { types: ["creature"], other: true }, count: fixed(1) } }]);
    assert.equal(parseOracleText("Exile this card from: Draw a card.").ok, false);
  });

  it("retains reveal, random discard, and grouped symbol choices", () => {
    costs("Reveal a blue card from your hand", [{ cost: "reveal", what: { ref: "filter", filter: { colors: ["blue"], cls: "card", zone: { zone: "hand", owner: "your" } }, count: fixed(1) } }]);
    costs("Discard a card at random", [{ cost: "discard", what: { ref: "filter", filter: { cls: "card" }, count: fixed(1) }, random: true }]);
    costs("{3}, {T} or {U}, {T}", [{ cost: "choice", options: [[mana("3"), { cost: "tap-self" }], [mana("U"), { cost: "tap-self" }]] }]);
    assert.equal(parseOracleText("{3}, {T}, Sacrifice an artifact or discard a card: Draw a card.").ok, false);
  });

  it("distinguishes unspecified counters, specified counters, and placing counters", () => {
    costs("Remove a counter from this artifact", [{ cost: "remove-counters", counter: "any", count: fixed(1), from: { ref: "this", noun: "artifact" } }]);
    costs("Remove a charge counter from this artifact", [{ cost: "remove-counters", counter: { type: "charge" }, count: fixed(1), from: { ref: "this", noun: "artifact" } }]);
    costs("Put a -1/-1 counter on this creature", [{ cost: "put-counters", counter: { type: "-1/-1" }, count: fixed(1), on: { ref: "this", noun: "creature" } }]);
    keyword("Ward—Get five poison counters.", { keyword: "ward", cost: [{ cost: "get-counters", counter: { type: "poison" }, count: fixed(5) }] });
  });

  it("preserves specialized evidence and waterbend payment identities", () => {
    keyword("Ward—Collect evidence 4.", { keyword: "ward", cost: [{ cost: "collect-evidence", amount: fixed(4) }] });
    keyword("Ward—Waterbend {4}.", { keyword: "ward", cost: [{ cost: "waterbend", symbols: ["4"] }] });
    costs("{1}, Waterbend {4}", [mana("1"), { cost: "waterbend", symbols: ["4"] }]);
    assert.equal(parseOracleText("Ward—Waterbend {T}.").ok, false);
    assert.equal(parseOracleText("Ward—Waterbend {TYPO}.").ok, false);
    costs("Forage", [{ cost: "forage" }]);
    costs("Blight 2", [{ cost: "blight", amount: fixed(2) }]);
    assert.equal(parseOracleText("Blight: Draw a card.").ok, false);
  });

  it("rolls back incomplete costs and keyword parameters", () => {
    for (const text of ["Pay {2} or", "Return a creature to", "Reveal", "Discard a card and"]) {
      const cursor = new Cursor(lex(text));
      assert.equal(parseCostList(cursor), null, text);
      assert.equal(cursor.pos, 0, text);
    }
    for (const text of ["Protection from red and from", "Prototype {2} — 2/", "Kicker {B} and/or"]) {
      const cursor = new Cursor(lex(normalizeOracleText(text)[0]));
      assert.equal(parseKeyword(cursor), null, text);
      assert.equal(cursor.pos, 0, text);
    }
  });
});
