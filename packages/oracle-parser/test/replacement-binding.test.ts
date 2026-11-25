import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ReplacementInstruction } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard } from "../src/semantic/resolution.ts";

const exileGraveyard = "If a card or token would be put into a graveyard from anywhere, exile it instead.";
const damageCounters = "If a source you control would deal noncombat damage to a creature an opponent controls, put that many -1/-1 counters on that creature instead.";
const yamabushi = "Yamabushi's Flame deals 3 damage to any target. If a creature dealt damage this way would die this turn, exile it instead.";
const whip = "Return target creature card from your graveyard to the battlefield. It gains haste. Exile it at the beginning of the next end step. If it would leave the battlefield, exile it instead of putting it anywhere else.";
const skipExtra = "If an opponent would begin an extra turn, that player skips that turn instead.";
const doubleProliferate = "If you would proliferate, proliferate twice instead.";
const exileOperand = "/replace/effects/0/steps/0/what";
const counterOperand = "/replace/effects/0/steps/0/on";

function inspect(name: string, typeLine: string, text: string, edit?: (abilities: Ability[]) => void) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  edit?.(result.abilities);
  const input = { identity: "replacement-fixture", source: { name, layout: "normal", type_line: typeLine },
    faces: [{ sourceFaceIndex: 0, result }] };
  const graph = analyzeCardBindings(input);
  const resolution = resolveCard(input, graph);
  const blocks = resolution.faces.flatMap(face => face.blocks ?? []);
  const ref = (suffix: string) => blocks.flatMap(block => block.refs).find(ref => ref.path.endsWith(suffix));
  return {
    ref,
    residual: (suffix: string) => {
      const entry = ref(suffix);
      return entry && "unresolved" in entry ? entry.unresolved : undefined;
    },
  };
}

function replaceStatic(edit: (replacement: ReplacementInstruction) => ReplacementInstruction) {
  return (abilities: Ability[]) => {
    const [ability] = abilities;
    assert.ok(ability.kind === "static" && ability.effect.static === "replacement");
    ability.effect.replacement = edit(ability.effect.replacement);
  };
}

function replaceSpellSteps(edit: (prior: Effect, replacement: Effect) => [Effect, Effect]) {
  return (abilities: Ability[]) => {
    const [ability] = abilities;
    assert.equal(ability.kind, "spell");
    if (ability.kind !== "spell") return;
    const [prior, replacement] = ability.effects;
    [prior.steps[0], replacement.steps[0]] = edit(prior.steps[0], replacement.steps[0]);
  };
}

// Frozen Oracle examples and CR 400.6, 120.4b, 608.2c, 614.1a–b and 614.6.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("replacement participant binding", () => {
  it("matches one moving object even when its category is a card or token", () => {
    const bind = (edit?: (abilities: Ability[]) => void) => inspect("Rest in Peace", "Enchantment", exileGraveyard, edit);
    assert.deepEqual(bind().ref(exileOperand), {
      path: "/effect/replacement" + exileOperand, ref: { ref: "event-object", role: "proposed-move-object" },
    });
    const group = bind(replaceStatic(rule => {
      assert.equal(rule.event.event, "move-zone");
      return { ...rule, event: { ...rule.event, what: { ref: "all", filter: { cls: "card" } } } };
    }));
    const destruction = bind(replaceStatic(rule => {
      assert.equal(rule.event.event, "move-zone");
      return { ...rule, event: { event: "destroy", what: rule.event.what } };
    }));
    for (const fixture of [group, destruction]) assert.ok(fixture.residual(exileOperand));
  });

  it("binds the single damage recipient but keeps the proposed amount unresolved", () => {
    const bind = (edit?: (abilities: Ability[]) => void) => inspect("Soul-Scar Mage", "Creature", damageCounters, edit);
    const fixture = bind();
    assert.deepEqual(fixture.ref(counterOperand), {
      path: "/effect/replacement" + counterOperand, ref: { ref: "event-object", role: "proposed-damage-recipient" },
    });
    assert.equal(fixture.residual("/replace/effects/0/steps/0/count"), "unsupported-number-binding");
    const two = bind(replaceStatic(rule => {
      assert.equal(rule.event.event, "damage");
      const [creature] = rule.event.to ?? [];
      assert.ok(creature);
      return { ...rule, event: { ...rule.event, to: [creature, { player: "an-opponent" }] } };
    }));
    assert.ok(two.residual(counterOperand));
  });

  it("captures the creature actually dealt damage by the sole preceding damage instruction", () => {
    const bind = (edit?: (abilities: Ability[]) => void) => inspect("Yamabushi's Flame", "Instant", yamabushi, edit);
    const capture = bind().ref("/replacement/event/what");
    assert.deepEqual(capture, {
      path: "/effects/1/steps/0/replacement/event/what", ref: { ref: "step-result", step: "/effects/0/steps/0", role: "objects" },
    });
    const two = bind(replaceSpellSteps((prior, replacement) => {
      assert.equal(prior.effect, "damage");
      const target = (selectionId: string) => ({ ref: "target" as const, filter: { types: ["creature" as const] },
        count: { amount: "fixed" as const, value: 1 }, selectionId });
      return [{ ...prior, to: { to: "each", refs: [target("one"), target("two")] } }, replacement];
    }));
    const nested = bind(replaceSpellSteps((prior, replacement) => [prior,
      { effect: "optional-group", who: { player: "you" }, steps: [replacement] }]));
    assert.ok(two.residual(exileOperand));
    assert.ok(nested.residual(exileOperand));
  });

  it("uses the same one returned incarnation as the preceding delayed exile", () => {
    const fixture = inspect("Whip of Erebos", "Artifact", "{T}: " + whip);
    assert.deepEqual(fixture.ref("/replacement/event/what"), {
      path: "/effects/3/steps/0/replacement/event/what", ref: { ref: "step-result", step: "/effects/0/steps/0", role: "objects" },
    });
    const group = inspect("Whip of Erebos", "Artifact", "{T}: " + whip, ([ability]) => {
      assert.equal(ability.kind, "activated");
      const move = ability.effects[0].steps[0];
      assert.equal(move.effect, "move-zone");
      assert.equal(move.what.ref, "target");
      move.what.count = { amount: "fixed", value: 2 };
    });
    assert.ok(group.residual(exileOperand));
  });

  it("keeps a granted replacement and conditional payment branches outside the new scopes", () => {
    const quoted = inspect("Rest in Peace", "Enchantment", exileGraveyard, abilities => {
      const [replacement] = abilities;
      abilities[0] = { kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{
        effect: "gain-abilities", what: { ref: "self" }, abilities: [{ gained: "quoted", ability: replacement }],
      }] }] };
    });
    assert.ok(quoted.residual(exileOperand));

    const lotus = inspect("Lotus Vale", "Land", "If this land would enter, sacrifice two untapped lands instead. If you do, put this land onto the battlefield. If you don't, put it into its owner's graveyard.\n{T}: Add three mana of any one color.");
    assert.ok(lotus.residual("/replace/effects/1/steps/0/what"));
    assert.ok(lotus.residual("/replace/effects/2/steps/0/what"));
  });

  it("binds a mandatory extra-turn skip and the controller's two proliferations", () => {
    const standing = inspect("Stranglehold", "Enchantment", skipExtra);
    assert.deepEqual(standing.ref("/event/who"), { path: "/effect/replacement/event/who", evaluated: "selection" });
    const optional = inspect("Fasting", "Enchantment", "If you would begin your draw step, you may skip that step instead. If you do, you gain 2 life.");
    assert.ok(optional.residual("/event/who"));
    const bind = (edit?: (abilities: Ability[]) => void) => inspect("Tekuthal, Inquiry Dominus", "Creature", doubleProliferate, edit);
    assert.deepEqual(bind().ref("/replace/effects/0/steps/0/who"), {
      path: "/effect/replacement/replace/effects/0/steps/0/who", ref: { ref: "controller", role: "static-controller" },
    });
    const another = bind(replaceStatic(rule => ({ ...rule, event: { event: "proliferate", who: { player: "an-opponent" } } })));
    assert.ok(another.residual("/replace/effects/0/steps/0/who"));
  });
});
