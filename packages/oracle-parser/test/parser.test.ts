// Structural parser tests: every assertion checks the AST content, not just
// that parsing succeeded. Texts are real oracle templates.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/index.ts";
import type { Ability } from "../src/ast.ts";

function parseOne(text: string, name?: string): Ability {
  const result = parseOracleText(text, name);
  assert.equal(result.ok, true, `parse failed: ${result.lines.find((l) => !l.ok)?.error}`);
  assert.equal(result.lines.length, 1);
  return result.lines[0].ability!;
}

describe("keyword lines", () => {
  it("parses bare keyword lists", () => {
    const a = parseOne("Flying, first strike");
    assert.deepEqual(a, {
      kind: "keywords",
      keywords: [{ keyword: "flying" }, { keyword: "first strike" }],
    });
  });

  it("parses parameterized keywords", () => {
    const a = parseOne("Ward {2}");
    assert.deepEqual(a, {
      kind: "keywords",
      keywords: [{ keyword: "ward", cost: [{ cost: "mana", symbols: ["2"] }] }],
    });
  });

  it("parses non-mana ward costs", () => {
    const a = parseOne("Ward—Pay 3 life.");
    assert.deepEqual(a, {
      kind: "keywords",
      keywords: [{ keyword: "ward", cost: [{ cost: "pay-life", amount: { amount: "fixed", value: 3 } }] }],
    });
  });

  it("parses protection and landwalk", () => {
    const a = parseOne("Protection from red, swampwalk");
    assert.deepEqual(a, {
      kind: "keywords",
      keywords: [
        { keyword: "protection", from: [{ scope: "color", color: "red" }] },
        { keyword: "landwalk", filter: { subtypes: ["Swamp"], types: ["land"] } },
      ],
    });
  });

  it("parses suspend-style number—cost keywords", () => {
    const a = parseOne("Suspend 3—{2}{U}");
    assert.deepEqual(a, {
      kind: "keywords",
      keywords: [{ keyword: "suspend", amount: 3, cost: [{ cost: "mana", symbols: ["2", "U"] }] }],
    });
  });

  it("rejects unknown keywords instead of guessing", () => {
    const result = parseOracleText("Zephyrblade, flying");
    assert.equal(result.ok, false);
  });
});

describe("activated abilities", () => {
  it("parses mana abilities", () => {
    const a = parseOne("{T}: Add {G}.");
    assert.deepEqual(a, {
      kind: "activated",
      abilityWord: undefined,
      restriction: undefined,
      costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "add-mana", mana: { mana: "fixed", symbols: ["G"] } }] }],
    });
  });

  it("parses compound costs", () => {
    const a = parseOne("{2}{B}, {T}, Sacrifice a creature: Draw a card.");
    assert.equal(a.kind, "activated");
    assert.deepEqual(a.costs, [
      { cost: "mana", symbols: ["2", "B"] },
      { cost: "tap-self" },
      {
        cost: "sacrifice",
        what: { ref: "filter", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } },
      },
    ]);
    assert.deepEqual(a.effects, [
      { steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 1 } }] },
    ]);
  });

  it("captures activation restrictions", () => {
    const a = parseOne("{T}: Draw a card. Activate only as a sorcery.");
    assert.equal(a.kind, "activated");
    assert.equal(a.restriction, "Activate only as a sorcery");
  });
});

describe("triggered abilities", () => {
  it("parses ETB draw triggers", () => {
    const a = parseOne("When Mulldrifter enters, draw two cards.", "Mulldrifter");
    assert.deepEqual(a, {
      kind: "triggered",
      abilityWord: undefined,
      condition: undefined,
      trigger: { trigger: "enters", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 2 } }] }],
    });
  });

  it("parses filtered event subjects", () => {
    const a = parseOne("Whenever another creature you control dies, draw a card.");
    assert.equal(a.kind, "triggered");
    assert.deepEqual(a.trigger, {
      trigger: "dies",
      what: {
        ref: "filter",
        filter: { types: ["creature"], other: true, control: "you" },
        count: { amount: "fixed", value: 1 },
      },
    });
  });

  it("parses cast triggers with type filters", () => {
    const a = parseOne("Whenever you cast a noncreature spell, draw a card.");
    assert.equal(a.kind, "triggered");
    assert.deepEqual(a.trigger, {
      trigger: "cast",
      who: { player: "you" },
      what: { nonTypes: ["creature"], cls: "spell" },
    });
  });

  it("parses phase triggers with turn owners", () => {
    const a = parseOne("At the beginning of your upkeep, you lose 1 life.");
    assert.equal(a.kind, "triggered");
    assert.deepEqual(a.trigger, { trigger: "phase", phase: "upkeep", whose: "your" });

    const b = parseOne("At the beginning of combat on your turn, draw a card.");
    assert.equal(b.kind, "triggered");
    assert.deepEqual(b.trigger, { trigger: "phase", phase: "combat", whose: "your" });
  });

  it("parses intervening-if clauses", () => {
    const a = parseOne("When ~ dies, if it's your turn, draw a card.");
    assert.equal(a.kind, "triggered");
    assert.deepEqual(a.condition, { condition: "your-turn" });
  });

  it("parses ability words", () => {
    const a = parseOne("Landfall — Whenever a land you control enters, draw a card.");
    assert.equal(a.kind, "triggered");
    assert.equal(a.abilityWord, "Landfall");
  });
});

describe("spell effects", () => {
  it("parses damage with any-target", () => {
    const a = parseOne("Lightning Bolt deals 3 damage to any target.", "Lightning Bolt");
    assert.deepEqual(a, {
      kind: "spell",
      abilityWord: undefined,
      effects: [
        {
          steps: [
            {
              effect: "damage",
              source: { ref: "self" },
              amount: { amount: "fixed", value: 3 },
              to: { to: "any-target" },
            },
          ],
        },
      ],
    });
  });

  it("parses destroy with filters and properties", () => {
    const a = parseOne("Destroy all creatures with power 4 or greater.");
    assert.equal(a.kind, "spell");
    assert.deepEqual(a.effects[0].steps[0], {
      effect: "destroy",
      what: {
        ref: "all",
        filter: {
          types: ["creature"],
          properties: [{ property: "power", comparison: { op: "ge", value: { amount: "fixed", value: 4 } } }],
        },
      },
    });
  });

  it("parses unless conditions", () => {
    const a = parseOne("Counter target spell unless its controller pays {2}.");
    assert.equal(a.kind, "spell");
    const s = a.effects[0];
    assert.equal(s.conditionKind, "unless");
    assert.deepEqual(s.condition, {
      condition: "pays",
      who: { player: "controller", of: { ref: "it" } },
      cost: [{ cost: "mana", symbols: ["2"] }],
    });
  });

  it("parses search-put-shuffle chains", () => {
    const a = parseOne(
      "Search your library for a basic land card, put that card onto the battlefield tapped, then shuffle.",
    );
    assert.equal(a.kind, "spell");
    const steps = a.effects[0].steps;
    assert.equal(steps.length, 3);
    assert.equal(steps[0].effect, "search");
    assert.deepEqual(steps[1], {
      effect: "move-zone",
      what: { ref: "that", filter: { cls: "card" } },
      to: { zone: "battlefield" },
      tapped: true,
    });
    assert.equal(steps[2].effect, "shuffle");
  });

  it("parses token creation", () => {
    const a = parseOne("Create two 1/1 white Soldier creature tokens with lifelink.");
    assert.equal(a.kind, "spell");
    assert.deepEqual(a.effects[0].steps[0], {
      effect: "create-token",
      count: { amount: "fixed", value: 2 },
      token: {
        power: { amount: "fixed", value: 1 },
        toughness: { amount: "fixed", value: 1 },
        colors: ["white"],
        supertypes: [],
        types: ["creature"],
        subtypes: ["Soldier"],
        keywords: [{ keyword: "lifelink" }],
      },
    });
  });

  it("parses pump with duration and elided-subject ability gain", () => {
    const a = parseOne("Target creature gets +2/+2 and gains flying until end of turn.");
    assert.equal(a.kind, "spell");
    const steps = a.effects[0].steps;
    assert.equal(steps.length, 2);
    assert.equal(steps[0].effect, "pump");
    assert.deepEqual(steps[1], {
      effect: "gain-abilities",
      what: { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } },
      abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }],
      duration: { duration: "end-of-turn" },
    });
  });

  it("parses you-may with if-you-do", () => {
    const a = parseOne("You may discard a card. If you do, draw two cards.");
    assert.equal(a.kind, "spell");
    assert.equal(a.effects.length, 2);
    assert.equal(a.effects[0].steps[0].optional, true);
    assert.deepEqual(a.effects[1].condition, { condition: "you-do" });
  });

  it("parses put-counters", () => {
    const a = parseOne("Put two +1/+1 counters on target creature.");
    assert.equal(a.kind, "spell");
    assert.deepEqual(a.effects[0].steps[0], {
      effect: "put-counters",
      counter: { type: "+1/+1" },
      count: { amount: "fixed", value: 2 },
      on: { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } },
    });
  });

  it("parses for-each riders", () => {
    const a = parseOne("You gain 1 life for each Elf you control.");
    assert.equal(a.kind, "spell");
    const step = a.effects[0].steps[0];
    assert.equal(step.effect, "gain-life");
    assert.deepEqual(step.forEach, { subtypes: ["Elf"], control: "you" });
  });

  it("parses loyalty abilities", () => {
    const a = parseOne("-3: Destroy target creature.");
    assert.equal(a.kind, "loyalty");
    assert.deepEqual(a.cost, { sign: -1, amount: 3 });

    const b = parseOne("+1: Draw a card.");
    assert.equal(b.kind, "loyalty");
    assert.deepEqual(b.cost, { sign: 1, amount: 1 });

    const z = parseOne("0: Create a Treasure token.");
    assert.equal(z.kind, "loyalty");
    assert.deepEqual(z.cost, { sign: 0, amount: 0 });
  });
});

describe("modal abilities", () => {
  it("folds bullet lines into the modal effect", () => {
    const text = "Choose one —\n• Destroy target artifact.\n• Destroy target enchantment.";
    const result = parseOracleText(text);
    assert.equal(result.ok, true, result.lines.find((l) => !l.ok)?.error ?? "parse failed");
    assert.equal(result.lines.length, 1);
    const a = result.lines[0].ability!;
    assert.equal(a.kind, "spell");
    const modal = a.effects[0].steps[0];
    assert.equal(modal.effect, "modal");
    if (modal.effect === "modal") {
      assert.deepEqual(modal.count, { exactly: 1 });
      assert.equal(modal.options.length, 2);
      assert.equal(modal.options[0][0].steps[0].effect, "destroy");
    }
  });

  it("parses modal counts", () => {
    const text = "Choose one or both —\n• Draw a card.\n• You gain 2 life.";
    const result = parseOracleText(text);
    assert.equal(result.ok, true, result.lines.find((l) => !l.ok)?.error ?? "parse failed");
    const modal = result.lines[0].ability!;
    assert.equal(modal.kind, "spell");
    const step = modal.effects[0].steps[0];
    if (step.effect === "modal") assert.deepEqual(step.count, { min: 1, max: 2 });
    else assert.fail("expected modal");
  });
});

describe("static abilities", () => {
  it("parses anthems", () => {
    const a = parseOne("Other Elves you control get +1/+1.");
    assert.equal(a.kind, "static");
    assert.deepEqual(a.effect, {
      static: "modify",
      what: { ref: "each", filter: { subtypes: ["Elf"], other: true, control: "you" } },
      modifiers: [
        {
          modifier: "pt",
          power: { sign: 1, amount: { amount: "fixed", value: 1 } },
          toughness: { sign: 1, amount: { amount: "fixed", value: 1 } },
        },
      ],
      condition: undefined,
      duration: undefined,
    });
  });

  it("parses conditional statics", () => {
    const a = parseOne("~ gets +2/+2 as long as you control an artifact.");
    assert.equal(a.kind, "static");
    if (a.effect.static !== "modify") assert.fail("expected modify");
    assert.deepEqual(a.effect.condition, {
      condition: "control",
      who: { player: "you" },
      what: { ref: "filter", filter: { types: ["artifact"] }, count: { amount: "fixed", value: 1 } },
    });
  });

  it("parses enters-tapped", () => {
    const a = parseOne("~ enters the battlefield tapped.");
    assert.deepEqual(a, { kind: "static", abilityWord: undefined, effect: { static: "enters-tapped", what: { ref: "self" } } });
    const b = parseOne("This land enters tapped.");
    assert.equal(b.kind, "static");
  });

  it("parses CDAs", () => {
    const a = parseOne("~'s power and toughness are each equal to the number of Zombies you control.");
    assert.equal(a.kind, "static");
    assert.deepEqual(a.effect, {
      static: "cda-pt",
      what: { ref: "self" },
      stat: "both",
      value: { amount: "count", of: { subtypes: ["Zombie"], control: "you" } },
    });
  });

  it("parses quoted granted abilities", () => {
    const a = parseOne('Lands you control have "{T}: Add one mana of any color."');
    assert.equal(a.kind, "static");
    if (a.effect.static !== "modify") assert.fail("expected modify");
    const gained = a.effect.modifiers[0];
    if (gained.modifier !== "abilities") assert.fail("expected abilities");
    const quoted = gained.abilities[0];
    if (quoted.gained !== "quoted") assert.fail("expected quoted");
    assert.equal(quoted.ability.kind, "activated");
  });
});

describe("multi-line cards", () => {
  it("parses full cards line by line", () => {
    const result = parseOracleText("Flying\nWhen ~ enters, draw a card.\n{T}: Add {U}.");
    assert.equal(result.ok, true);
    assert.deepEqual(
      result.abilities!.map((a) => a.kind),
      ["keywords", "triggered", "activated"],
    );
  });

  it("reports per-line failures with diagnostics", () => {
    const result = parseOracleText("Flying\nEnrapture target essence.");
    assert.equal(result.ok, false);
    assert.equal(result.lines[0].ok, true);
    assert.equal(result.lines[1].ok, false);
    assert.match(result.lines[1].error!, /expected/);
  });
});
