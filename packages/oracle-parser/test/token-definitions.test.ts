import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";

const n = (value: number) => ({ amount: "fixed", value });
function one(text: string, name?: string) {
  const parsed = parseOracleText(text, name);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
describe("a token definition includes its separately stated printed abilities", () => {
  it("keeps Elephant Resurgence's per-player tokens and intrinsic CDA in the same creation", () => {
    assert.deepEqual(one('Each player creates a green Elephant creature token. Those creatures have "This token\'s power and toughness are each equal to the number of creature cards in its controller\'s graveyard."'), {
      kind: "spell", effects: [{ steps: [{ effect: "create-token", who: { player: "each-player" }, count: n(1), token: {
        colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Elephant"], keywords: [], abilities: [{ gained: "quoted", ability: {
          kind: "static", effect: { static: "cda-pt", what: { ref: "this", noun: "token" }, stat: "both", value: {
            amount: "count", of: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: { player: "controller", of: { ref: "it" } } } },
          } },
        } }],
      } }] }],
    });
    const create = { effect: "create-token", count: n(2), token: { colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Elephant"], keywords: [{ keyword: "haste" }], power: n(3), toughness: n(3) } };
    for (const noun of ['tokens', 'creatures', 'Elephants']) assert.deepEqual(one(`Create two 3/3 green Elephant creature tokens. Those ${noun} have haste.`), {
      kind: "spell", effects: [{ steps: [create] }],
    });
    for (const clause of ['Those artifacts have haste.', 'That creature has haste.', 'Those creatures you control have haste.'])
      assert.equal(parseOracleText(`Create two 3/3 green Elephant creature tokens. ${clause}`).ok, false, clause);
    assert.equal(parseOracleText('Each player creates a 3/3 green Elephant creature token. It has haste.').ok, false);
  });
  it("puts Ajani Goldmane's characteristic ability in the created token's text", () => {
    assert.deepEqual(one('-6: Create a white Avatar creature token. It has "This token\'s power and toughness are each equal to your life total."'), {
      kind: "loyalty", cost: { sign: -1, amount: 6 }, effects: [{ steps: [{ effect: "create-token", count: n(1), token: {
        colors: ["white"], supertypes: [], types: ["creature"], subtypes: ["Avatar"], keywords: [], abilities: [{ gained: "quoted", ability: {
          kind: "static", effect: { static: "cda-pt", what: { ref: "this", noun: "token" }, stat: "both", value: { amount: "life-total", of: { player: "you" } } },
        } }],
      } }] }],
    });
  });
  it("retains the outer source capture in Saproling Burst's token characteristic ability", () => {
    assert.deepEqual(one('Create a green Saproling creature token. It has "This token\'s power and toughness are each equal to the number of fade counters on Saproling Burst."', "Saproling Burst"), {
      kind: "spell", effects: [{ steps: [{ effect: "create-token", count: n(1), token: {
        colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Saproling"], keywords: [], abilities: [{ gained: "quoted", ability: {
          kind: "static", effect: { static: "cda-pt", what: { ref: "this", noun: "token" }, stat: "both", value: { amount: "counter-count", counter: { type: "fade" }, on: { ref: "self" } } },
        } }],
      } }] }],
    });
  });
  it("retains a death trigger and its source inside the created token (Make Mischief)", () => {
    assert.deepEqual(one('Create a 1/1 red Devil creature token. It has "When this token dies, it deals 1 damage to any target."'), {
      kind: "spell", effects: [{ steps: [{ effect: "create-token", count: n(1), token: {
        colors: ["red"], supertypes: [], types: ["creature"], subtypes: ["Devil"], keywords: [], power: n(1), toughness: n(1),
        abilities: [{ gained: "quoted", ability: { kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "token" } },
          effects: [{ steps: [{ effect: "damage", source: { ref: "it" }, amount: n(1), to: { to: "any-target" } }] }],
        } }],
      } }] }],
    });
  });
  it("keeps Essence Feed's token definition after the separate life gain", () => {
    assert.deepEqual(one('Target player loses 3 life. You gain 3 life and create three 0/1 colorless Eldrazi Spawn creature tokens. They have "Sacrifice this token: Add {C}."'), {
      kind: "spell", effects: [
        { steps: [{ effect: "lose-life", who: { player: "target-player" }, amount: n(3) }] },
        { steps: [
          { effect: "gain-life", who: { player: "you" }, amount: n(3) },
          { effect: "create-token", count: n(3), token: {
            colors: [], supertypes: [], types: ["creature"], subtypes: ["Eldrazi", "Spawn"], keywords: [], power: n(0), toughness: n(1),
            abilities: [{ gained: "quoted", ability: { kind: "activated", costs: [{ cost: "sacrifice", what: { ref: "this", noun: "token" } }],
              effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "fixed", symbols: ["C"] } }] }],
            } }],
          } },
        ] },
      ],
    });
  });
  it("distinguishes a copiable haste definition from a resolving grant and a temporary has clause", () => {
    const token = { colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Saproling"], keywords: [], power: n(1), toughness: n(1) };
    const create = { effect: "create-token", count: n(1), token };
    const gain = { effect: "gain-abilities", what: { ref: "it" }, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] };
    assert.deepEqual(one("Create a 1/1 green Saproling creature token. It has haste."), {
      kind: "spell", effects: [{ steps: [{ ...create, token: { ...token, keywords: [{ keyword: "haste" }] } }] }],
    });
    assert.deepEqual(one("Create a 1/1 green Saproling creature token. It gains haste."), {
      kind: "spell", effects: [{ steps: [create] }, { steps: [gain] }],
    });
    assert.deepEqual(one("Create a 1/1 green Saproling creature token. It has haste until end of turn."), {
      kind: "spell", effects: [{ steps: [create] }, { steps: [{ ...gain, duration: { duration: "end-of-turn" } }] }],
    });
    assert.equal(parseOracleText("Create a 1/1 green Saproling creature token. It has haste and draw a card.").ok, false);
    assert.equal(parseOracleText("Create a Treasure token. It has haste.").ok, false);
    assert.equal(parseOracleText("Create a 1/1 green Saproling creature token. Put a +1/+1 counter on it. It has haste.").ok, false);
    assert.equal(parseOracleText("You may gain 1 life and create a 1/1 green Saproling creature token. It has haste.").ok, false);
    assert.equal(parseOracleText("Create a 1/1 red Goblin creature token and create a 1/1 green Saproling creature token. They have haste.").ok, false);
    assert.equal(parseOracleText("If you control a Forest, create a 1/1 green Saproling creature token. Otherwise, create a 1/1 red Goblin creature token. It has haste.").ok, false);
    assert.equal(parseOracleText("Create two 1/1 green Saproling creature tokens. It has haste.").ok, false);
    assert.equal(parseOracleText("Create a 1/1 green Saproling creature token. They have haste.").ok, false);
  });
});
