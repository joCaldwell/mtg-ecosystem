import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
const atLeastOne = { op: "ge", value: n(1) };
function parsed(text: string, name?: string, typeLine = "Sorcery") {
  const result = parseOracleText(text, name, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

// CR 400.7, 608.2c, 614.6: "is put into a graveyard / onto the battlefield
// this way" asks where the earlier instruction of the same resolution actually
// moved the object, after replacement effects; never its current zone.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("passive \"is put into <zone> this way\" outcomes", () => {
  it("tests a filtered count of the moved objects (Come Back Wrong)", () => {
    assert.deepEqual(parsed("Destroy target creature. If a creature card is put into a graveyard this way, return it to the battlefield under your control.", "Come Back Wrong"),
      [{ kind: "spell", effects: [
        { steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["creature"] }, count: n(1) } }] },
        { steps: [{ effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, controller: "you" }],
          condition: { condition: "count", of: { types: ["creature"], cls: "card",
            relation: { relation: "zone-change", to: { zone: "graveyard", owner: "any" }, scope: "this-way" } }, comparison: atLeastOne },
          conditionKind: "if" }] }]);
  });
  it("tests the referenced object itself (Werewolf Ransacker)", () => {
    assert.deepEqual(parsed("Whenever this creature transforms into ~, you may destroy target artifact. If that artifact is put into a graveyard this way, this creature deals 3 damage to that artifact's controller.", "Werewolf Ransacker", "Creature — Werewolf"),
      [{ kind: "triggered", trigger: { trigger: "transforms", what: { ref: "this", noun: "creature" }, into: { face: "source-face" } }, effects: [
        { steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["artifact"] }, count: n(1) }, optional: true }] },
        { steps: [{ effect: "damage", source: { ref: "this", noun: "creature" }, amount: n(3),
            to: { to: "player", ref: { player: "controller", of: { ref: "that", filter: { types: ["artifact"] } } } } }],
          condition: { condition: "is-filter", what: { ref: "that", filter: { types: ["artifact"] } },
            filter: { relation: { relation: "zone-change", to: { zone: "graveyard", owner: "any" }, scope: "this-way" } } },
          conditionKind: "if" }] }]);
  });
  it("covers the battlefield and exile (Vault 101: Birthday Party, Mysterious Limousine)", () => {
    const [chapter] = parsed("II, III — You may put an Aura or Equipment card from your hand or graveyard onto the battlefield. If an Equipment is put onto the battlefield this way, you may attach it to a creature you control.", "Vault 101: Birthday Party", "Enchantment — Saga");
    assert.deepEqual(chapter.effects[1].condition, { condition: "count", of: { subtypes: ["Equipment"],
      relation: { relation: "zone-change", to: { zone: "battlefield" }, scope: "this-way" } }, comparison: atLeastOne });
    const [trigger] = parsed("Whenever this Vehicle enters or attacks, exile up to one other target creature until this Vehicle leaves the battlefield. If a creature is put into exile this way, return each other card exiled with this Vehicle to the battlefield under its owner's control.", "Mysterious Limousine", "Artifact — Vehicle");
    assert.deepEqual(trigger.effects[1].condition, { condition: "count", of: { types: ["creature"],
      relation: { relation: "zone-change", to: { zone: "exile" }, scope: "this-way" } }, comparison: atLeastOne });
  });
  it("rejects a move without the this-way link, or with a turn window", () => {
    fails("Destroy target creature. If a creature card is put into a graveyard, return it to the battlefield under your control.");
    fails("Destroy target creature. If a creature card is put into a graveyard this turn, return it to the battlefield under your control.");
    fails("Destroy target creature. If a creature card is put on top of a library this way, return it to the battlefield under your control.");
  });
});

// CR 702.138c: "[This permanent] escapes with [counters]" means "If this
// permanent escaped, it enters with [those counters]" (escaped: 702.138b).
describe("this creature escapes with counters", () => {
  const self = { ref: "this", noun: "creature" };
  it("is a conditional entry with counters (Woe Strider, Ox of Agonas)", () => {
    assert.deepEqual(parsed("This creature escapes with two +1/+1 counters on it.", "Woe Strider", "Creature — Horror"),
      [{ kind: "static", effect: { static: "enters-with-counters", what: self, counter: { type: "+1/+1" }, count: n(2),
        condition: { condition: "keyword-choice", keyword: "escape", what: self }, conditionKind: "if" } }]);
    assert.deepEqual(parsed("This creature escapes with a +1/+1 counter on it.", "Ox of Agonas", "Creature — Ox"),
      [{ kind: "static", effect: { static: "enters-with-counters", what: self, counter: { type: "+1/+1" }, count: n(1),
        condition: { condition: "keyword-choice", keyword: "escape", what: self }, conditionKind: "if" } }]);
  });
  it("rejects an extra condition, a linked trigger and another subject", () => {
    fails("This creature escapes with a +1/+1 counter on it if you control a Forest.", "Creature — Ox");
    fails("This creature escapes with two +1/+1 counters on it. When it enters this way, each opponent sacrifices a non-Gorgon creature of their choice.", "Creature — Gorgon");
    fails("Target creature escapes with a +1/+1 counter on it.", "Creature — Ox");
  });
});

// CR 509.1b, 611.2a: the period may precede the exception; the rule and the
// effect's lifetime are those of "can't be blocked except by Walls this turn".
describe("can't be blocked this turn except by …", () => {
  it("is the same rule and duration as the period-last wording (Joven's Tools)", () => {
    const expected = [{ kind: "activated", costs: [{ cost: "mana", symbols: ["4"] }, { cost: "tap-self" }], effects: [{ steps: [{
      effect: "combat-rule", what: { ref: "target", filter: { types: ["creature"] }, count: n(1) },
      rule: { rule: "blocker-filter", mode: "only", blockers: { ref: "each", filter: { subtypes: ["Wall"] } } },
      duration: { duration: "this-turn" } }] }] }];
    assert.deepEqual(parsed("{4}, {T}: Target creature can't be blocked this turn except by Walls.", "Joven's Tools", "Artifact"), expected);
    assert.deepEqual(parsed("{4}, {T}: Target creature can't be blocked except by Walls this turn.", "Joven's Tools", "Artifact"), expected);
  });
  it("keeps a combat-only period and rejects a second period or a static use", () => {
    assert.deepEqual(parsed("Target creature can't be blocked this combat except by Walls.", undefined, "Instant")[0].effects[0].steps[0].duration, { duration: "this-combat" });
    fails("Target creature can't be blocked this turn except by Walls until end of turn.", "Instant");
    fails("Creatures you control can't be blocked this turn except by Walls.", "Enchantment");
  });
});

// CR 702.122b-c (crewed), 702.171c (saddled), 702.51c (convoked): creatures
// tapped to pay that cost, not creatures that are currently tapped.
describe("creatures that crewed, saddled or convoked an object", () => {
  const tapped = (action: string, of: object, period = true) => ({ types: ["creature"], relation: { relation: "tapped-for-cost", action, of, ...(period ? { period: "this-turn" } : {}) } });
  it("selects a target that crewed the Vehicle this turn (Subterranean Schooner)", () => {
    assert.deepEqual(parsed("Whenever this Vehicle attacks, target creature that crewed it this turn explores.", "Subterranean Schooner", "Artifact — Vehicle"),
      [{ kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "vehicle" } }, effects: [{ steps: [{
        effect: "explore", what: { ref: "target", filter: tapped("crew", { ref: "it" }), count: n(1) } }] }] }]);
  });
  it("counts creatures that saddled or convoked (Giant Beaver, Venerated Loxodon)", () => {
    assert.deepEqual(parsed("Whenever this creature attacks while saddled, put a +1/+1 counter on target creature that saddled it this turn.", "Giant Beaver", "Creature — Beaver Mount")[0].effects[0].steps[0].on,
      { ref: "target", filter: tapped("saddle", { ref: "it" }), count: n(1) });
    assert.deepEqual(parsed("When this creature enters, put a +1/+1 counter on each creature that convoked it.", "Venerated Loxodon", "Creature — Elephant Cleric"),
      [{ kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } }, effects: [{ steps: [{
        effect: "put-counters", counter: { type: "+1/+1" }, count: n(1), on: { ref: "each", filter: tapped("convoke", { ref: "it" }, false) } }] }] }]);
  });
  it("requires this turn for crewing and saddling, and none for convoke", () => {
    fails("Whenever this Vehicle attacks, target creature that crewed it explores.", "Artifact — Vehicle");
    fails("When this creature enters, put a +1/+1 counter on each creature that convoked it this turn.", "Creature — Elephant");
    fails("Whenever this Vehicle attacks, target creature that crewed a Vehicle this turn explores.", "Artifact — Vehicle");
  });
});

// CR 702.185c: "a spell was warped this turn" means a spell was cast for its
// warp cost this turn, by any player.
describe("a spell was warped this turn", () => {
  const warped = { condition: "cast-count", who: "all-players-combined", what: { cls: "spell", castForKeywordCost: "warp" }, comparison: atLeastOne, during: "this-turn" };
  it("is a cast count over spells cast for their warp cost (Elegy Acolyte)", () => {
    assert.deepEqual(parsed("Void — At the beginning of your end step, if a nonland permanent left the battlefield this turn or a spell was warped this turn, create a 2/2 colorless Robot artifact creature token.", "Elegy Acolyte", "Creature — Human Cleric"),
      [{ kind: "triggered", abilityWord: "Void", trigger: { trigger: "phase", phase: "end-step", whose: "your" }, condition: { condition: "any-of", conditions: [
        { condition: "zone-change-history", event: "leaves-battlefield", subject: { match: "filter", filter: { nonTypes: ["land"], cls: "permanent" }, comparison: atLeastOne },
          from: { zone: "battlefield" }, during: "this-turn" },
        warped] },
      effects: [{ steps: [{ effect: "create-token", count: n(1), token: { colors: [], supertypes: [], types: ["artifact", "creature"], subtypes: ["Robot"], keywords: [], power: n(2), toughness: n(2) } }] }] }]);
  });
  it("differs from any cast, and needs its period", () => {
    assert.deepEqual(parsed("If a spell was cast this turn, draw a card.")[0].effects[0].condition, { ...warped, what: { cls: "spell" } });
    fails("If a spell was warped, draw a card.");
    fails("If a creature was warped this turn, draw a card.");
  });
});

// CR 614.1c, 614.12a: one entry replacement; its controller chooses the
// counters before the permanent enters, and only the chosen ones are put.
describe("enters with your choice of counters", () => {
  const self = { ref: "this", noun: "creature" };
  const opt = (type: string) => ({ counter: { type }, count: n(1) });
  it("chooses one of two full options (Wingfold Pteron)", () => {
    assert.deepEqual(parsed("This creature enters with your choice of a flying counter or a hexproof counter on it.", "Wingfold Pteron", "Creature — Dinosaur"),
      [{ kind: "static", effect: { static: "enters-with-counter-choice", what: self, who: { player: "you" }, count: 1, options: [opt("flying"), opt("hexproof")] } }]);
  });
  it("shares the counter noun across a list (Denry Klin)", () => {
    assert.deepEqual(parsed("~ enters with your choice of a +1/+1, first strike, or vigilance counter on it.", "Denry Klin, Editor in Chief", "Legendary Creature — Human Advisor"),
      [{ kind: "static", effect: { static: "enters-with-counter-choice", what: { ref: "self" }, who: { player: "you" }, count: 1, options: [opt("+1/+1"), opt("first strike"), opt("vigilance")] } }]);
  });
  it("chooses two different options from among three (Grimdancer)", () => {
    assert.deepEqual(parsed("This creature enters with your choice of two different counters on it from among menace, deathtouch, and lifelink.", "Grimdancer", "Creature — Zombie"),
      [{ kind: "static", effect: { static: "enters-with-counter-choice", what: self, who: { player: "you" }, count: 2, options: [opt("menace"), opt("deathtouch"), opt("lifelink")] } }]);
  });
  it("is conditional on escaping when it escapes with the choice (Tizerus Charger)", () => {
    assert.deepEqual(parsed("This creature escapes with your choice of a +1/+1 counter or a flying counter on it.", "Tizerus Charger", "Creature — Horse"),
      [{ kind: "static", effect: { static: "enters-with-counter-choice", what: self, who: { player: "you" }, count: 1, options: [opt("+1/+1"), opt("flying")],
        condition: { condition: "keyword-choice", keyword: "escape", what: self }, conditionKind: "if" } }]);
  });
  it("rejects both counters, an unknown bare kind and a choice of as many as listed", () => {
    fails("This creature enters with your choice of a flying counter and a hexproof counter on it.", "Creature — Bird");
    fails("This creature enters with your choice of a +1/+1, oil, or vigilance counter on it.", "Creature — Bird");
    fails("This creature enters with your choice of two different counters on it from among menace and deathtouch.", "Creature — Bird");
    fails("This creature enters with your choice of a flying counter or a hexproof counter on it if you control a Bird.", "Creature — Bird");
  });
});

// CR 701.23a-b: a player who searched their library "this way" is one who
// performed the earlier search of this resolution, whatever it found.
describe("each player who searched their library this way", () => {
  it("selects the searching players for the shuffle (Noble Benefactor)", () => {
    const searchers = { player: "filtered", base: { player: "each-player" }, filter: { searched: { zone: { zone: "library", owner: "their" }, scope: "this-way" } } };
    const [ability] = parsed("When this creature dies, each player may search their library for a card and put that card into their hand. Then each player who searched their library this way shuffles.", "Noble Benefactor", "Creature — Human Cleric");
    assert.deepEqual(ability.effects[1], { steps: [{ effect: "shuffle", who: searchers, library: { zone: "library", owner: searchers } }] });
  });
  it("differs from \"who does\" and rejects other zones or no link", () => {
    const [ability] = parsed("When this creature dies, each player may search their library for a card and put that card into their hand. Then each player who does shuffles.", "Noble Benefactor", "Creature — Human Cleric");
    assert.deepEqual(ability.effects[1].steps[0].who.filter, { precedingAction: "performed" });
    fails("When this creature dies, each player may search their library for a card and put that card into their hand. Then each player who searched their library shuffles.", "Creature — Human");
    fails("When this creature dies, each player may search their library for a card and put that card into their hand. Then each player who searched your library this way shuffles.", "Creature — Human");
  });
});

// "Choose odd or even" is a value choice; "of the chosen quality" refers to
// it as a linked chosen value (CR 607.2d). Zero is even.
describe("odd or even", () => {
  const chosen = { choice: "chosen-value", category: "parity", which: "chosen" };
  it("chooses a parity and tests mana value against it (Extinction Event)", () => {
    assert.deepEqual(parsed("Choose odd or even. Exile each creature with mana value of the chosen quality.", "Extinction Event"),
      [{ kind: "spell", effects: [
        { steps: [{ effect: "choose-value", who: { player: "you" }, category: "parity" }] },
        { steps: [{ effect: "exile", what: { ref: "each", filter: { types: ["creature"], parity: { property: "mana-value", parity: chosen } } } }] }] }]);
  });
  it("tests power, \"that quality\" and a printed parity (Zimone's Hypothesis, Twins of Discord, Void Winnower)", () => {
    assert.deepEqual(parsed("You may put a +1/+1 counter on a creature. Then choose odd or even. Return each creature with power of the chosen quality to its owner's hand.", "Zimone's Hypothesis", "Instant")[0].effects[2].steps[0].what,
      { ref: "each", filter: { types: ["creature"], parity: { property: "power", parity: chosen } } });
    assert.deepEqual(parsed("Whenever you attack, choose odd or even. Creatures with mana value of that quality can't block this turn.", "Twins of Discord", "Creature — Eldrazi")[0].effects[1].steps[0].what,
      { ref: "each", filter: { types: ["creature"], parity: { property: "mana-value", parity: chosen } } });
    assert.deepEqual(parsed("Your opponents can't cast spells with even mana values.", "Void Winnower", "Creature — Eldrazi"),
      [{ kind: "static", effect: { static: "prohibit", prohibition: { prohibition: "cast", who: { player: "each-opponent" }, what: { ref: "each", filter: { cls: "spell", parity: { property: "mana-value", parity: "even" } } } } } }]);
  });
  it("rejects other qualities and a parity of toughness", () => {
    fails("Choose odd or even. Exile each creature with mana value of the chosen color.");
    fails("Choose odd or even. Exile each creature with toughness of the chosen quality.");
    fails("Choose odd or prime. Exile each creature with mana value of the chosen quality.");
  });
});

// CR 102.4: "your team" is you and/or your teammates (you, outside team games).
describe("permanents your team controls", () => {
  it("is a controller reference of its own (Rushblade Commander)", () => {
    assert.deepEqual(parsed("Warriors your team controls have haste.", "Rushblade Commander", "Creature — Azra Warrior"),
      [{ kind: "static", effect: { static: "modify", what: { ref: "each", filter: { subtypes: ["Warrior"], controlledBy: { player: "your-team" } } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }] } }]);
    assert.notDeepEqual(parsed("Warriors your team controls have haste.", undefined, "Creature — Warrior"), parsed("Warriors you control have haste.", undefined, "Creature — Warrior"));
  });
  it("rejects other team phrases", () => {
    fails("Warriors your teammates control have haste.", "Creature — Warrior");
  });
});
