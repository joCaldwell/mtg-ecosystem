import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";

// CR 506.4-506.5, 508.1c-d, 509.1b-c, 509.1g-h, 509.4 (2026-09-25):
// requirements, restrictions, one-shot combat actions and combat-status
// filters are different things and must stay different nodes.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed", value });
const thisCreature = { ref: "this", noun: "creature" };
const creature = { types: ["creature"] };
const target = { ref: "target", filter: creature, count: fixed(1) };
const thisTurn = { duration: "this-turn" };
const endOfTurn = { duration: "end-of-turn" };

function abilities(text: string, typeLine: string): unknown[] {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.equal(result.ok, true, `${text}: ${JSON.stringify(result.lines)}`);
  return JSON.parse(JSON.stringify(result.ok ? result.abilities : []));
}
function fails(text: string, typeLine: string): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}
const steps = (text: string, typeLine = "Sorcery") =>
  (abilities(text, typeLine)[0] as { effects: { steps: unknown[] }[] }).effects.flatMap(sentence => sentence.steps);
const staticEffect = (text: string, typeLine = "Creature — Beast") =>
  (abilities(text, typeLine)[0] as { effect: unknown }).effect;

describe("block requirements", () => {
  it("keeps 'must be blocked' distinct from a requirement on every potential blocker", () => {
    // Gaea's Protector; Lure (static body on a creature, as Taunting Elf).
    assert.deepEqual(staticEffect("This creature must be blocked if able."),
      { static: "combat-rule", what: thisCreature, rule: { rule: "blocked-requirement" } });
    assert.deepEqual(staticEffect("All creatures able to block this creature do so."),
      { static: "combat-rule", what: { ref: "all", filter: creature }, rule: { rule: "block-requirement", attackers: thisCreature } });
    // Slayer's Cleaver.
    assert.deepEqual(staticEffect("Equipped creature gets +3/+1 and must be blocked by an Eldrazi if able.", "Artifact — Equipment"), {
      static: "modify", what: { ref: "equipped", noun: "creature" }, modifiers: [
        { modifier: "pt", power: { sign: 1, amount: fixed(3) }, toughness: { sign: 1, amount: fixed(1) } },
        { modifier: "combat-rule", rule: { rule: "blocked-requirement", blockers: { ref: "each", filter: { subtypes: ["Eldrazi"] } } } },
      ],
    });
  });

  it("carries the requirement's own period as the effect duration", () => {
    // Compelled Duel; Alluring Scent; Hunt Down; Culling Mark.
    assert.deepEqual(steps("Target creature gets +3/+3 until end of turn and must be blocked this turn if able."), [
      { effect: "pump", what: target, power: { sign: 1, amount: fixed(3) }, toughness: { sign: 1, amount: fixed(3) }, duration: endOfTurn },
      { effect: "combat-rule", what: target, rule: { rule: "blocked-requirement" }, duration: thisTurn },
    ]);
    assert.deepEqual(steps("All creatures able to block target creature this turn do so."), [
      { effect: "combat-rule", what: { ref: "all", filter: creature }, rule: { rule: "block-requirement", attackers: target }, duration: thisTurn },
    ]);
    assert.deepEqual(steps("Target creature blocks target creature this turn if able."), [
      { effect: "combat-rule", what: target, rule: { rule: "block-requirement", attackers: target }, duration: thisTurn },
    ]);
    assert.deepEqual(steps("Target creature blocks this turn if able."), [
      { effect: "combat-rule", what: target, rule: { rule: "block-requirement" }, duration: thisTurn },
    ]);
    // Razorgrass Screen: a static requirement has no duration.
    assert.deepEqual(staticEffect("This creature blocks each combat if able."),
      { static: "combat-rule", what: thisCreature, rule: { rule: "block-requirement" } });
  });

  it("rejects a requirement without 'if able' and an incomplete lure", () => {
    fails("This creature must be blocked.", "Creature — Beast");
    fails("Target creature blocks this turn.", "Sorcery");
    fails("All creatures able to block this creature.", "Creature — Beast");
    fails("Target creature able to block this creature does so.", "Creature — Beast");
  });
});

describe("attack requirements and alone restrictions", () => {
  it("applies a turn-long attack requirement at each combat of that turn", () => {
    // Alluring Siren; Iron Golem.
    assert.deepEqual(steps("{T}: Target creature an opponent controls attacks you this turn if able.", "Creature — Siren"), [{
      effect: "combat-rule", what: { ref: "target", filter: { types: ["creature"], control: "opponent" }, count: fixed(1) },
      rule: { rule: "attack-requirement", frequency: "each-combat", defender: { defender: "player", who: { player: "you" } } }, duration: thisTurn,
    }]);
    assert.deepEqual(staticEffect("This creature attacks or blocks each combat if able."), {
      static: "combat-rule", what: thisCreature,
      rule: { rule: "all-of", rules: [{ rule: "attack-requirement", frequency: "each-combat" }, { rule: "block-requirement" }] },
    });
    fails("Target creature attacks this turn.", "Sorcery");
  });

  it("distinguishes forbidden solo declarations from required ones", () => {
    // Mogg Flunkies; Master of Cruelties (first ability); Craven Hulk.
    assert.deepEqual(staticEffect("This creature can't attack or block alone."), { static: "combat-rule", what: thisCreature, rule: { rule: "all-of", rules: [
      { rule: "alone-restriction", action: "attack", mode: "exclude" }, { rule: "alone-restriction", action: "block", mode: "exclude" },
    ] } });
    assert.deepEqual(staticEffect("This creature can only attack alone."),
      { static: "combat-rule", what: thisCreature, rule: { rule: "alone-restriction", action: "attack", mode: "only" } });
    assert.deepEqual(staticEffect("This creature can't block alone."),
      { static: "combat-rule", what: thisCreature, rule: { rule: "alone-restriction", action: "block", mode: "exclude" } });
    // The plain prohibition remains the simple node.
    assert.deepEqual(staticEffect("This creature can't block."), { static: "cant", what: thisCreature, action: "block" });
    fails("This creature can't attack alone or.", "Creature — Beast");
  });

  it("treats attacking alone as its own status in conditions and filters", () => {
    // Dream Prowler; Crowd of True Believers (first sentence).
    assert.deepEqual(staticEffect("This creature can't be blocked as long as it's attacking alone."), {
      static: "cant", what: thisCreature, action: "be-blocked", condition: { condition: "status", what: { ref: "it" }, status: "attacking-alone" },
    });
    assert.deepEqual(steps("Target creature you control that's attacking alone gets +1/+0 until end of turn."), [{
      effect: "pump", what: { ref: "target", filter: { types: ["creature"], control: "you", status: ["attacking-alone"] }, count: fixed(1) },
      power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(0) }, duration: endOfTurn,
    }]);
  });
});

describe("coordinated prohibitions and one-shot combat actions", () => {
  it("applies an elided combat prohibition to the coordinated subject", () => {
    // Distortion Strike.
    assert.deepEqual(steps("Target creature gets +1/+0 until end of turn and can't be blocked this turn."), [
      { effect: "pump", what: target, power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(0) }, duration: endOfTurn },
      { effect: "cant", what: target, action: "be-blocked", duration: thisTurn },
    ]);
    fails("Target creature gets +1/+0 until end of turn and can't untap this turn.", "Sorcery");
  });

  it("removes from combat as a one-shot action inside a shared optional choice", () => {
    // Labyrinth of Skophos; Gustcloak Runner.
    assert.deepEqual(steps("{4}, {T}: Remove target attacking or blocking creature from combat.", "Land"), [{
      effect: "remove-from-combat", what: { ref: "target", count: fixed(1),
        filter: { allOf: [{ anyOf: [{ status: ["attacking"] }, { status: ["blocking"] }] }], types: ["creature"] } },
    }]);
    assert.deepEqual(steps("Whenever this creature becomes blocked, you may untap it and remove it from combat.", "Creature — Soldier"), [{
      effect: "optional-group", who: { player: "you" },
      steps: [{ effect: "untap", what: { ref: "it" } }, { effect: "remove-from-combat", what: { ref: "it" } }],
    }]);
    fails("Remove target attacking creature from combat until end of turn.", "Instant");
  });

  it("makes an attacker blocked by effect, but not by a named blocker", () => {
    // Fog Patch (second line of the card).
    assert.deepEqual(steps("Attacking creatures become blocked.", "Instant"),
      [{ effect: "become-blocked", what: { ref: "each", filter: { status: ["attacking"], types: ["creature"] } } }]);
    fails("Target attacking creature becomes blocked by this creature.", "Instant");
  });
});

describe("combat-status filters and recipients", () => {
  it("retains whom a creature is attacking", () => {
    // Soul Snare; Blessed Reversal.
    assert.deepEqual(steps("{W}, Sacrifice this enchantment: Exile target creature that's attacking you or a planeswalker you control.", "Enchantment"), [{
      effect: "exile", what: { ref: "target", count: fixed(1), filter: { types: ["creature"], attacking: [
        { defender: "player", who: { player: "you" } },
        { defender: "object", what: { ref: "filter", filter: { types: ["planeswalker"], control: "you" }, count: fixed(1) } },
      ] } },
    }]);
    assert.deepEqual(steps("You gain 3 life for each creature attacking you.", "Instant"), [{
      effect: "gain-life", who: { player: "you" }, amount: fixed(3),
      forEach: { types: ["creature"], attacking: [{ defender: "player", who: { player: "you" } }] },
    }]);
    // A bare plural pronoun may stand for players; it is not an object here.
    fails("Whenever you attack, each opponent loses life equal to the number of creatures attacking them.", "Enchantment");
  });

  it("keeps block history, its direction and its counterpart", () => {
    // Sizzling Barrage; Time to Reflect; Ride Down (second sentence).
    assert.deepEqual(steps("~ deals 4 damage to target creature that blocked this turn.", "Instant"), [{
      effect: "damage", source: { ref: "self" }, amount: fixed(4), to: { to: "object", ref: { ref: "target", count: fixed(1),
        filter: { types: ["creature"], combatHistory: { action: "blocked", during: "this-turn" } } } },
    }]);
    const zombie = { ref: "filter", filter: { subtypes: ["Zombie"] }, count: fixed(1) };
    assert.deepEqual(steps("Exile target creature that blocked or was blocked by a Zombie this turn.", "Instant"), [{
      effect: "exile", what: { ref: "target", count: fixed(1), filter: { types: ["creature"], anyOf: [
        { combatHistory: { action: "blocked", during: "this-turn", attacker: zombie } },
        { combatHistory: { action: "was-blocked", during: "this-turn", blocker: zombie } },
      ] } },
    }]);
    assert.deepEqual(steps("Creatures that were blocked by that creature this combat gain trample until end of turn.", "Instant"), [{
      effect: "gain-abilities", abilities: [{ gained: "keyword", keyword: { keyword: "trample" } }], duration: endOfTurn,
      what: { ref: "each", filter: { types: ["creature"],
        combatHistory: { action: "was-blocked", during: "this-combat", blocker: { ref: "that", filter: creature } } } },
    }]);
    assert.deepEqual(steps("Destroy all creatures that didn't attack this turn.", "Sorcery"), [{
      effect: "destroy", what: { ref: "all", filter: { types: ["creature"], combatHistory: { action: "attacked", during: "this-turn", negated: true } } },
    }]);
    fails("Destroy target creature that blocked.", "Instant");
  });

  it("deals damage to whatever the creature attacks without targeting", () => {
    // Scorch Spitter; Graven Abomination.
    assert.deepEqual(abilities("Whenever this creature attacks, it deals 1 damage to the player or planeswalker it's attacking.", "Creature — Lizard"), [{
      kind: "triggered", trigger: { trigger: "attacks", what: thisCreature }, effects: [{ steps: [{
        effect: "damage", source: { ref: "it" }, amount: fixed(1), to: { to: "attacked", by: { ref: "it" }, kinds: ["player", "planeswalker"] },
      }] }],
    }]);
    assert.deepEqual(steps("Whenever this creature attacks, exile target card from defending player's graveyard.", "Creature — Horror"), [{
      effect: "exile", what: { ref: "target", count: fixed(1),
        filter: { cls: "card", zone: { zone: "graveyard", owner: { player: "defending-player" } } } },
    }]);
    fails("Whenever this creature attacks, it deals 1 damage to the player or planeswalker it attacked.", "Creature — Lizard");
  });

  it("does not read the trigger's comma as the start of an attacked object", () => {
    // Chieftain en-Dal.
    assert.deepEqual(abilities("Whenever this creature attacks, attacking creatures gain first strike until end of turn.", "Creature — Knight"), [{
      kind: "triggered", trigger: { trigger: "attacks", what: thisCreature }, effects: [{ steps: [{
        effect: "gain-abilities", what: { ref: "each", filter: { status: ["attacking"], types: ["creature"] } },
        abilities: [{ gained: "keyword", keyword: { keyword: "first strike" } }], duration: endOfTurn,
      }] }],
    }]);
  });
});
