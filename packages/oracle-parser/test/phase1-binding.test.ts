import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import type { CardBindingAnalysis } from "../src/semantic/types.ts";

function analyze(oracleText: string, typeLine: string): CardBindingAnalysis {
  const name = "Fixture";
  const result = parseOracleText(oracleText, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return analyzeCardBindings({
    identity: "fixture", source: { name, type_line: typeLine, layout: "normal" },
    faces: [{ sourceFaceIndex: 0, result }],
  });
}

function bound(graph: CardBindingAnalysis, suffix: string): string {
  const reference = graph.references.find(entry => entry.path.endsWith(suffix));
  assert.ok(reference?.binding === "bound", JSON.stringify(reference ?? graph.diagnostics));
  return reference.target;
}

function boundCard(graph: CardBindingAnalysis): void {
  assert.equal(graph.referencesBound, true, JSON.stringify(graph.diagnostics));
}

describe("counter-change matches", () => {
  it("binds the measured change separately from the actor and recipient", () => {
    const graph = analyze("Whenever you put one or more -1/-1 counters on a creature, "
      + "create that many 1/1 black Insect creature tokens.", "Enchantment");
    boundCard(graph);
    const match = graph.nodes.find(node => node.node === "counter-change-match");
    assert.ok(match && match.actor);
    assert.deepEqual(match.pattern.match, { counters: "one-or-more", recipients: "each" });
    assert.equal(match.actor, bound(graph, "/trigger/by"));
    assert.deepEqual(match.recipients, [bound(graph, "/trigger/on/what")]);
    const count = graph.nodes.find(node => node.id === bound(graph, "/steps/0/count"));
    assert.ok(count?.node === "event-result" && count.identity === "measured-quantity" && count.event === match.event);
    assert.equal(graph.engineFeatures[0]?.feature, "counter-change-event");
  });

  it("does not turn any-of recipients into one guaranteed creature", () => {
    const graph = analyze("Whenever you put one or more counters on a permanent or player, "
      + "put a +1/+1 counter on that creature.", "Enchantment");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "missing-antecedent"));
    assert.ok(!graph.nodes.some(node => node.node === "event-result" && node.result === "objects"));
  });

  it("keeps a threshold transition outside ordinary per-recipient counting", () => {
    const graph = analyze("Whenever the last time counter is removed from this creature, draw a card.", "Creature");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"));
    assert.ok(!graph.nodes.some(node => node.node === "counter-change-match"));
  });
});

describe("payment scope and announced X", () => {
  it("binds an unambiguous activation exclusion and reads the sacrificed object's last known power", () => {
    const graph = analyze("{B}, {T}, Sacrifice another creature: Create X 2/2 black Zombie creature tokens, "
      + "where X is the sacrificed creature's power.", "Creature");
    boundCard(graph);
    assert.equal(bound(graph, "/costs/2/what/filter/other"), graph.nodes.find(node => node.node === "source")?.id);
    assert.equal(bound(graph, "/bindings/0/value/of"), bound(graph, "/costs/2/what"));
  });

  it("rejects an ambiguous exclusion when another object was selected by a cost", () => {
    const graph = analyze("Tap an untapped creature you control, Sacrifice another creature: Draw a card.", "Creature");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-reference" && diagnostic.path.endsWith("/filter/other")));
  });

  it("does not follow a contextual sacrifice reference into the graveyard as an action", () => {
    const graph = analyze("Sacrifice a creature: Exile the sacrificed creature.", "Artifact");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.path.endsWith("/steps/0/what")));
  });

  it("shares the spell's announced X with the spell's additional payment", () => {
    const graph = analyze("As an additional cost to cast this spell, pay X life.\nAll creatures get -X/-X until end of turn.", "Sorcery");
    boundCard(graph);
    const cost = bound(graph, "/costs/0/amount");
    assert.equal(bound(graph, "/power/amount"), cost);
    assert.equal(bound(graph, "/toughness/amount"), cost);
    assert.equal(graph.nodes.filter(node => node.node === "announced-number").length, 1);
  });

  it("keeps a nonzero constraint on the announced activation amount", () => {
    const graph = analyze("{2}{B}, Sacrifice X artifacts: Return target creature card with power X or less "
      + "from your graveyard to the battlefield. X can't be 0.", "Creature");
    boundCard(graph);
    const number = graph.nodes.find(node => node.node === "announced-number");
    assert.ok(number);
    assert.deepEqual(number.constraints, [{ variable: "x", not: 0 }]);
    assert.equal(bound(graph, "/costs/1/what/count"), number.id);
    assert.equal(bound(graph, "/comparison/value"), number.id);
  });

  it("binds a local X definition used by the payment modifier", () => {
    const graph = analyze("{7}{G}, {T}: Create a 4/4 green Wurm creature token. "
      + "This ability costs {X} less to activate, where X is the greatest power among Wurms you control.", "Creature");
    boundCard(graph);
    const number = graph.nodes.find(node => node.node === "defined-number");
    assert.ok(number && number.path.endsWith("/costModifications/0/bindings/0"));
    assert.equal(bound(graph, "/costModifications/0/change/symbols/0"), number.id);
  });
});

describe("manifest result identity", () => {
  it("captures the one possible face-down creature, including when there was no card", () => {
    const graph = analyze("When this Equipment enters, manifest dread, then attach this Equipment to that creature.", "Artifact — Equipment");
    boundCard(graph);
    const result = graph.nodes.find(node => node.id === bound(graph, "/steps/1/to"));
    assert.ok(result?.node === "event-result" && result.identity === "new-objects");
    assert.equal(result.cardinality, "one");
    assert.equal(result.availability, "if-produced");
    assert.deepEqual(result.description, { cls: "permanent", types: ["creature"] });
  });

  it("does not make one manifest result available outside a conditional branch", () => {
    const graph = analyze("When this Equipment enters, you gain 1 life. If you control a creature, manifest dread. Attach this Equipment to that creature.", "Artifact — Equipment");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.path.endsWith("/to")));
  });
});

describe("plural pronouns in ability grants", () => {
  it("uses the sole established object group", () => {
    const graph = analyze("Untap all creatures and gain control of them until end of turn. They gain haste until end of turn.", "Sorcery");
    boundCard(graph);
    assert.equal(bound(graph, "/effects/1/steps/0/what"), bound(graph, "/effects/0/steps/0/what"));
  });

  it("rejects competing groups", () => {
    const graph = analyze("Untap all creatures. Tap all artifacts. They gain haste until end of turn.", "Sorcery");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-reference" && diagnostic.path.endsWith("/what")));
  });
});

describe("stack and text operations", () => {
  it("distinguishes changing the old target from optionally rechoosing targets", () => {
    const change = analyze("Change the target of target spell or ability with a single target.", "Instant");
    const choose = analyze("You may choose new targets for target spell or ability.", "Instant");
    boundCard(change);
    boundCard(choose);
    assert.deepEqual(change.nodes.find(node => node.node === "stack-instruction")?.action,
      { kind: "change-targets", scope: "all" });
    assert.deepEqual(choose.nodes.find(node => node.node === "stack-instruction")?.action,
      { kind: "choose-new-targets", timing: "resolution" });
  });

  it("does not borrow a copy created in a preceding conditional branch", () => {
    const graph = analyze("If you control a creature, copy target spell. You may choose new targets for the copy.", "Instant");
    assert.equal(graph.referencesBound, false);
    assert.ok(graph.references.some(reference => "ref" in reference.expression &&
      reference.expression.ref === "copy-result" && reference.binding === "unresolved"));
  });

  it("does not resolve a bare original-or-copy pronoun by recency", () => {
    const graph = analyze("Copy target instant or sorcery spell, then return it to its owner's hand.", "Instant");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "ambiguous-antecedent"));
  });

  it("binds the target and the contextual word choices separately", () => {
    const graph = analyze("Change the text of target permanent by replacing all instances of one color word with another "
      + "or one basic land type with another.", "Instant");
    boundCard(graph);
    const choice = graph.nodes.find(node => node.node === "text-choice");
    assert.ok(choice);
    assert.equal(choice.subject, bound(graph, "/steps/0/what"));
    assert.deepEqual(choice.replacements.map(option => option.category), ["color-word", "basic-land-type"]);
    assert.equal(graph.engineFeatures[0]?.feature, "text-change");
  });
});

describe("private information and player roles", () => {
  it("binds the viewer and library owner, and defers refreshing the private look", () => {
    const graph = analyze("You may look at the top card of your library any time.", "Artifact");
    boundCard(graph);
    const look = graph.nodes.find(node => node.node === "private-library-look");
    assert.ok(look);
    assert.equal(look.viewer, look.libraryOwner);
    assert.equal(look.viewer, bound(graph, "/permission/who"));
    assert.equal(look.refresh, "after-announcement-or-special-action");
    assert.equal(graph.engineFeatures[0]?.feature, "private-library-look");
  });

  it("binds a cast trigger's possessive zone to the matching caster", () => {
    const graph = analyze("Whenever a player casts a spell from their hand, that player draws a card.", "Artifact");
    boundCard(graph);
    assert.equal(bound(graph, "/trigger/from/owner"), bound(graph, "/trigger/who"));
  });

  it("retains your library or hand as a separate role from another caster", () => {
    const graph = analyze("Whenever a player casts a spell from your hand, that player draws a card.", "Artifact");
    boundCard(graph);
    assert.notEqual(bound(graph, "/trigger/from/owner"), bound(graph, "/trigger/who"));
  });

  it("recognizes repeated ownership of the same announced object", () => {
    const graph = analyze("The owner of target permanent shuffles it into their library, then reveals the top card "
      + "of their library. If it's a permanent card, they put it onto the battlefield.", "Instant");
    boundCard(graph);
    const player = graph.nodes.find(node => node.id === bound(graph, "/effects/1/steps/0/who"));
    assert.ok(player?.node === "selection" && "player" in player.expression && player.expression.player === "owner");
    assert.equal(bound(graph, player.path + "/of"), bound(graph, "/effects/0/steps/0/who/of"));
  });

  it("does not identify owners of independently chosen objects", () => {
    const graph = analyze("The owner of target permanent draws a card. The owner of target artifact loses 1 life. "
      + "They gain 1 life.", "Instant");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "ambiguous-antecedent"));
  });
});
