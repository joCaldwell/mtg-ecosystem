import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/parser/index.ts";

// Sticker sheets, CR 123.2-123.3c, 123.7-123.8, 107.17a: each Oracle line is
// one sticker with its ticket cost; sheets have no characteristics.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// Oracle text of Giant Mana Cake and Ancestral Hot Dog Minotaur from the
// frozen corpus.
const parse = (text: string, typeLine = "Stickers") => {
  const result = parseOracleText(text, "Test Sheet", { typeLine });
  return JSON.parse(JSON.stringify(result)) as { ok: boolean; lines: { text: string; ok: boolean; ability?: unknown; error?: string }[] };
};

describe("sticker sheets (CR 123)", () => {
  it("keeps each sticker's ticket cost with its ability or power/toughness", () => {
    const result = parse("{TK}{TK} — When this permanent leaves the battlefield, create two Food tokens. (They're artifacts with \"{2}, {T}, Sacrifice this artifact: You gain 3 life.\")\n" +
      "{TK}{TK}{TK} — Flying\n{TK}{TK}{TK}{TK}{TK} — 8/6");
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines.map(line => line.ability), [
      { kind: "sticker", ticketCost: 2, sticker: { sticker: "ability", ability: {
        kind: "triggered", trigger: { trigger: "leaves", what: { ref: "this", noun: "permanent" } },
        effects: [{ steps: [{ effect: "create-token", count: { amount: "fixed", value: 2 }, token: { predefined: "Food" } }] }],
      } } },
      { kind: "sticker", ticketCost: 3, sticker: { sticker: "ability", ability: { kind: "keywords", keywords: [{ keyword: "flying" }] } } },
      { kind: "sticker", ticketCost: 5, sticker: { sticker: "power-toughness", power: 8, toughness: 6 } },
    ]);
  });

  it("parses the abilities as abilities of the stickered permanent", () => {
    const result = parse("{TK} — {T}: Target creature gains haste until end of turn.");
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines[0].ability, { kind: "sticker", ticketCost: 1, sticker: { sticker: "ability", ability: {
      kind: "activated", costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "gain-abilities", what: { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } },
        abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }], duration: { duration: "end-of-turn" } }] }],
    } } });
  });

  it("rejects unpriced lines, spell text and unsupported sticker abilities", () => {
    assert.equal(parse("Flying").ok, false);
    assert.equal(parse("{TK}{TK} Flying").ok, false);
    assert.equal(parse("{TK}{TK} — Draw two cards.").ok, false);
    assert.equal(parse("{TK}{TK} — 3/*").ok, false);
    assert.equal(parse("{TK}{TK} — Protection from odd mana values").ok, false);
  });

  it("is only the structure of a face whose type line is Stickers", () => {
    assert.equal(parse("{TK}{TK} — Flying", "Creature — Elf").ok, false);
    assert.equal(parse("{TK}{TK} — 3/3", "Artifact").ok, false);
  });
});

// Oracle text of Aerialephant, Lineprancers, Scampire, Proficient Pyrodancer
// and Big Winner from the frozen corpus.
const CREATURE = "Creature — Elephant Performer";
const nonlandYouOwn = { ref: "filter", filter: { nonTypes: ["land"], cls: "permanent", own: "you" }, count: { amount: "fixed", value: 1 } };

describe("putting stickers on objects (CR 123.3)", () => {
  it("keeps the actor, the sticker kind, the object and optionality", () => {
    const result = parse("When this creature enters, you get {TK}, then you may put a sticker on a nonland permanent you own.", CREATURE);
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines[0].ability, {
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [
        { effect: "get-counters", who: { player: "you" }, counter: { type: "ticket" }, count: { amount: "fixed", value: 1 } },
        { effect: "put-sticker", who: { player: "you" }, sticker: "any", on: nonlandYouOwn, optional: true },
      ] }],
    });
  });

  it("distinguishes the four sticker kinds", () => {
    const kindOf = (phrase: string) => {
      const result = parse(`When this creature enters, put ${phrase} on a nonland permanent you own.`, CREATURE);
      const ability = result.lines[0].ability as { effects: { steps: { sticker?: string }[] }[] } | undefined;
      return result.ok ? ability?.effects[0].steps[0].sticker : "FAIL";
    };
    assert.equal(kindOf("a name sticker"), "name");
    assert.equal(kindOf("an ability sticker"), "ability");
    assert.equal(kindOf("an art sticker"), "art");
    assert.equal(kindOf("a power and toughness sticker"), "power-toughness");
    assert.equal(kindOf("a art sticker"), "FAIL");
    assert.equal(kindOf("two name stickers"), "FAIL");
    assert.equal(kindOf("up to two name stickers"), "FAIL");
  });

  it("filters objects by the stickers on them (CR 123.4)", () => {
    const stickered = parse("This creature has trample as long as you control a stickered permanent.", CREATURE);
    assert.equal(stickered.ok, true);
    assert.deepEqual(stickered.lines[0].ability, { kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" },
      modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "trample" } }] }],
      condition: { condition: "control", who: { player: "you" },
        what: { ref: "filter", filter: { sticker: "any", cls: "permanent" }, count: { amount: "fixed", value: 1 } } } } });
    const art = parse("{2}{R}: Another target creature with an art sticker on it gets +2/+0 until end of turn.", CREATURE);
    assert.equal(art.ok, true);
    assert.deepEqual(art.lines[0].ability, { kind: "activated", costs: [{ cost: "mana", symbols: ["2", "R"] }],
      effects: [{ steps: [{ effect: "pump", what: { ref: "target", filter: { types: ["creature"], sticker: "art", other: true }, count: { amount: "fixed", value: 1 } },
        power: { sign: 1, amount: { amount: "fixed", value: 2 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 0 } },
        duration: { duration: "end-of-turn" } }] }] });
  });

  it("rejects sticker counts and qualifiers it cannot represent", () => {
    assert.equal(parse("Enchanted creature gets +0/+2 for each name sticker on this Aura with eight or more letters.", "Enchantment — Aura").ok, false);
    assert.equal(parse("This creature has trample as long as you control a stickered player.", CREATURE).ok, false);
    assert.equal(parse("{1}: Target creature with a sticker gets +1/+1 until end of turn.", CREATURE).ok, false);
  });
});
