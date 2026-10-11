import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, DiceResultTable, Effect, ParseCardResult } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { attachDiceTable, parseDiceRange, parseDiceRoll, parseDiceTableRows } from "../src/parser/dice.ts";
import { parseSentences } from "../src/parser/effects.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseAmount, parseCondition, parseCount, parsePlayerRef } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Cached Scryfall corpus SHA256 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60:
// Contact Other Plane, Ancient Copper Dragon, Diviner's Portent, Critical Hit,
// Netherese Puzzle-Ward, Berserker's Frenzy, Fractured Powerstone, Delina.
// CR 706.1–7, 901.9: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const two = { amount: "fixed", value: 2 } as const;
const you = { player: "you" } as const;
const resultAmount = { amount: "dice-result", roll: { roll: "contextual" }, measure: "result" } as const;
const die20 = { effect: "roll-dice", who: you, die: { die: "numeric", sides: 20 }, count: one } as const;
const g = { parseAmount, parseCount, parsePlayerRef };
function parsed(text: string, name = "Fixture"): Extract<ParseCardResult, { ok: true }> {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  return result;
}
function effects(text: string): Effect[] {
  const ability = parsed(text).abilities[0];
  assert.equal(ability.kind, "spell");
  if (ability.kind !== "spell") throw new Error("expected spell");
  return ability.effects.flatMap(sentence => sentence.steps);
}
function complete<T>(parser: (c: Cursor) => T | null, text: string): T {
  const c = new Cursor(lex(text));
  const value = parser(c);
  assert.ok(value, c.errorMessage());
  assert.ok(c.done(), text);
  return value;
}
function table(lines: string[]): DiceResultTable | null {
  return parseDiceTableRows(lines.map((line, index) => lex(line, `line:${index + 1}`)), c => parseSentences(c, { modalOptions: [], allowRollAgain: true }));
}

describe("dice instructions preserve the roll and its numerical interpretation", () => {
  it("stores exact die kinds and counts without interpreting X or compact notation as sides", () => {
    for (const text of ["Roll two d4.", "Roll 2d4."])
      assert.deepEqual(effects(text), [{ effect: "roll-dice", who: you, die: { die: "numeric", sides: 4 }, count: two }]);
    assert.deepEqual(effects("Roll a six-sided die."), [{ effect: "roll-dice", who: you, die: { die: "numeric", sides: 6 }, count: one }]);
    assert.deepEqual(effects("Roll X six-sided dice."), [{ effect: "roll-dice", who: you, die: { die: "numeric", sides: 6 }, count: { amount: "x" } }]);
    assert.deepEqual(effects("Roll a 100-sided die."), [{ effect: "roll-dice", who: you, die: { die: "numeric", sides: 100 }, count: one }]);
    assert.deepEqual(parsed("{T}: Roll the planar die. Activate only as a sorcery.", "Fractured Powerstone").abilities, [{
      kind: "activated", abilityWord: undefined, costs: [{ cost: "tap-self" }], restriction: { restriction: "sorcery" },
      effects: [{ steps: [{ effect: "roll-dice", who: you, die: { die: "planar" }, count: one }] }],
    }]);
  });
  it("retains actors and coordinated optionality", () => {
    assert.deepEqual(effects("Target player rolls a d20."), [{ ...die20, who: { player: "target-player", selectionId: "line:0:0" } }]);
    assert.deepEqual(effects("Each player rolls a d20."), [{ ...die20, who: { player: "each-player" } }]);
    assert.deepEqual(effects("You may roll a d20 and draw a card."), [{ effect: "optional-group", who: you, steps: [die20, { effect: "draw", who: you, amount: one }] }]);
    assert.equal(parseOracleText("That player may roll a d20 and draw a card.").ok, false);
  });
  it("keeps adjustments inside one roll with explicit scalar scope", () => {
    const hand = { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "your" } } } as const;
    assert.deepEqual(effects("Roll a d20 and add the number of cards in your hand."), [{ ...die20,
      adjustments: [{ operation: "add", appliesTo: "single-result", amount: hand }],
    }]);
    assert.deepEqual(effects("Roll a d20 and subtract the number of cards in your hand."), [{ ...die20,
      adjustments: [{ operation: "subtract", appliesTo: "single-result", amount: hand }],
    }]);
    assert.deepEqual(effects("Roll two d20 and ignore the lower roll."), [{ ...die20, count: two, ignore: { ignore: "lowest", count: 1 } }]);
    assert.deepEqual(effects("Roll three d20 and ignore all but the highest roll."), [{ ...die20, count: { amount: "fixed", value: 3 }, ignore: { ignore: "all-but-highest" } }]);
    for (const text of ["Roll two d20 and add 3.", "Roll the planar die and add 1.", "Roll two d20 and choose which result to ignore."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
  it("preserves Ancient Copper Dragon's result-based token count", () => {
    assert.deepEqual(parsed("Whenever this creature deals combat damage to a player, roll a d20. You create a number of Treasure tokens equal to the result.", "Ancient Copper Dragon").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, combat: true, to: "any" },
      effects: [{ steps: [die20] }, { steps: [{ effect: "create-token", count: resultAmount, token: { predefined: "Treasure" } }] }],
    }]);
    assert.deepEqual(complete(parseAmount, "the total of those results"), { ...resultAmount, measure: "total" });
    assert.deepEqual(complete(parseAmount, "the result minus 1"), { amount: "minus", a: resultAmount, b: one });
    assert.deepEqual(complete(parseCondition, "the total of those results is 3 or less"), { condition: "compare", value: { ...resultAmount, measure: "total" }, comparison: { op: "le", value: { amount: "fixed", value: 3 } } });
  });
  it("rejects unspecified dice, custom dice, unrepresented choices and reroll modifiers", () => {
    for (const text of ["Roll a die.", "Roll a d0.", "Roll a d-6.", "Roll the party die.", "Roll two d6 and choose one result.", "Reroll a die.", "Roll again.", "Roll a d20. You may roll again.", "Roll a d20 and exchange its result with this creature's power.", "Roll a d20 until end of turn."])
      assert.equal(parseOracleText(text).ok, false, text);
    for (const text of ["roll a d0", "roll two d20 and add 3", "roll again"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseDiceRoll(c, g), null);
      assert.equal(c.pos, 0);
    }
  });
});

describe("dice events distinguish natural results, final results and grouped rolls", () => {
  it("preserves each occurrence versus one-or-more grouping", () => {
    for (const [event, trigger] of [
      ["a die", { trigger: "roll-dice", who: you }],
      ["one or more dice", { trigger: "roll-dice", who: you, grouping: "one-or-more" }],
      ["a six-sided die", { trigger: "roll-dice", who: you, die: { die: "numeric", sides: 6 } }],
    ] as const) {
      assert.deepEqual(parsed(`Whenever you roll ${event}, draw a card.`).abilities, [{
        kind: "triggered", abilityWord: undefined, condition: undefined, trigger,
        effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }],
      }]);
    }
  });
  it("keeps Critical Hit's natural 20 distinct from modified 20 or a highest face", () => {
    for (const [printed, result] of [
      ["a natural 20", { basis: "natural", range: { range: "exact", value: 20 } }],
      ["a 20", { basis: "final", range: { range: "exact", value: 20 } }],
      ["a 4 or higher", { basis: "final", range: { range: "at-least", value: 4 } }],
      ["a die's highest natural result", { basis: "natural", highest: true }],
    ] as const) {
      const ability = parsed(`Whenever you roll ${printed}, draw a card.`).abilities[0];
      assert.ok(ability.kind === "triggered");
      assert.deepEqual(ability.trigger, { trigger: "roll-dice", who: you, result });
    }
    assert.deepEqual(complete(parseCondition, "any of those results was 10 or higher"), {
      condition: "dice-result", roll: { roll: "contextual" }, quantifier: "any", test: { basis: "final", range: { range: "at-least", value: 10 } },
    });
    assert.deepEqual(complete(parseCondition, "you rolled 6 or higher"), {
      condition: "dice-result", roll: { roll: "contextual" }, who: you, quantifier: "single", test: { basis: "final", range: { range: "at-least", value: 6 } },
    });
  });
});

describe("results tables remain conditional parts of the original ability", () => {
  it("groups Contact Other Plane's complete table into its original spell ability", () => {
    const text = "Roll a d20.\n1—9 | Draw two cards.\n10—19 | Scry 2, then draw two cards.\n20 | Scry 3, then draw three cards.";
    const result = parsed(text, "Contact Other Plane");
    assert.equal(result.lines.length, 1);
    assert.equal(result.lines[0].text, text);
    assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{ ...die20, table: { rows: [
      { range: { range: "closed", minimum: 1, maximum: 9 }, effects: [{ steps: [{ effect: "draw", who: you, amount: two }] }] },
      { range: { range: "closed", minimum: 10, maximum: 19 }, effects: [{ steps: [{ effect: "scry", amount: two }, { effect: "draw", who: you, amount: two }] }] },
      { range: { range: "exact", value: 20 }, effects: [{ steps: [{ effect: "scry", amount: { amount: "fixed", value: 3 } }, { effect: "draw", who: you, amount: { amount: "fixed", value: 3 } }] }] },
    ] } }] }] }]);
  });
  it("preserves activated restrictions, later abilities and original row target identities", () => {
    const result = parsed("{3}{G}: Roll a d20. Activate only if this card is in your graveyard.\n1—9 | Put this card on top of your library.\n10—19 | Return this card to your hand.\n20 | Return this card to the battlefield tapped.", "Loathsome Troll");
    const ability = result.abilities[0];
    assert.equal(ability.kind, "activated");
    if (ability.kind !== "activated") throw new Error("expected activated");
    assert.deepEqual(ability.costs, [{ cost: "mana", symbols: ["3", "G"] }]);
    assert.deepEqual(ability.restriction, { restriction: "condition", condition: { condition: "is-filter", what: { ref: "this", noun: "card" }, filter: { zone: { zone: "graveyard", owner: "your" } } } });
    assert.equal(ability.effects[0].steps[0].effect, "roll-dice");
    const withTargets = parsed("Flying\nWhen this creature enters, roll a d20.\n1—19 | Destroy target artifact.\n20 | Destroy target creature.\n{T}: Draw a card.");
    assert.equal(withTargets.lines.length, 3);
    const event = withTargets.abilities[1];
    assert.ok(event.kind === "triggered");
    const roll = event.effects[0].steps[0];
    assert.ok(roll.effect === "roll-dice");
    assert.deepEqual(roll.table?.rows.map(row => row.effects[0].steps[0]), [
      { effect: "destroy", what: { ref: "target", count: one, filter: { types: ["artifact"] }, selectionId: "line:2:5" } },
      { effect: "destroy", what: { ref: "target", count: one, filter: { types: ["creature"] }, selectionId: "line:3:3" } },
    ]);
  });
  it("keeps Treasure Chest's optional destination fallback independent of artifact eligibility", () => {
    // The official ruling says an artifact may enter hand when its optional
    // battlefield move is declined; 'otherwise' does not just negate its type.
    // https://magic.wizards.com/en/news/feature/adventures-forgotten-realms-release-notes-2021-07-09
    const result = parsed("{4}, Sacrifice this artifact: Roll a d20.\n1 | Trapped! — You lose 3 life.\n2—9 | Create five Treasure tokens.\n10—19 | You gain 3 life and draw three cards.\n20 | Search your library for a card. If it's an artifact card, you may put it onto the battlefield. Otherwise, put that card into your hand. Then shuffle.", "Treasure Chest");
    const three = { amount: "fixed", value: 3 } as const;
    assert.deepEqual(result.abilities, [{ kind: "activated", abilityWord: undefined, restriction: undefined,
      costs: [{ cost: "mana", symbols: ["4"] }, { cost: "sacrifice", what: { ref: "this", noun: "artifact" } }],
      effects: [{ steps: [{ ...die20, table: { rows: [
        { range: { range: "exact", value: 1 }, label: "Trapped!", effects: [{ steps: [{ effect: "lose-life", who: you, amount: three }] }] },
        { range: { range: "closed", minimum: 2, maximum: 9 }, effects: [{ steps: [{ effect: "create-token", count: { amount: "fixed", value: 5 }, token: { predefined: "Treasure" } }] }] },
        { range: { range: "closed", minimum: 10, maximum: 19 }, effects: [{ steps: [{ effect: "gain-life", who: you, amount: three }, { effect: "draw", who: you, amount: three }] }] },
        { range: { range: "exact", value: 20 }, effects: [
          { steps: [{ effect: "search", who: you, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: { ref: "filter", filter: { cls: "card" }, count: one } }] },
          { condition: { condition: "is-filter", what: { ref: "it" }, filter: { types: ["artifact"], cls: "card" } }, conditionKind: "if",
            steps: [{ effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, optional: true }],
            otherwise: { on: "action-not-performed", steps: [{ effect: "move-zone", what: { ref: "that", filter: { cls: "card" } }, to: { zone: "hand", owner: "your" } }] } },
          { steps: [{ effect: "shuffle", who: you, library: { zone: "library", owner: you } }] },
        ] },
      ] } }] }],
    }]);
  });
  it("retains exact, bounded, open and explicitly disjoint ranges without clamping", () => {
    for (const [text, range] of [
      ["20", { range: "exact", value: 20 }], ["1—9", { range: "closed", minimum: 1, maximum: 9 }],
      ["10-19", { range: "closed", minimum: 10, maximum: 19 }], ["15+", { range: "at-least", value: 15 }],
      ["9 or less", { range: "at-most", value: 9 }], ["-2—0", { range: "closed", minimum: -2, maximum: 0 }],
      ["1 or 3", { range: "any-of", ranges: [{ range: "exact", value: 1 }, { range: "exact", value: 3 }] }],
    ] as const) assert.deepEqual(complete(parseDiceRange, text), range);
    assert.ok(table(["21+ | Draw a card."])); // Modifiers may exceed d20's natural maximum.
  });
  it("attaches completely parsed rows and preserves original source target identities", () => {
    const parsedTable = table(["1—9 | Draw two cards.", "10—19 | Scry 2, then draw two cards.", "20 | Destroy target creature."]);
    assert.deepEqual(parsedTable, { rows: [
      { range: { range: "closed", minimum: 1, maximum: 9 }, effects: [{ steps: [{ effect: "draw", who: you, amount: two }] }] },
      { range: { range: "closed", minimum: 10, maximum: 19 }, effects: [{ steps: [{ effect: "scry", amount: two }, { effect: "draw", who: you, amount: two }] }] },
      { range: { range: "exact", value: 20 }, effects: [{ steps: [{ effect: "destroy", what: { ref: "target", count: one, selectionId: "line:3:3", filter: { types: ["creature"] } } }] }] },
    ] });
    assert.ok(parsedTable);
    const header = parsed("Choose target creature, then roll a d20.").abilities[0];
    const combined = attachDiceTable(header, parsedTable);
    assert.ok(combined && "effects" in combined);
    assert.deepEqual(combined.effects[0].steps, [
      { effect: "declare-target", what: { ref: "target", count: one, selectionId: "line:0:1", filter: { types: ["creature"] } } }, { ...die20, table: parsedTable },
    ]);
    assert.equal("table" in (header as Extract<Ability, { kind: "spell" }>).effects[0].steps[1], false);
  });
  it("represents a fresh optional roll of the same table without allowing scope leakage", () => {
    const parsedTable = table(["1—14 | Draw a card.", "15+ | You may roll again."]);
    assert.ok(parsedTable);
    assert.deepEqual(parsedTable.rows[1], { range: { range: "at-least", value: 15 }, effects: [{ steps: [{ effect: "roll-again", who: you, of: { roll: "enclosing-table" }, optional: true }] }] });
    const header = parsed("Roll a d20 and add 2.").abilities[0];
    const combined = attachDiceTable(header, parsedTable);
    assert.ok(combined && "effects" in combined);
    assert.deepEqual(combined.effects[0].steps, [{ ...die20, adjustments: [{ operation: "add", appliesTo: "single-result", amount: two }], table: parsedTable }]);
    assert.equal(table(["20 | Target creature gains \"{T}: Roll again.\" until end of turn."]), null);
    assert.equal(table(["20 | At the beginning of the next end step, roll again."]), null);
  });
  it("rejects malformed, overlapping and incompletely parsed rows or header scopes", () => {
    for (const rows of [[], ["1—9 | Draw a card.", "9—20 | Draw two cards."], ["20 | Invent a result."], ["20 | X is three."], ["20 | Create one of those tokens."], ["20 | Draw a card. Unknown tail."], ["20 | Destroy everything — Draw a card."], ["20 |"], ["9—1 | Draw a card."]])
      assert.equal(table(rows), null, JSON.stringify(rows));
    const parsedTable = table(["20 | Draw a card."]);
    assert.ok(parsedTable);
    for (const header of ["Roll the planar die.", "Roll a d20. Draw a card.", "Roll a d20. Roll a d6.", "Roll two d20.", "Roll three d20 and ignore the lowest roll.", "You may roll a d20.", "Each player rolls a d20.", "Each opponent with no cards in hand rolls a d20.", "If you control a creature, roll a d20."])
      assert.equal(attachDiceTable(parsed(header).abilities[0], parsedTable), null, header);
    assert.ok(attachDiceTable(parsed("Roll two d20 and ignore the lower roll.").abilities[0], parsedTable));
    for (const text of ["20 | Draw a card.", "Draw a card.\n20 | Draw a card.", "Roll a d20.\n1—19 | Draw a card.\n20 | Unsupported action.", "Roll a d20.\n1—14 | Create a Treasure token.\n15—20 | Create one of those tokens. You may roll again.", "Roll a d20.\n9—1 | Draw a card.", "Roll two d20.\n1—20 | Draw a card."])
      assert.equal(parseOracleText(text).ok, false, text);
    assert.deepEqual(table(["1 | Bad Luck! — Draw a card."])?.rows[0].label, "Bad Luck!");
  });
  it("keeps event/result/table bindings explicitly unresolved", () => {
    const result = parsed("Roll a d20. Draw cards equal to the result.");
    const graph = analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", type_line: "Sorcery", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.message.includes("exact roll occurrence")), JSON.stringify(graph));
  });
});
