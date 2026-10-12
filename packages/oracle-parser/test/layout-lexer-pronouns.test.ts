import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { normalizeOracleText } from "../src/normalize.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { rewriteSourcePronouns } from "../src/source-pronouns.ts";

const NAME = "Kara, Test Hero";
const LEGEND = "Legendary Creature — Human Hero";
const normalize = (text: string, typeLine: string | undefined = LEGEND, name = NAME) =>
  normalizeOracleText(text, name, undefined, typeLine);
const ability = (text: string, typeLine: string | undefined = LEGEND) => {
  const result = parseOracleText(text, NAME, { typeLine });
  assert.equal(result.lines.length, 1);
  const line = result.lines[0];
  // Absent optional fields are printed as undefined-valued keys by some
  // productions; compare the serialized AST, as the corpus tools do.
  return line.ok ? JSON.parse(JSON.stringify(line.ability)) : null;
};

describe("gendered pronouns for a legendary source", () => {
  it("uses the neuter template when the source is the only object mentioned", () => {
    assert.deepEqual(normalize("Whenever Kara attacks, she gets +1/+0 until end of turn."),
      ["Whenever ~ attacks, it gets +1/+0 until end of turn."]);
    assert.deepEqual(normalize("When Kara dies, return her to the battlefield under her owner's control. She gains haste."),
      ["When ~ dies, return it to the battlefield under its owner's control. It gains haste."]);
    assert.deepEqual(normalize("As long as Kara has a shield counter on him, he's a 7/7 and his power can't change."),
      ["As long as ~ has a shield counter on it, it's a 7/7 and its power can't change."]);
    assert.deepEqual(normalize("{4}: Return this card from your graveyard to the battlefield with a finality counter on him."),
      ["{4}: Return this card from your graveyard to the battlefield with a finality counter on it."]);
  });

  it("produces exactly the reference the neuter wording produces", () => {
    const expected = {
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "pump", what: { ref: "it" },
        power: { sign: 1, amount: { amount: "fixed", value: 1 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 0 } },
        duration: { duration: "end-of-turn" } }] }],
    };
    assert.deepEqual(ability("Whenever Kara attacks, she gets +1/+0 until end of turn."), expected);
    assert.deepEqual(ability("Whenever Kara attacks, it gets +1/+0 until end of turn."), expected);
    assert.deepEqual(ability("When Kara dies, return her to the battlefield under her owner's control."), {
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, controller: "owner" }] }],
    });
  });

  it("names the source where another object or a printed neuter pronoun competes", () => {
    assert.deepEqual(normalize("When Kara enters, you may discard a card. If you do, put a +1/+1 counter on her."),
      ["When ~ enters, you may discard a card. If you do, put a +1/+1 counter on ~."]);
    assert.deepEqual(normalize("Whenever Kara attacks, return target creature card from your graveyard to your hand. If her sneak cost was paid, draw a card."),
      ["Whenever ~ attacks, return target creature card from your graveyard to your hand. If ~'s sneak cost was paid, draw a card."]);
    assert.deepEqual(normalize("0: Kara deals 3 damage to target creature. That creature deals damage equal to its power to her."),
      ["0: ~ deals 3 damage to target creature. That creature deals damage equal to its power to ~."]);
    assert.deepEqual(normalize("Reveal the top card of your library and put it into your hand. Kara deals damage to herself equal to that card's mana value."),
      ["Reveal the top card of your library and put it into your hand. ~ deals damage to ~ equal to that card's mana value."]);
    assert.deepEqual(ability("When Kara enters, you may discard a card. If you do, put a +1/+1 counter on her."), {
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "self" } },
      effects: [
        { steps: [{ effect: "discard", who: { player: "you" }, optional: true,
          what: { ref: "filter", filter: { cls: "card" }, count: { amount: "fixed", value: 1 } } }] },
        { condition: { condition: "you-do" }, conditionKind: "if",
          steps: [{ effect: "put-counters", on: { ref: "self" }, counter: { type: "+1/+1" }, count: { amount: "fixed", value: 1 } }] },
      ],
    });
  });

  it("leaves pronouns that are not established as the source, so the line fails", () => {
    const unchanged = (text: string, typeLine?: string, name = NAME) =>
      assert.deepEqual(normalize(text, typeLine ?? LEGEND, name), [text.replaceAll("Kara", "~")]);
    // No self-reference precedes the pronoun in this ability.
    unchanged("She gets +1/+0 until end of turn.");
    // A capitalized title or name is not a pronoun.
    unchanged("Whenever a creature dies, put a +1/+1 counter on Her Majesty.");
    unchanged("Whenever Kara attacks, She-Hulk gets +1/+0 until end of turn.");
    // "her" with an unattested continuation is neither object nor possessive.
    unchanged("Whenever Kara attacks, tap her friend.");
    // Another character on a card that names none.
    unchanged("Destroy target black creature. If that permanent was a Liliana planeswalker, her controller loses 3 life.", "Sorcery", "Liliana's Defeat");
    assert.deepEqual(normalize("Whenever Kara attacks, she gets +1/+0 until end of turn.", "Creature — Human"),
      ["Whenever ~ attacks, she gets +1/+0 until end of turn."]);
    assert.deepEqual(normalizeOracleText("Whenever Kara attacks, she gets +1/+0 until end of turn.", NAME),
      ["Whenever ~ attacks, she gets +1/+0 until end of turn."]);
    assert.equal(ability("Whenever Kara attacks, she gets +1/+0 until end of turn.", "Creature — Human"), null);
    assert.equal(ability("She gets +1/+0 until end of turn."), null);
    assert.equal(ability("Whenever Kara attacks, tap her friend."), null);
  });

  it("is all-or-nothing per line and never touches the rest of the text", () => {
    assert.equal(rewriteSourcePronouns("Whenever ~ attacks, he taps her friend."), "Whenever ~ attacks, he taps her friend.");
    assert.equal(rewriteSourcePronouns("Flying, the first strike"), "Flying, the first strike");
  });
});
