import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { LexError, lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/parser/index.ts";

// An ability quoted inside a quoted ability is typeset in single quotation
// marks (Koth of the Hammer's emblem, Reef Worm, Old-Growth Troll). The
// nested ability is an ordinary granted ability of the outer one (CR 113.1a).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const ability = (text: string, typeLine: string) => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1);
  const line = result.lines[0];
  return line.ok ? JSON.parse(JSON.stringify(line.ability)) : null;
};
const n = (value: number) => ({ amount: "fixed", value });
const puncts = (text: string) => lex(text).map(token => token.kind === "punct" ? token.value : token.kind === "word" ? token.raw : "?");
const tapDraw = { kind: "activated", costs: [{ cost: "tap-self" }],
  effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }] };
const islands = { ref: "each", filter: { subtypes: ["Island"], control: "you" } };

describe("abilities quoted inside quoted abilities", () => {
  it("lexes a free single quotation mark only inside a double-quoted ability", () => {
    assert.deepEqual(puncts(`have "Draw 'a card.'"`), ["have", '"', "Draw", "'", "a", "card", "'", '"', "."]);
    // Apostrophes and plural possessives stay part of their words.
    assert.deepEqual(lex(`"their owners' hands"`).filter(token => token.kind === "punct").map(token => token.kind === "punct" && token.value), ['"', '"']);
    assert.throws(() => lex("Draw 'a card.'"), LexError);
  });

  it("parses the nested ability as a granted ability of the quoted one", () => {
    assert.deepEqual(ability(`-8: You get an emblem with "Islands you control have '{T}: Draw a card.'"`, "Legendary Planeswalker — Test"), {
      kind: "loyalty", cost: { sign: -1, amount: 8 },
      effects: [{ steps: [{ effect: "get-emblem", who: { player: "you" }, abilities: [{ gained: "quoted", ability: {
        kind: "static", effect: { static: "modify", what: islands,
          modifiers: [{ modifier: "abilities", abilities: [{ gained: "quoted", ability: tapDraw }] }] } } }] }] }],
    });
    // The same node as one level of quotation gives the static ability itself.
    assert.deepEqual(ability(`Islands you control have "{T}: Draw a card."`, "Enchantment"), {
      kind: "static", effect: { static: "modify", what: islands,
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "quoted", ability: tapDraw }] }] },
    });
  });

  it("keeps several nested abilities and a keyword in one grant", () => {
    assert.deepEqual(ability(`-6: You get an emblem with "Islands you control have haste, '{T}: Draw a card' and '{T}: Draw a card.'"`, "Legendary Planeswalker — Test"), {
      kind: "loyalty", cost: { sign: -1, amount: 6 },
      effects: [{ steps: [{ effect: "get-emblem", who: { player: "you" }, abilities: [{ gained: "quoted", ability: {
        kind: "static", effect: { static: "modify", what: islands, modifiers: [{ modifier: "abilities", abilities: [
          { gained: "keyword", keyword: { keyword: "haste" } }, { gained: "quoted", ability: tapDraw }, { gained: "quoted", ability: tapDraw }] }] } } }] }] }],
    });
  });

  it("gives the sentence period after a nested closing quote to the enclosing sentence", () => {
    assert.deepEqual(ability(`Create a 1/1 blue Fish creature token with "When this token dies, create a 2/2 blue Whale creature token with '{T}: Draw a card.'" Draw a card.`, "Sorcery"), {
      kind: "spell", effects: [
        { steps: [{ effect: "create-token", count: n(1), token: { colors: ["blue"], supertypes: [], types: ["creature"], subtypes: ["Fish"], keywords: [],
          power: n(1), toughness: n(1), abilities: [{ gained: "quoted", ability: {
            kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "token" } },
            effects: [{ steps: [{ effect: "create-token", count: n(1), token: { colors: ["blue"], supertypes: [], types: ["creature"], subtypes: ["Whale"], keywords: [],
              power: n(2), toughness: n(2), abilities: [{ gained: "quoted", ability: tapDraw }] } }] }] } }] } }] },
        { steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] },
      ],
    });
  });

  it("fails on an unclosed or unsupported nested ability", () => {
    assert.equal(ability(`-8: You get an emblem with "Islands you control have '{T}: Draw a card."`, "Legendary Planeswalker — Test"), null);
    assert.equal(ability(`-8: You get an emblem with "Islands you control have '{T}: Frobnicate.'"`, "Legendary Planeswalker — Test"), null);
    assert.equal(ability(`-8: You get an emblem with "Islands you control have ''"`, "Legendary Planeswalker — Test"), null);
  });
});
