import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ChosenValueCategory, ChosenValueRef, Effect, ValueChoice } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseChosenValueRef, parseValueChoice } from "../src/parser/choices.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle excerpts checked against the cached corpus: Citadel Gate, Coldsteel
// Heart, Brave the Elements, True-Name Nemesis, Runed Halo, Stenn, Paranoid
// Partisan, Tablet of the Guilds, Sanctuary Blade and Heraldic Banner.
// CR 105.1, 106, 607.2d, 702.16a/k:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed" as const, value });
const chosen = <C extends ChosenValueCategory>(category: C, which: ChosenValueRef["which"] = "chosen"): ChosenValueRef<C> =>
  ({ choice: "chosen-value", category, which });
function parse(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function spell(text: string, steps: Effect[]) {
  assert.deepEqual(parse(text), [{ kind: "spell", effects: [{ steps }] }]);
}
function value(text: string, expected: ValueChoice) {
  const cursor = new Cursor(lex(text));
  assert.deepEqual(parseValueChoice(cursor), expected);
  assert.ok(cursor.done(), cursor.errorMessage());
}

describe("restricted game-value choices", () => {
  it("keeps color counts, exact color options, and exclusions distinct", () => {
    value("a color other than white", { category: "color", excluding: ["white"] });
    value("two colors", { category: "color", count: 2 });
    value("a color other than blue or black", { category: "color", excluding: ["blue", "black"] });
    value("white, blue, black, red, or green", { category: "color", options: ["white", "blue", "black", "red", "green"] });
    value("a card type other than creature or land", { category: "card-type", excluding: ["creature", "land"] });
    spell("Choose a card type other than creature or land.", [{ effect: "choose-value", who: { player: "you" }, category: "card-type", excluding: ["creature", "land"] }]);
  });
  it("rejects unknown values and malformed lists with a clean rewind", () => {
    for (const text of ["a color other than", "a color other than purple", "a color other than white or white", "a card type other than Dragon",
      "a card type other than creature or", "white or", "white, blue", "white or white", "six colors", "zero colors", "two color"]) {
      const cursor = new Cursor(lex(text));
      assert.equal(parseValueChoice(cursor), null, text);
      assert.equal(cursor.pos, 0, text);
    }
    for (const text of ["Choose a color other than white and blue.", "Choose white or blue creatures.", "Choose a color at random."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
  it("retains the kind and latest-choice qualifier instead of inventing a resolved value", () => {
    for (const category of ["color", "player", "card-type", "card-name"] as const) {
      const printed = category.replace("-", " ");
      for (const last of [false, true]) {
        const cursor = new Cursor(lex(`the ${last ? "last " : ""}chosen ${printed}`));
        assert.deepEqual(parseChosenValueRef(cursor), chosen(category, last ? "last-chosen" : "chosen"));
        assert.ok(cursor.done());
      }
    }
    for (const text of ["a chosen color", "that color", "the chosen colors", "the chosen power", "the last color"]) {
      const cursor = new Cursor(lex(text));
      assert.equal(parseChosenValueRef(cursor), null, text);
      assert.equal(cursor.pos, 0, text);
    }
  });
});

describe("chosen-color mana and filter references", () => {
  it("keeps Citadel Gate's entry choice separate from each activation's production alternative", () => {
    assert.deepEqual(parse("This land enters tapped.\nAs this land enters, choose a color other than white.\n{T}: Add {W} or one mana of the chosen color."), [
      { kind: "static", effect: { static: "enters-tapped", what: { ref: "this", noun: "land" } } },
      { kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "land" }, effects: [{ steps: [
        { effect: "choose-value", who: { player: "you" }, category: "color", excluding: ["white"] },
      ] }] } },
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "choice", options: [
        { mana: "fixed", symbols: ["W"] }, { mana: "chosen-color", amount: n(1), choice: chosen("color") },
      ] } }] }] },
    ]);
  });
  it("preserves the printed recipient, amount, and restrictions with a symbolic color", () => {
    spell("Target player adds three mana of the chosen color.", [{ effect: "add-mana", who: { player: "target-player" }, mana: { mana: "chosen-color", amount: n(3), choice: chosen("color") } }]);
    spell("Its controller adds an additional one mana of the chosen color.", [{ effect: "add-mana", who: { player: "controller", of: { ref: "it" } }, mana: { mana: "chosen-color", amount: n(1), choice: chosen("color") } }]);
    spell("Add two mana of the chosen color. Spend this mana only to activate abilities of land sources.", [{ effect: "add-mana", who: { player: "you" },
      mana: { mana: "chosen-color", amount: n(2), choice: chosen("color") }, restriction: { restriction: "only", uses: [{ use: "activate", source: { types: ["land"], cls: "source" } }] } }]);
    spell("Add {R}{R}, {G}, or one mana of the chosen color.", [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "choice", options: [
      { mana: "fixed", symbols: ["R", "R"] }, { mana: "fixed", symbols: ["G"] }, { mana: "chosen-color", amount: n(1), choice: chosen("color") },
    ] } }]);
  });
  it("retains symbolic filter eligibility and leaves a later action outside the mana alternatives", () => {
    assert.deepEqual(parse("Creatures you control of the chosen color get +1/+1."), [{ kind: "static", effect: { static: "modify",
      what: { ref: "each", filter: { types: ["creature"], control: "you", chosenColor: chosen("color") } },
      modifiers: [{ modifier: "pt", power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) } }],
    } }]);
    spell("Add {R} or one mana of the chosen color, then draw a card.", [
      { effect: "add-mana", who: { player: "you" }, mana: { mana: "choice", options: [{ mana: "fixed", symbols: ["R"] }, { mana: "chosen-color", amount: n(1), choice: chosen("color") }] } },
      { effect: "draw", who: { player: "you" }, amount: n(1) },
    ]);
    for (const text of ["Add one mana of the chosen player.", "Add one mana of the chosen colors.", "Add {R}, one mana of the chosen color.",
      "Add {R} or {G}, {W}.", "Add {R} or one mana of the chosen color or.", "Add one mana of the chosen color at random."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("chosen protection qualities and honest reference status", () => {
  it("preserves distinct color, player, card-name, and card-type protection qualities", () => {
    for (const category of ["color", "player", "card-name", "card-type"] as const) {
      assert.deepEqual(parse(`Protection from the chosen ${category.replace("-", " ")}`), [{ kind: "keywords", keywords: [
        { keyword: "protection", from: [{ scope: "chosen-value", value: chosen(category) }] },
      ] }]);
    }
    assert.deepEqual(parse("Protection from the last chosen color and from red"), [{ kind: "keywords", keywords: [{ keyword: "protection", from: [
      { scope: "chosen-value", value: chosen("color", "last-chosen") }, { scope: "color", color: "red" },
    ] }] }]);
    assert.deepEqual(parse("You have protection from the chosen card name."), [{ kind: "static", effect: { static: "player-abilities", who: { player: "you" }, abilities: [
      { gained: "keyword", keyword: { keyword: "protection", from: [{ scope: "chosen-value", value: chosen("card-name") }] } },
    ] } }]);
  });
  it("retains Brave the Elements's choice, eligibility and duration as separate information", () => {
    assert.deepEqual(parse("Choose a color. White creatures you control gain protection from the chosen color until end of turn."), [{ kind: "spell", effects: [
      { steps: [{ effect: "choose-value", who: { player: "you" }, category: "color" }] },
      { steps: [{ effect: "gain-abilities", what: { ref: "each", filter: { colors: ["white"], types: ["creature"], control: "you" } }, abilities: [
        { gained: "keyword", keyword: { keyword: "protection", from: [{ scope: "chosen-value", value: chosen("color") }] } },
      ], duration: { duration: "end-of-turn" } }] },
    ] }]);
    for (const text of ["Protection from the chosen power", "Protection from the chosen colors"])
      assert.equal(parseOracleText(text).ok, false, text);
  });
  it("never reports a chosen value as bound merely because a compatible choice appears nearby", () => {
    for (const text of ["Add one mana of the chosen color.", "Choose a color. Add one mana of the chosen color.",
      "As this artifact enters, choose a color.\n{T}: Add one mana of the chosen color.", "Creatures of the last chosen color get +1/+1."]) {
      const result = parseOracleText(text, "Choice fixture", { includeSourceProvenance: true });
      assert.ok(result.ok, JSON.stringify(result.lines));
      const graph = analyzeCardBindings({ identity: "choice-fixture", source: { name: "Choice fixture", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(graph.fullyBound, false, text);
      assert.ok(graph.diagnostics.some(d => d.code === "unsupported-reference" && d.rules.includes("607.2d")), text);
    }
  });
});
