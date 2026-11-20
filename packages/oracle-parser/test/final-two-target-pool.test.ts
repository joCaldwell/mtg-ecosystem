import assert from "node:assert/strict";
import { test } from "node:test";
import { analyzeCardBindings, parseOracleText, type CardBindingAnalysis } from "../src/index.ts";

const haze = "Enchant player\nAt the beginning of enchanted player's first upkeep each turn, that player gets an additional upkeep step after this step.";
const dilemma = "Each opponent secretly chooses silence or snitch, then the choices are revealed. If each opponent chose silence, ~ deals 4 damage to each of them. If each opponent chose snitch, ~ deals 8 damage to each of them. Otherwise, ~ deals 12 damage to each opponent who chose silence.";
const aura = "Enchantment — Aura";

function parse(text: string, typeLine: string, name = "Fixture") {
  return parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
}

function analyze(text: string, typeLine: string, name = "Fixture") {
  const result = parse(text, typeLine, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  const graph = analyzeCardBindings({ identity: name, source: { name, layout: "normal", type_line: typeLine },
    faces: [{ sourceFaceIndex: 0, result }] });
  return { abilities: result.abilities, graph };
}

function oneNode<K extends CardBindingAnalysis["nodes"][number]["node"]>(graph: CardBindingAnalysis, kind: K) {
  const found = graph.nodes.filter(node => node.node === kind);
  assert.equal(found.length, 1, kind);
  return found[0] as Extract<CardBindingAnalysis["nodes"][number], { node: K }>;
}

test("capture the active player and first actual upkeep before inserting a step", () => {
  const { abilities, graph } = analyze(haze, aura, "Paradox Haze");
  assert.equal(abilities.length, 2);
  const triggered = abilities[1];
  assert.equal(triggered.kind, "triggered");
  assert.deepEqual(triggered.trigger, {
    trigger: "phase", phase: "upkeep", whose: { player: "enchanted-player" }, firstEachTurn: true,
  });
  assert.deepEqual(triggered.effects[0].steps, [{
    effect: "add-steps", step: "upkeep", count: { amount: "fixed", value: 1 },
    who: { player: "that-player" }, after: { step: "current" },
  }]);
  assert.deepEqual(graph.diagnostics, []);
  assert.equal(graph.referencesBound, true);
  assert.equal(graph.fullyBound, false);
  const upkeep = oneNode(graph, "first-upkeep-start");
  const insertion = oneNode(graph, "upkeep-insertion");
  const attachment = oneNode(graph, "derived-object");
  const capturedPlayer = oneNode(graph, "event-result");
  assert.equal(upkeep.occurrence, "first-actual-upkeep-of-turn");
  assert.equal(upkeep.attachmentAt, "trigger-match");
  assert.equal(upkeep.enchantedPlayer, attachment.id);
  assert.equal(attachment.role, "attached-to");
  assert.equal(capturedPlayer.event, upkeep.event);
  assert.equal(capturedPlayer.result, "player");
  assert.equal(insertion.player, capturedPlayer.id);
  assert.equal(insertion.after, upkeep.id);
  assert.equal(insertion.placement, "same-phase");
  assert.equal(insertion.order, "most-recent-first");
  const resolvingPlayer = graph.references.find(reference => reference.path.endsWith("/steps/0/who"));
  assert.ok(resolvingPlayer?.binding === "bound");
  assert.equal(resolvingPlayer.target, capturedPlayer.id);
  assert.ok(graph.engineFeatures.some(need => need.feature === "phase-scheduling" && need.rules.includes("500.9")));
});

test("upkeep insertion needs the printed step anchor and an identified participant", () => {
  for (const adjacent of [
    haze.replace("first upkeep each turn", "upkeep"),
    haze.replace("first upkeep each turn", "first upkeep each game"),
    haze.replace("first upkeep each turn", "first draw step each turn"),
    haze.replace("each turn,", "each turn during your turn,"),
    haze.replace("each turn,", "each turn during combat,"),
    haze.replace("each turn,", "each turn for the first time each turn,"),
    haze.replace("after this step", "after this phase"),
    haze.replace("after this step", "at the end of this step"),
    haze.replace("that player gets", "you get"),
    haze.replace("that player gets", "draw a card. That player gets"),
  ]) assert.equal(parse(adjacent, aura).ok, false, adjacent);
  const { abilities } = analyze("You get an additional upkeep step after this phase.", "Sorcery");
  assert.equal(abilities[0].kind, "spell");
  assert.equal(abilities[0].effects[0].steps[0].effect, "add-phases");
});

test("one revealed choice cohort feeds two unanimous outcomes and an exclusive fallback", () => {
  const { abilities, graph } = analyze(dilemma.replaceAll("~", "Prisoner's Dilemma") + "\nFlashback {5}{R}{R}",
    "Sorcery", "Prisoner's Dilemma");
  assert.equal(abilities.length, 2);
  assert.equal(abilities[0].kind, "spell");
  const effect = abilities[0].effects[0].steps[0];
  assert.deepEqual(effect, {
    effect: "secret-choice-damage", who: { player: "each-opponent" }, choices: ["silence", "snitch"],
    outcomes: [
      { when: "unanimous", choice: "silence", recipients: "all-choosers", source: { ref: "self" }, amount: { amount: "fixed", value: 4 } },
      { when: "unanimous", choice: "snitch", recipients: "all-choosers", source: { ref: "self" }, amount: { amount: "fixed", value: 8 } },
      { when: "otherwise", recipients: { chose: "silence" }, source: { ref: "self" }, amount: { amount: "fixed", value: 12 } },
    ],
  });
  assert.equal(abilities[1].kind, "keywords");
  assert.equal(abilities[1].keywords[0].keyword, "flashback");
  assert.deepEqual(graph.diagnostics, []);
  assert.equal(graph.referencesBound, true);
  assert.equal(graph.fullyBound, false);
  const reveal = oneNode(graph, "secret-choice-reveal");
  const subset = oneNode(graph, "secret-choice-subset");
  assert.deepEqual([reveal.collection, reveal.count, reveal.reveal], ["private-apnap", "one-per-chooser", "simultaneous"]);
  const controller = graph.nodes.find(node => node.id === reveal.controller);
  assert.equal(controller?.node, "runtime-player");
  assert.equal(controller.role, "spell-controller");
  const chooser = graph.nodes.find(node => node.id === reveal.choosers);
  assert.equal(chooser?.node, "selection");
  assert.deepEqual(chooser.expression, { player: "each-opponent" });
  assert.equal(subset.revealed, reveal.id);
  assert.equal(subset.chose, "silence");
  const outcomes = graph.nodes.filter(node => node.node === "secret-choice-outcome");
  assert.equal(outcomes.length, 3);
  assert.deepEqual(outcomes.map(node => node.when), [
    { kind: "unanimous", choice: "silence" },
    { kind: "unanimous", choice: "snitch", unless: outcomes[0].id },
    { kind: "otherwise", unless: outcomes.slice(0, 2).map(node => node.id) },
  ]);
  assert.deepEqual(outcomes.map(node => node.recipients), [reveal.choosers, reveal.choosers, subset.id]);
  const source = oneNode(graph, "source");
  for (const outcome of outcomes) {
    assert.equal(outcome.revealed, reveal.id);
    assert.equal(outcome.damage.timing, "simultaneous");
    assert.equal(outcome.damage.sourceInformation, "current-or-last-known");
    const damage = graph.nodes.find(node => node.node === "event" && node.path === outcome.path);
    assert.ok(damage?.node === "event");
    assert.equal(damage.operation, "damage");
    assert.equal(damage.condition, outcome.id);
    assert.equal(damage.requiresResult, reveal.id);
    assert.deepEqual(damage.inputs, [source.id, outcome.recipients]);
  }
  assert.ok(!graph.nodes.some(node => node.node === "named-vote" || node.node === "vote-outcome"));
  assert.equal(new Set(graph.nodes.map(node => node.id)).size, graph.nodes.length);
});

test("secret choice outcomes require one reveal, distinct offered options, and the same chooser group", () => {
  const labels = dilemma.replaceAll("silence", "left").replaceAll("snitch", "right");
  assert.equal(parse(labels, "Sorcery").ok, true);
  for (const adjacent of [
    dilemma.replace("secretly chooses", "chooses"),
    dilemma.replace("secretly chooses", "secretly votes for"),
    dilemma.replace(", then the choices are revealed", ""),
    dilemma.replace("snitch, then", "silence, then"),
    dilemma.replace("chose snitch", "chose silence"),
    dilemma.replace("chose snitch", "chose leave"),
    dilemma.replace("each of them", "each player"),
    dilemma.replace("each opponent who chose silence", "each opponent who chose leave"),
    dilemma.replace("each opponent who chose silence", "target opponent who chose silence"),
    dilemma.replace("If each opponent chose snitch, ~ deals 8 damage to each of them. ", ""),
    dilemma + " Those players draw a card.",
  ]) assert.equal(parse(adjacent, "Sorcery").ok, false, adjacent);
});

test("a later spell paragraph cannot use the private choices as a player antecedent", () => {
  for (const [suffix, pronoun] of [
    ["That player draws a card.", "that-player"],
    ["Those players draw a card.", "those-players"],
  ]) {
    const { graph } = analyze(dilemma + "\n" + suffix, "Sorcery");
    assert.ok(graph.references.some(ref => ref.binding === "unresolved" && "player" in ref.expression &&
      ref.expression.player === pronoun));
  }
});
