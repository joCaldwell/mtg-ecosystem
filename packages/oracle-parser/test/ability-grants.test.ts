import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";

// Oracle wording checked against the frozen Scryfall corpus; rules against
// the 2026-09-25 Comprehensive Rules (113.10–11, 608.2d, 613.1f, 702.62a).
const n = (value: number) => ({ amount: "fixed", value });
const kw = (keyword: string) => ({ gained: "keyword", keyword: { keyword } });
const eot = { duration: "end-of-turn" };
const targetCreature = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
const thisCreature = { ref: "this", noun: "creature" };
const yourCreatures = { ref: "each", filter: { types: ["creature"], control: "you" } };
const theirCreatures = { ref: "each", filter: { types: ["creature"], control: "opponent" } };
function one(text: string, typeLine?: string) {
  const parsed = parseOracleText(text, undefined, { typeLine });
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.ok ? parsed.abilities[0] : null));
}
const fails = (text: string, typeLine?: string) =>
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

describe("turn-scoped continuous modifiers", () => {
  it("keeps the active-player condition around the whole modifier", () => {
    assert.deepEqual(one("During your turn, this creature has first strike.", "Creature — Cat"), {
      kind: "static", effect: { static: "conditional", condition: { condition: "your-turn" },
        effect: { static: "modify", what: thisCreature, modifiers: [{ modifier: "abilities", abilities: [kw("first strike")] }] } },
    });
    assert.deepEqual(one("During turns other than yours, this creature gets +0/+2.", "Creature — Cat"), {
      kind: "static", effect: { static: "conditional", condition: { condition: "not-your-turn" },
        effect: { static: "modify", what: thisCreature, modifiers: [{ modifier: "pt", power: { sign: 1, amount: n(0) }, toughness: { sign: 1, amount: n(2) } }] } },
    });
  });
  it("does not extend the prefix to prevention or other static families", () => {
    fails("During your turn, prevent all damage that would be dealt to you.", "Enchantment");
    fails("During each turn, this creature has first strike.", "Creature — Cat");
  });
  it("ends a condition's noun phrase at a comma that has no conjunction", () => {
    assert.deepEqual(one("As long as you control an Island, creatures you control have flying.", "Enchantment"), {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "control", who: { player: "you" }, what: { ref: "filter", filter: { subtypes: ["Island"] }, count: n(1) } },
        effect: { static: "modify", what: yourCreatures, modifiers: [{ modifier: "abilities", abilities: [kw("flying")] }] } },
    });
    fails("Destroy target artifact, creature.", "Sorcery");
  });
});

describe("keyword choices and abilities of other objects", () => {
  it("keeps chooser, count and every option of a resolution-time choice", () => {
    assert.deepEqual(one("Target creature gains your choice of lifelink or indestructible until end of turn.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "gain-abilities", what: targetCreature, duration: eot, abilities: [{
        gained: "keyword-choice", who: { player: "you" }, count: n(1), when: "granting-effect-applies",
        options: [{ keyword: "lifelink" }, { keyword: "indestructible" }],
      }] }] }],
    });
  });
  it("rejects a conjunction, a static choice and a choice in a token definition", () => {
    fails("Target creature gains your choice of flying and haste until end of turn.", "Instant");
    fails("This creature has your choice of flying or haste.", "Creature — Cat");
    fails("Create a 1/1 red Goblin creature token with your choice of flying or haste.", "Sorcery");
  });
  it("grants the stated kinds of abilities of the referenced objects", () => {
    assert.deepEqual(one("This creature has all activated abilities of all legendary creatures you control.", "Creature — Human"), {
      kind: "static", effect: { static: "modify", what: thisCreature, modifiers: [{ modifier: "abilities", abilities: [{
        gained: "abilities-of", kinds: ["activated"], of: { ref: "all", filter: { supertypes: ["legendary"], types: ["creature"], control: "you" } },
      }] }] },
    });
    assert.deepEqual(one("~ has all activated abilities of lands your opponents control except mana abilities.", "Legendary Creature — Avatar").effect.modifiers, [
      { modifier: "abilities", abilities: [{ gained: "abilities-of", kinds: ["activated"], of: { ref: "each", filter: { types: ["land"], control: "opponent" } }, except: "mana-abilities" }] },
    ]);
    assert.deepEqual(one("Target creature gains all activated abilities of target creature until end of turn.", "Instant").effects[0].steps[0].abilities,
      [{ gained: "abilities-of", kinds: ["activated"], of: targetCreature }]);
    fails("This creature has all activated abilities.", "Creature — Human");
    fails("This creature has all loyalty abilities of all planeswalkers except mana abilities.", "Creature — Human");
  });
});

describe("keyword costs defined by a following sentence", () => {
  const cards = { cls: "card", zone: { zone: "graveyard", owner: "your" } };
  it("attaches the recipient's mana cost to the resolving grant", () => {
    assert.deepEqual(one("Target sorcery card in your graveyard gains flashback until end of turn. The flashback cost is equal to its mana cost.", "Sorcery"), {
      kind: "spell", effects: [{ steps: [{ effect: "gain-abilities", what: { ref: "target", filter: { types: ["sorcery"], ...cards }, count: n(1) },
        abilities: [{ gained: "keyword", keyword: { keyword: "flashback" }, costDefinition: { manaCostOf: "each-recipient" } }], duration: eot }] }],
    });
  });
  it("keeps additional components and reductions of a static grant's cost", () => {
    assert.deepEqual(one("Each nonland card in your graveyard has escape. The escape cost is equal to the card's mana cost plus exile three other cards from your graveyard.", "Enchantment"), {
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { nonTypes: ["land"], ...cards } }, modifiers: [{ modifier: "abilities", abilities: [{
        gained: "keyword", keyword: { keyword: "escape" }, costDefinition: { manaCostOf: "each-recipient",
          additional: [{ cost: "exile", what: { ref: "filter", filter: { other: true, ...cards }, count: n(3) } }] },
      }] }] },
    });
    assert.deepEqual(one("Each enchantment card in your hand has miracle. Its miracle cost is equal to its mana cost reduced by {4}.", "Legendary Creature — Human").effect.modifiers[0].abilities,
      [{ gained: "keyword", keyword: { keyword: "miracle" }, costDefinition: { manaCostOf: "each-recipient", reducedBy: ["4"] } }]);
  });
  it("never grants a cost keyword without its cost", () => {
    fails("Target sorcery card in your graveyard gains flashback until end of turn.", "Sorcery");
    fails("Target sorcery card in your graveyard gains flashback until end of turn. The escape cost is equal to its mana cost.", "Sorcery");
    fails("Each creature card in your graveyard has scavenge. The scavenge cost is equal to your life total.", "Creature — Troll");
    fails("Draw a card. The flashback cost is equal to its mana cost.", "Sorcery");
  });
  it("grants bare suspend, but not after an unrepresented exile-with-counters", () => {
    assert.deepEqual(one("Exile target creature and put two time counters on it. If it doesn't have suspend, it gains suspend.", "Instant").effects[1], {
      steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: [kw("suspend")] }],
      condition: { condition: "not", of: { condition: "has-keyword", what: { ref: "it" }, keyword: "suspend" } }, conditionKind: "if",
    });
    fails("Exile target creature with two time counters on it. If it doesn't have suspend, it gains suspend.", "Instant");
  });
});

describe("ability prohibition and removal of other abilities", () => {
  it("represents \"can't have\" separately from losing the ability (CR 113.11)", () => {
    assert.deepEqual(one("Creatures your opponents control lose flying and can't have or gain flying.", "Enchantment Creature — Human Wizard"), {
      kind: "static", effect: { static: "modify", what: theirCreatures, modifiers: [
        { modifier: "lose-abilities", removal: { removal: "listed", abilities: [kw("flying")] } },
        { modifier: "cant-have-abilities", abilities: [{ keyword: "flying" }] },
      ] },
    });
    assert.deepEqual(one("{1}, {T}: Until end of turn, creatures your opponents control lose hexproof and shroud and can't have hexproof or shroud.", "Land").effects, [{ steps: [
      { effect: "lose-abilities", what: theirCreatures, removal: { removal: "listed", abilities: [kw("hexproof"), kw("shroud")] }, duration: eot },
      { effect: "cant-have-abilities", what: theirCreatures, abilities: [{ keyword: "hexproof" }, { keyword: "shroud" }], duration: eot },
    ] }]);
    fails("Creatures your opponents control can't have abilities.", "Enchantment");
  });
  it("spares only the abilities granted by the same static effect", () => {
    assert.deepEqual(one("Enchanted creature has base power and toughness 0/2, has defender, and loses all other abilities.", "Enchantment — Aura"), {
      kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [
        { modifier: "base-pt", power: n(0), toughness: n(2) },
        { modifier: "abilities", abilities: [kw("defender")] },
        { modifier: "lose-abilities", removal: { removal: "all-other", than: "abilities-granted-by-this-effect" } },
      ] },
    });
    fails("Enchanted creature loses all other abilities.", "Enchantment — Aura");
    fails("Target creature loses all other abilities until end of turn.", "Instant");
  });
});

describe("grant shapes around quoted abilities and plural subjects", () => {
  it("gives a comma or period inside the closing quote to the enclosing sentence", () => {
    assert.deepEqual(one('Equipped creature gets +1/+1, has "Whenever this creature deals combat damage to a player, draw a card," and is a Rogue in addition to its other types.', "Artifact — Equipment"), {
      kind: "static", effect: { static: "modify", what: { ref: "equipped", noun: "creature" }, modifiers: [
        { modifier: "pt", power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) } },
        { modifier: "abilities", abilities: [{ gained: "quoted", ability: { kind: "triggered",
          trigger: { trigger: "deals-damage", what: thisCreature, combat: true, to: "any" },
          effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }] } }] },
        { modifier: "types", change: { subtypes: [{ family: "creature", operation: "add", types: ["Rogue"] }] } },
      ] },
    });
    assert.deepEqual(one('Create X 1/1 black Rat creature tokens with "This token can\'t block." Creatures you control gain haste until end of turn.', "Sorcery"), {
      kind: "spell", effects: [
        { steps: [{ effect: "create-token", count: { amount: "x" }, token: { colors: ["black"], supertypes: [], types: ["creature"], subtypes: ["Rat"], keywords: [],
          power: n(1), toughness: n(1), abilities: [{ gained: "quoted", ability: { kind: "static", effect: { static: "cant", what: { ref: "this", noun: "token" }, action: "block" } } }] } }] },
        { steps: [{ effect: "gain-abilities", what: yourCreatures, abilities: [kw("haste")], duration: eot }] },
      ],
    });
  });
  it("leaves a quoted period alone when the sentence continues", () => {
    const lasting = one('Target creature gains "{T}: Draw a card." until end of turn.', "Instant");
    assert.deepEqual(lasting.effects.length, 1);
    assert.deepEqual(lasting.effects[0].steps[0].duration, eot);
    // "Then" stays the existing same-sentence continuation of the instruction.
    const then = one('Create a 1/1 black Rat creature token with "This token can\'t block." Then draw a card.', "Sorcery");
    assert.deepEqual(then.effects.length, 1);
    assert.deepEqual(then.effects[0].steps.map((step: { effect: string }) => step.effect), ["create-token", "draw"]);
  });
  it("reads distributive each and additive also without changing the modifier", () => {
    assert.deepEqual(one("Two target creatures each get -1/-1 until end of turn.", "Sorcery"), {
      kind: "spell", effects: [{ steps: [{ effect: "pump", what: { ref: "target", filter: { types: ["creature"] }, count: n(2) },
        power: { sign: -1, amount: n(1) }, toughness: { sign: -1, amount: n(1) }, duration: eot }] }],
    });
    assert.deepEqual(one("Target creature gets +2/+2 until end of turn. If it's a Goblin, it also gains haste until end of turn.", "Instant").effects[1], {
      steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: [kw("haste")], duration: eot }],
      condition: { condition: "is-filter", what: { ref: "it" }, filter: { subtypes: ["Goblin"] } }, conditionKind: "if",
    });
    fails("Target creature each gets +1/+1 until end of turn.", "Instant");
    fails("Target creature also gets +1/+1 until end of turn.", "Instant");
  });
});
