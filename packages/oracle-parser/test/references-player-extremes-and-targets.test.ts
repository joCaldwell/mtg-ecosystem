import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
function parsed(text: string, name?: string, typeLine = "Sorcery") {
  const result = parseOracleText(text, name, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

describe("the player with the most life / who controls the most", () => {
  const upkeep = { trigger: "phase", phase: "upkeep", whose: "your" };
  const self = { ref: "this", noun: "creature" };
  it("is a definite description over every player (Ghazbán Ogre)", () => {
    assert.deepEqual(parsed("At the beginning of your upkeep, if a player has more life than each other player, the player with the most life gains control of this creature.", "Ghazbán Ogre", "Creature — Ogre"),
      [{ kind: "triggered", trigger: upkeep,
        condition: { condition: "player-comparison", who: { player: "a-player" }, quantity: { quantity: "life-total" }, op: "gt", than: { player: "each-other-player" } },
        effects: [{ steps: [{ effect: "gain-control", what: self, who: { player: "definite", filter: { lifeTotal: { op: "eq",
          value: { amount: "life-total-extremum", extremum: "maximum", among: { player: "each-player" } } } } } }] }] }]);
  });
  it("measures controlled objects for each player (Wild Mammoth)", () => {
    const creatures = { types: ["creature"] };
    assert.deepEqual(parsed("At the beginning of your upkeep, if a player controls more creatures than each other player, the player who controls the most creatures gains control of this creature.", "Wild Mammoth", "Creature — Elephant"),
      [{ kind: "triggered", trigger: upkeep,
        condition: { condition: "player-comparison", who: { player: "a-player" }, quantity: { quantity: "controlled-count", of: creatures }, op: "gt", than: { player: "each-other-player" } },
        effects: [{ steps: [{ effect: "gain-control", what: self, who: { player: "definite", filter: { controls: { filter: creatures, comparison: { op: "eq",
          value: { amount: "per-player-extremum", extremum: "maximum", among: { player: "each-player" }, count: { count: "controlled", of: creatures } } } } } } }] }] }]);
  });
  it("rejects a tie clause, which admits several players", () => {
    fails("At the beginning of your upkeep, the player with the most life or tied for most life gains control of this creature.", "Creature — Ogre");
    fails("At the beginning of your upkeep, the player who controls the most creatures or is tied for the most gains control of this creature.", "Creature — Ogre");
  });
});

describe("prenominal target qualifiers", () => {
  it("reads target N/N creature as exact power and toughness (Pendelhaven)", () => {
    assert.deepEqual(parsed("{T}: Target 1/1 creature gets +1/+2 until end of turn.", "Pendelhaven", "Legendary Land"),
      [{ kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "pump",
        what: { ref: "target", count: n(1), filter: { types: ["creature"], properties: [
          { property: "power", comparison: { op: "eq", value: n(1) } }, { property: "toughness", comparison: { op: "eq", value: n(1) } }] } },
        power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(2) }, duration: { duration: "end-of-turn" } }] }] }]);
    fails("{T}: Target 1/1 artifact gets +1/+2 until end of turn.", "Legendary Land");
  });
  it("reads nonattacking/nonblocking as a negated status (Unlikely Alliance, Alarum)", () => {
    assert.deepEqual(parsed("{1}{W}: Target nonattacking, nonblocking creature gets +0/+2 until end of turn.", "Unlikely Alliance", "Enchantment"),
      [{ kind: "activated", costs: [{ cost: "mana", symbols: ["1", "W"] }], effects: [{ steps: [{ effect: "pump",
        what: { ref: "target", count: n(1), filter: { nonStatus: ["attacking", "blocking"], types: ["creature"] } },
        power: { sign: 1, amount: n(0) }, toughness: { sign: 1, amount: n(2) }, duration: { duration: "end-of-turn" } }] }] }]);
    const [alarum] = parsed("Untap target nonattacking creature. It gets +1/+3 until end of turn.", "Alarum", "Instant");
    assert.deepEqual(alarum.effects[0].steps[0], { effect: "untap", what: { ref: "target", count: n(1), filter: { nonStatus: ["attacking"], types: ["creature"] } } });
  });
});

describe("random player targets", () => {
  it("marks the target as chosen by the game (Knight Rampager, Witch Hunt)", () => {
    assert.deepEqual(parsed("When this creature dies, it deals 4 damage to target opponent chosen at random.", "Knight Rampager", "Artifact Creature — Knight"),
      [{ kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } }, effects: [{ steps: [{ effect: "damage",
        source: { ref: "it" }, amount: n(4), to: { to: "player", ref: { player: "target-opponent", random: true } } }] }] }]);
    const [hunt] = parsed("At the beginning of your end step, target opponent chosen at random gains control of this enchantment.", "Witch Hunt", "Enchantment");
    assert.deepEqual(hunt.effects[0].steps[0], { effect: "gain-control", who: { player: "target-opponent", random: true }, what: { ref: "this", noun: "enchantment" } });
  });
});

describe("no opponent controls", () => {
  it("negates the existential control condition (Kavu Runner)", () => {
    assert.deepEqual(parsed("This creature has haste as long as no opponent controls a white or blue creature.", "Kavu Runner", "Creature — Kavu"),
      [{ kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }],
        condition: { condition: "not", of: { condition: "control", who: { player: "an-opponent" }, what: { ref: "filter", count: n(1),
          filter: { allOf: [{ anyOf: [{ colors: ["white"] }, { colors: ["blue"] }] }], types: ["creature"] } } } } } }]);
  });
});

// CR 723.1: control of another player during that player's next turn.
describe("controlling another player", () => {
  it("names the controller, the controlled player and their next turn (Mindslaver, Worst Fears)", () => {
    assert.deepEqual(parsed("{4}, {T}, Sacrifice ~: You control target player during that player's next turn.", "Mindslaver", "Legendary Artifact"),
      [{ kind: "activated", costs: [{ cost: "mana", symbols: ["4"] }, { cost: "tap-self" }, { cost: "sacrifice", what: { ref: "self" } }],
        effects: [{ steps: [{ effect: "control-player", who: { player: "you" }, controlled: { player: "target-player" },
          during: { turn: "next", whose: { player: "that-player" } } }] }] }]);
    assert.deepEqual(parsed("You control target player during that player's next turn. Exile ~.", "Worst Fears")[0].effects[1],
      { steps: [{ effect: "exile", what: { ref: "self" } }] });
  });
  it("rejects a turn that is not the controlled player's own", () => {
    fails("You control target player during your next turn.");
    fails("You control target player during that player's next combat.");
  });
});

// CR 108.4: a group of targets constrained by who controls them.
describe("targets controlled by the same or different players", () => {
  const creatures = (extra: object = {}) => ({ types: ["creature"], ...extra });
  const those = { ref: "from-pool", pool: { ref: "that", filter: { types: ["creature"] }, plural: true }, count: n(1) };
  it("constrains the whole target group (Cannibalize)", () => {
    assert.deepEqual(parsed("Choose two target creatures controlled by the same player. Exile one of those creatures and put two +1/+1 counters on the other.", "Cannibalize"),
      [{ kind: "spell", effects: [
        { steps: [{ effect: "declare-target", what: { ref: "target", filter: creatures(), groupConstraints: [{ constraint: "same-controller" }], count: n(2) } }] },
        { steps: [{ effect: "exile", what: those }, { effect: "put-counters", on: { ref: "rest" }, counter: { type: "+1/+1" }, count: n(2) }] }] }]);
  });
  it("reads the same opponent as opponent control plus one controller (Retribution)", () => {
    assert.deepEqual(parsed("Choose two target creatures controlled by the same opponent. That player chooses and sacrifices one of those creatures. Put a -1/-1 counter on the other.", "Retribution"),
      [{ kind: "spell", effects: [
        { steps: [{ effect: "declare-target", what: { ref: "target", filter: creatures({ control: "opponent" }), groupConstraints: [{ constraint: "same-controller" }], count: n(2) } }] },
        { steps: [{ effect: "sacrifice", who: { player: "that-player" }, what: those, chosenBy: { player: "that-player" } }] },
        { steps: [{ effect: "put-counters", on: { ref: "rest" }, counter: { type: "-1/-1" }, count: n(1) }] }] }]);
  });
  it("keeps different controllers pairwise distinct (Run Away Together)", () => {
    assert.deepEqual(parsed("Choose two target creatures controlled by different players. Return those creatures to their owners' hands.", "Run Away Together", "Instant")[0].effects[0],
      { steps: [{ effect: "declare-target", what: { ref: "target", filter: creatures(), groupConstraints: [{ constraint: "pairwise-different", property: "controller" }], count: n(2) } }] });
  });
  it("rejects other controller groupings and a non-pool chosen sacrifice", () => {
    fails("Choose two target creatures controlled by the same team.");
    fails("Target player chooses and sacrifices a creature.");
  });
});

// Counters are not objects (CR 122.1): "those counters" can only be the
// counters the governing condition just counted, as with "all of them".
describe("remove those counters", () => {
  it("removes every counted counter of that kind (Prize Pig)", () => {
    const [trigger] = parsed("Whenever you gain life, put that many ribbon counters on this creature. Then if there are three or more ribbon counters on this creature, remove those counters and untap it.", "Prize Pig", "Creature — Boar");
    const self = { ref: "this", noun: "creature" };
    assert.deepEqual(trigger.effects[1], { steps: [
      { effect: "remove-counters", counter: { type: "ribbon" }, count: "all", from: self },
      { effect: "untap", what: { ref: "it" } }],
    condition: { condition: "is-filter", what: self, filter: { counterConstraints: [{ counter: { type: "ribbon" }, comparison: { op: "ge", value: n(3) } }] } },
    conditionKind: "if" });
  });
  it("needs a governing counter condition", () => {
    fails("Whenever you gain life, put a ribbon counter on this creature. Remove those counters and untap it.", "Creature — Boar");
    fails("Whenever you gain life, put a ribbon counter on this creature. If you control a Forest, remove those counters and untap it.", "Creature — Boar");
  });
});

// CR 608.2i: counts of completed actions this turn, as each happened.
describe("player-action history counts with they've / created", () => {
  it("keeps the contextual player of they've (Cerebral Vortex, Rug of Smothering)", () => {
    const [vortex] = parsed("Target player draws two cards, then ~ deals damage to that player equal to the number of cards they've drawn this turn.", "Cerebral Vortex", "Instant");
    assert.deepEqual(vortex.effects[0].steps[1].amount, { amount: "player-action-count", who: { player: "they" }, action: { action: "draw" }, during: "this-turn" });
    const [rug] = parsed("Whenever a player casts a spell, they lose 1 life for each spell they've cast this turn.", "Rug of Smothering", "Artifact Creature — Construct");
    assert.deepEqual(rug.effects[0].steps[0], { effect: "lose-life", who: { player: "they" }, amount: { amount: "product", factors: [n(1),
      { amount: "cast-count", who: { player: "they" }, what: { cls: "spell" }, during: "this-turn" }] } });
  });
  it("counts tokens a player created (Thalisse, Reverent Medium)", () => {
    const [trigger] = parsed("At the beginning of each end step, create X 1/1 white Spirit creature tokens with flying, where X is the number of tokens you created this turn.", "Thalisse, Reverent Medium", "Legendary Creature — Human Cleric");
    assert.deepEqual(trigger.effects[0].bindings, [{ variable: "x", value: { amount: "player-action-count", who: { player: "you" },
      action: { action: "create-token", what: { cls: "token" } }, during: "this-turn" } }]);
  });
  it("rejects created non-tokens and other periods", () => {
    fails("Draw cards equal to the number of creatures you created this turn.");
    fails("Draw cards equal to the number of tokens you created last turn.");
  });
});

// CR 608.2h-i: past-tense control is control as the named event happened.
describe("past control at a historical event", () => {
  const destroyedThisWay = { relation: "event-result", event: "destroy", scope: "this-way" };
  it("qualifies an event-result count (Kaya's Wrath, Deadly Tempest)", () => {
    assert.deepEqual(parsed("Destroy all creatures. You gain life equal to the number of creatures you controlled that were destroyed this way.", "Kaya's Wrath")[0].effects[1],
      { steps: [{ effect: "gain-life", who: { player: "you" }, amount: { amount: "count", of: { types: ["creature"], relation: destroyedThisWay, control: "you" } } }] });
    assert.deepEqual(parsed("Destroy all creatures. Each player loses life equal to the number of creatures they controlled that were destroyed this way.", "Deadly Tempest")[0].effects[1],
      { steps: [{ effect: "lose-life", who: { player: "each-player" }, amount: { amount: "count", of: { types: ["creature"], relation: destroyedThisWay, controlledBy: { player: "they" } } } }] });
  });
  it("qualifies the subject of a this-way outcome (Demonic Junker)", () => {
    const [trigger] = parsed("When this Vehicle enters, for each player, destroy up to one target creature that player controls. If a creature you controlled was destroyed this way, put two +1/+1 counters on this Vehicle.", "Demonic Junker", "Artifact — Vehicle");
    assert.deepEqual(trigger.effects[1].condition, { condition: "count", of: { types: ["creature"], control: "you", relation: destroyedThisWay }, comparison: { op: "ge", value: n(1) } });
  });
  it("rejects past control without a historical event", () => {
    fails("You gain life equal to the number of creatures you controlled.");
    fails("Destroy all creatures you controlled.");
    fails("When ~ dies, for each nontoken creature you controlled that died this turn, create a 2/2 black Zombie creature token.", "Creature — Human");
    fails("Destroy all creatures. Draw a card for each creature you controlled that was dealt damage this turn.");
  });
});

// "This process" refers back to earlier instructions of the same ability;
// their extent is left to binding.
describe("repeat this process", () => {
  it("repeats under its own condition (Hoarder's Greed)", () => {
    assert.deepEqual(parsed("You lose 2 life and draw two cards, then clash with an opponent. If you win, repeat this process.", "Hoarder's Greed"),
      [{ kind: "spell", effects: [
        { steps: [{ effect: "lose-life", who: { player: "you" }, amount: n(2) }, { effect: "draw", who: { player: "you" }, amount: n(2) },
          { effect: "clash", who: { player: "you" }, with: { player: "an-opponent" } }] },
        { steps: [{ effect: "repeat-process", process: "contextual" }], condition: { condition: "clash-result", who: { player: "you" }, result: "won" }, conditionKind: "if" }] }]);
  });
  it("keeps a stated number of repetitions (Another Round, Zimone and Dina)", () => {
    assert.deepEqual(parsed("Exile any number of creatures you control, then return them to the battlefield under their owner's control. Then repeat this process X more times.", "Another Round")[0].effects[1],
      { steps: [{ effect: "repeat-process", process: "contextual", times: { amount: "x" } }] });
    assert.deepEqual(parsed("{T}, Sacrifice another creature: Draw a card. You may put a land card from your hand onto the battlefield tapped. If you control eight or more lands, repeat this process once.", "Zimone and Dina", "Legendary Creature — Human Elf Druid")[0].effects[2].steps,
      [{ effect: "repeat-process", process: "contextual", times: n(1) }]);
  });
  it("lets the player keep repeating (Kindle the Carnage)", () => {
    assert.deepEqual(parsed("Discard a card at random. If you do, ~ deals damage equal to that card's mana value to each creature. You may repeat this process any number of times.", "Kindle the Carnage")[0].effects[2],
      { steps: [{ effect: "repeat-process", process: "contextual", times: "any-number", optional: true }] });
  });
  it("rejects repetitions that change what is repeated", () => {
    fails("Target opponent reveals their hand. That player exiles a card from it. Repeat this process until all cards in that hand have been exiled.");
    fails("Target opponent chooses a card in your graveyard. You may pay {G}. If you do, repeat this process except that opponent can't choose a card already chosen for ~.");
    fails("Target player returns a creature they control to its owner's hand, then repeats this process for an artifact.");
  });
});

// CR 118.12a: the payer's alternatives, each printed with its own verb.
describe("unless a player does one of two conjugated payments", () => {
  it("offers either payment to that player (Indulgent Tormentor)", () => {
    const [trigger] = parsed("At the beginning of your upkeep, draw a card unless target opponent sacrifices a creature of their choice or pays 3 life.", "Indulgent Tormentor", "Creature — Demon");
    assert.deepEqual(trigger.effects[0], { steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }], conditionKind: "unless",
      condition: { condition: "pays", who: { player: "target-opponent" }, cost: [{ cost: "choice", options: [
        [{ cost: "sacrifice", what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) } }], [{ cost: "pay-life", amount: n(3) }]] }] } });
  });
  it("is the same payment as the imperative alternative list", () => {
    assert.deepEqual(parsed("Each opponent loses 3 life unless that player sacrifices a nonland permanent of their choice or discards a card."),
      parsed("Each opponent loses 3 life unless that player sacrifices a nonland permanent of their choice or discard a card."));
    fails("Each opponent loses 3 life unless that player sacrifices a nonland permanent and draws a card.");
  });
});
