import assert from "node:assert/strict";
import { it } from "node:test";
import { parseOracleText } from "../src/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard } from "../src/semantic/resolution.ts";
import type { BindingNode, CardBindingAnalysis } from "../src/semantic/types.ts";

// Real card fixtures use frozen Scryfall corpus f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738.
// CR 101.4, 118.12, 601.2c, 603.12 and 608.2c–f:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function card(name: string, type_line: string, text: string) {
  const result = parseOracleText(text, name, { includeSourceProvenance: true, typeLine: type_line });
  assert.ok(result.ok, `${name}: ${JSON.stringify(result.lines.filter(line => !line.ok))}`);
  const input = { identity: "fixture", source: { name, type_line, layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] };
  const graph = analyzeCardBindings(input);
  return { graph, resolution: resolveCard(input, graph) };
}

function node<K extends BindingNode["node"]>(graph: CardBindingAnalysis, kind: K, path: string): Extract<BindingNode, { node: K }> {
  const value = graph.nodes.find(entry => entry.node === kind && entry.path.endsWith(path));
  assert.ok(value, `Missing ${kind} at ${path}`);
  return value as Extract<BindingNode, { node: K }>;
}

function target(graph: CardBindingAnalysis, path: string): string {
  const ref = graph.references.find(entry => entry.path.endsWith(path));
  assert.ok(ref?.binding === "bound", `${path}: ${JSON.stringify(ref)}`);
  return ref.target;
}

function residual(result: ReturnType<typeof card>, path: string, message: string) {
  assert.equal(result.resolution.complete, false);
  assert.ok(result.graph.diagnostics.some(d => d.path.endsWith(path) && d.message.startsWith(message)),
    JSON.stringify(result.graph.diagnostics));
}

it("B14 binds Energy Tap's condition to its sole mandatory tap cost", () => {
  const text = "Tap target untapped creature you control. If you do, add an amount of {C} equal to that creature's mana value.";
  const { graph, resolution } = card("Energy Tap", "Sorcery", text);
  assert.equal(resolution.complete, true);
  // CR 118.12: "if you do" asks whether the tap cost was started, so the
  // condition is the cost condition on that tap, never a plain event link.
  const cost = node(graph, "action-cost-condition", "/effects/1/condition");
  assert.equal(cost.event, node(graph, "event", "/effects/0/steps/0").id);
  assert.equal(cost.action, "tap");
  assert.ok(!graph.nodes.some(n => n.node === "event-link" && n.path.endsWith("/effects/1/condition")));
  assert.equal(target(graph, "/amount/of"), target(graph, "/effects/0/steps/0/what"));
  const twoTaps = text.replace("you control.", "you control, then tap target untapped creature you control.");
  residual(card("Two taps", "Sorcery", twoTaps), "/condition", "Payment/performance conditions need");
});

it("B14 watches the source's exact sacrifice event with a separate reflexive ability", () => {
  const text = "When this land enters, sacrifice it. When you do, search your library for a basic Swamp, Mountain, or Forest card, put it onto the battlefield tapped, then shuffle and you gain 1 life.";
  const { graph, resolution } = card("Riveteers Overlook", "Land", text);
  assert.equal(resolution.complete, true);
  const sacrifice = node(graph, "event", "/effects/0/steps/0");
  const link = node(graph, "event-link", "/effects/1/steps/0/event");
  assert.equal(link.event, sacrifice.id);
  assert.equal(link.scope, sacrifice.scope);
  const reflexive = node(graph, "ability", "/effects/1/steps/0");
  assert.equal(reflexive.abilityKind, "reflexive-triggered");
  assert.equal(node(graph, "event", "/effects/1/steps/0/effects/0/steps/0").scope, reflexive.id);

  const mill = "When this creature enters, if you cast it, mill four cards. When you do, return target creature card with mana value 3 or less from your graveyard to the battlefield.";
  residual(card("Yathan Roadwatcher", "Creature — Human Scout", mill), "/event", "Payment/performance conditions need");
  residual(card("Other entrant", "Creature", "Whenever another creature enters, sacrifice it. When you do, draw a card."),
    "/event", "Payment/performance conditions need");
  residual(card("Other actor", "Land", "When this land enters, target opponent sacrifices a land. When you do, draw a card."),
    "/event", "Payment/performance conditions need");
  residual(card("Nested choice", "Land", "When this land enters, sacrifice it or draw a card. When you do, draw a card."),
    "/event", "Payment/performance conditions need");
  residual(card("Quoted", "Enchantment", 'Lands you control have "When this land enters, sacrifice it. When you do, draw a card."'),
    "/event", "Payment/performance conditions need");
});

it("B14 links an optional action through the path of its performed event", () => {
  const { graph, resolution } = card("Optional sacrifice", "Sorcery", "You may sacrifice a creature. If you do, draw two cards.");
  assert.equal(resolution.complete, true);
  const event = node(graph, "event", "/effects/0/steps/0");
  const link = node(graph, "decision-link", "/effects/1/condition");
  assert.equal(event.availability, "when-performed");
  assert.equal(event.optional, true);
  assert.equal(link.decision, event.path);
  assert.equal(link.condition, "you-do");
});

it("B17 binds Smothering Tithe's payment and decline check to the drawing opponent", () => {
  const text = `Whenever an opponent draws a card, that player may pay {2}. If the player doesn't, you create a Treasure token. (It's an artifact with "{T}, Sacrifice this token: Add one mana of any color.")`;
  const { graph, resolution } = card("Smothering Tithe", "Enchantment", text);
  assert.equal(resolution.complete, true);
  const event = node(graph, "event", "/effects/0/steps/0");
  const decision = node(graph, "decision-link", "/effects/1/condition");
  assert.equal(event.operation, "pay-cost");
  assert.equal(event.optional, true);
  assert.equal(decision.decision, event.path);
  assert.equal(decision.condition, "you-dont");
  assert.equal(target(graph, "/effects/1/condition/who"), event.decider);
  assert.equal(target(graph, "/effects/0/steps/0/costs/0/payer"), event.decider);
  assert.notEqual(target(graph, "/effects/1/steps/0/who"), event.decider);
  const otherPayer = text.replace("that player may", "you may");
  residual(card("Other payer", "Enchantment", otherPayer), "/effects/1/condition", "This condition queries earlier events");
  const tap = "Vigilance\nWhenever an artifact or creature an opponent controls enters untapped, they may tap that permanent. If they don't, you create a 1/1 white Vampire creature token with lifelink.";
  residual(card("Charismatic Conqueror", "Creature — Vampire Soldier", tap), "/effects/1/condition", "This condition queries earlier events");
});

it("B23 keeps Ancient Animus's fight unconditional and uses its announced target", () => {
  const text = "Put a +1/+1 counter on target creature you control if it's legendary. Then it fights target creature an opponent controls. (Each deals damage equal to its power to the other.)";
  const { graph, resolution } = card("Ancient Animus", "Instant", text);
  assert.equal(resolution.complete, true);
  assert.equal(node(graph, "event", "/effects/0/steps/0").conditional, true);
  assert.equal(node(graph, "event", "/effects/1/steps/0").conditional, undefined);
  assert.equal(target(graph, "/effects/1/steps/0/a"), target(graph, "/effects/0/steps/0/on"));
  const neyith = "Whenever one or more creatures you control fight or become blocked, draw a card.\nAt the beginning of combat on your turn, you may pay {2}{R/G}. If you do, double target creature's power until end of turn. That creature must be blocked this combat if able. ({R/G} can be paid with either {R} or {G}.)";
  residual(card("Neyith of the Dire Hunt", "Legendary Creature — Human Warrior", neyith),
    "/effects/2", "This sentence refers back to a conditional sentence");
});

it("B48 binds each Rishadan Cutpurse payment to the same opponent who would sacrifice", () => {
  const text = "When this creature enters, each opponent sacrifices a permanent of their choice unless they pay {1}.";
  const { graph, resolution } = card("Rishadan Cutpurse", "Creature — Human Pirate", text);
  assert.equal(resolution.complete, true);
  const member = node(graph, "instruction-member", "/condition");
  assert.equal(member.group, target(graph, "/steps/0/who"));
  const payment = node(graph, "event", "/condition");
  assert.equal(payment.decider, member.id);
  assert.equal(target(graph, "/condition/cost/0/payer"), member.id);
  assert.equal(target(graph, "/steps/0/chosenBy"), member.id);
  const sacrifice = node(graph, "event", "/steps/0");
  const decline = node(graph, "decision-link", "/condition");
  assert.ok(sacrifice.inputs.includes(member.id));
  assert.equal(sacrifice.condition, decline.id);
  assert.equal(decline.decision, payment.path);
  const mauler = "At the beginning of your end step, each player loses 4 life unless they sacrifice a nontoken creature of their choice.";
  residual(card("Bellowing Mauler", "Creature — Ogre Warrior", mauler), "/condition", "The optional payment event must bind its payer");
});

it("B50 binds both shuffle roles to the exact target of Quiet Speculation", () => {
  const text = "Search target player's library for up to three cards with flashback and put them into that player's graveyard. Then the player shuffles.";
  const { graph, resolution } = card("Quiet Speculation", "Sorcery", text);
  assert.equal(resolution.complete, true);
  const searched = target(graph, "/zones/of/0/owner");
  assert.equal(target(graph, "/effects/1/steps/0/who"), searched);
  assert.equal(target(graph, "/effects/1/steps/0/library/owner"), searched);
  const other = text.replace("that player's graveyard", "your graveyard");
  residual(card("Other destination", "Sorcery", other), "/effects/1/steps/0/who", "This definite player requires");
  const gate = "At the beginning of each player's upkeep, that player reveals the top card of their library. If it's an artifact, creature, enchantment, or land card, the player may put it onto the battlefield.";
  residual(card("Gate to the Aether", "Artifact", gate), "/steps/0/who", "This definite player requires");
});

it("keeps Lotus Vale's replacement payment outside the action-link rules", () => {
  const text = "If this land would enter, sacrifice two untapped lands instead. If you do, put this land onto the battlefield. If you don't, put it into its owner's graveyard.\n{T}: Add three mana of any one color.";
  const { resolution } = card("Lotus Vale", "Land", text);
  assert.equal(resolution.complete, false);
});
