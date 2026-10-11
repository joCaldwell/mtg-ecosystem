import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Effect, ManaProduction, ManaSpendingRestriction, PlayerRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseAddManaEffect } from "../src/parser/mana.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you: PlayerRef = { player: "you" };
function production(text: string, mana: ManaProduction, who: PlayerRef = you, restriction?: ManaSpendingRestriction): void {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.deepEqual(parsed.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{
    effect: "add-mana", who, mana, ...(restriction ? { restriction } : {}),
  }] }] }]);
}

// Oracle fixtures checked from the cached corpus: Arcane Signet, Noble
// Hierarch, Saltcrusted Steppe, Gilded Lotus, Viridian Joiner, Fellwar Stone,
// Dictate of Karametra, Vedalken Engineer, Jeweled Lotus, and Thran Turbine.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("mana production and spending restrictions", () => {
  it("keeps mana bundles, alternatives, and recipient identity", () => {
    production("Add {G}{G}.", { mana: "fixed", symbols: ["G", "G"] });
    production("Add {G}, {W}, or {U}.", { mana: "choice", options: ["G", "W", "U"].map(symbol => ({ mana: "fixed", symbols: [symbol] })) });
    assert.equal(parseOracleText("Add {G}, {U}.").ok, false);
    production("Target player adds {R}{G}.", { mana: "fixed", symbols: ["R", "G"] }, { player: "target-player" });
    production("Its controller adds an additional one mana of any color.", { mana: "any-color", amount: fixed(1) }, { player: "controller", of: { ref: "it" } });
  });

  it("retains production from symbols with special CR 106.8-.11 meanings", () => {
    for (const symbol of ["W/U", "2/W", "W/P", "2", "S"])
      production(`Add {${symbol}}.`, { mana: "fixed", symbols: [symbol] });
    for (const symbol of ["T", "Q", "E", "CHAOS", "NOT-MANA"])
      assert.equal(parseOracleText(`Add {${symbol}}.`).ok, false, symbol);
  });

  it("distinguishes any one color from mixing chosen colors or types", () => {
    production("Add three mana of any one color.", { mana: "any-one-color", amount: fixed(3) });
    production("Add three mana in any combination of colors.", { mana: "combination", amount: fixed(3), options: "colors" });
    production("Add X mana in any combination of {G} and/or {W}.", { mana: "combination", amount: { amount: "x" }, options: ["G", "W"] });
    production("Add three mana in any combination of {U}, {B}, and/or {R}.", { mana: "combination", amount: fixed(3), options: ["U", "B", "R"] });
    production("Add one mana of any type.", { mana: "any-type", amount: fixed(1) });
    assert.equal(parseOracleText("Add two mana in any combination of {2} and/or {G}.").ok, false);
  });

  it("records whose commander supplies the allowed color identity", () => {
    production("Add one mana of any color in your commander's color identity.", {
      mana: "commander-color", amount: fixed(1), whose: you,
    });
    production("Target player adds one mana of any color in your commander's color identity.", {
      mana: "commander-color", amount: fixed(1), whose: you,
    }, { player: "target-player" });
    assert.equal(parseOracleText("Add one mana of any color in your commander's power.").ok, false);
  });

  it("retains amounts calculated from object characteristics", () => {
    production("Add an amount of {G} equal to this creature's power.", { mana: "scaled-symbol", symbol: "G",
      amount: { amount: "attribute", attribute: "power", of: { ref: "this", noun: "creature" } } });
    assert.equal(parseOracleText("Add an amount of {G}{G} equal to this creature's power.").ok, false);
  });

  it("distinguishes possible production from the mana actually produced", () => {
    production("Add one mana of any color that a land an opponent controls could produce.", {
      mana: "could-produce", amount: fixed(1), colorOnly: true,
      what: { ref: "filter", count: fixed(1), filter: { types: ["land"], control: "opponent" } },
    });
    production("That player adds one mana of any type that land produced.", {
      mana: "produced", amount: fixed(1), colorOnly: false,
      what: { ref: "that", filter: { types: ["land"] } },
    }, { player: "that-player" });
    production("Add one mana of any type that a land you control could produce.", {
      mana: "could-produce", amount: fixed(1), colorOnly: false,
      what: { ref: "filter", count: fixed(1), filter: { types: ["land"], control: "you" } },
    });
  });

  it("attaches allowed payment purposes to the produced mana", () => {
    production("Add two mana of any one color. Spend this mana only to cast artifact spells or activate abilities of artifacts.", {
      mana: "any-one-color", amount: fixed(2),
    }, you, { restriction: "only", uses: [
      { use: "cast", filter: { types: ["artifact"], cls: "spell" } },
      { use: "activate", source: { types: ["artifact"] } },
    ] });
    production("Add one mana of any color. Spend this mana only to cast a creature spell.", { mana: "any-color", amount: fixed(1) }, you,
      { restriction: "only", uses: [{ use: "cast", filter: { types: ["creature"], cls: "spell" } }] });
    production("Add three mana of any one color. Spend this mana only to cast your commander.", { mana: "any-one-color", amount: fixed(3) }, you,
      { restriction: "only", uses: [{ use: "cast-commander", whose: you }] });
  });

  it("distinguishes forbidden spending from spending only on a purpose", () => {
    production("Add {C}{C}. This mana can't be spent to cast spells.", { mana: "fixed", symbols: ["C", "C"] }, you,
      { restriction: "not", uses: [{ use: "cast", filter: { cls: "spell" } }] });
    production("Add {W}{U}{B}{R}{G}. This mana can't be spent to cast spells from your hand.", { mana: "fixed", symbols: ["W", "U", "B", "R", "G"] }, you,
      { restriction: "not", uses: [{ use: "cast", filter: { cls: "spell" }, from: { zone: "hand", owner: "your" } }] });
    production("Add {G}. Spend this mana only to cast creature spells from your graveyard.", { mana: "fixed", symbols: ["G"] }, you,
      { restriction: "only", uses: [{ use: "cast", filter: { types: ["creature"], cls: "spell" }, from: { zone: "graveyard", owner: "your" } }] });
  });

  it("keeps the explicit player for coordinated conjugated actions only", () => {
    const parsed = parseOracleText("Target player draws a card and adds {G}.");
    assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
    if (!parsed.ok) throw new Error("failed player instructions");
    assert.deepEqual(parsed.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [
      { effect: "draw", who: { player: "target-player" }, amount: fixed(1) },
      { effect: "add-mana", who: { player: "target-player" }, mana: { mana: "fixed", symbols: ["G"] } },
    ] }] }]);
    for (const text of ["Adds {G}.", "Target player draws a card. Adds {G}."])
      assert.equal(parseOracleText(text).ok, false, text);
  });

  it("retains the chosen-type condition on a mana spending restriction", () => {
    production("Add {G}. Spend this mana only to cast a creature spell of the chosen type.", { mana: "fixed", symbols: ["G"] }, you,
      { restriction: "only", uses: [{ use: "cast", filter: { types: ["creature"], cls: "spell", chosenType: { choice: "chosen-type" } } }] });
  });

  it("rejects unrepresented riders and preserves parser state on failure", () => {
    for (const text of [
      "Add {G}. Spend this mana only to cast creature spells if you control an artifact.",
      "Add {G}. Spend this mana only to cast creature spells, and that spell can't be countered.",
      "Add {G}. Spend this mana only to cast creature spells or.",
      "Add {G}. This mana can't be spent to cast.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
    for (const text of ["Add {T}", "Add an amount of {G} equal to", "Adds {G}"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseAddManaEffect(c), null, text);
      assert.equal(c.pos, 0, text);
    }
    const c = new Cursor(lex("Adds {G}."));
    const actor: PlayerRef = { player: "target-player" };
    const expected: Effect[] = [{ effect: "add-mana", who: actor, mana: { mana: "fixed", symbols: ["G"] } }];
    assert.deepEqual(parseAddManaEffect(c, actor), { effects: expected, actor });
  });
});
