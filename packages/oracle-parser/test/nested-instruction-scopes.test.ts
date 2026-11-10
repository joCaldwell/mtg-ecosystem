// Reference binding through optional, modal, chosen, simultaneous and
// "for each" instructions (CR 608.2c-d, 700.2). Wordings follow Shaper
// Parasite, Sagu Pummeler, Elvish Promenade, and the "Choose one"
// artifact/enchantment modes.
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import type { BindingNode, CardBindingAnalysis } from "../src/semantic/types.ts";

function analyze(text: string, typeLine = "Sorcery"): CardBindingAnalysis {
  const result = parseOracleText(text, "Fixture", { includeSourceProvenance: true, typeLine });
  assert.equal(result.ok, true, `${text}: ${JSON.stringify(result.lines)}`);
  return analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: typeLine }, faces: [{ sourceFaceIndex: 0, result }] });
}
type EventNode = Extract<BindingNode, { node: "event" }>;
const events = (graph: CardBindingAnalysis) => graph.nodes.filter((node): node is EventNode => node.node === "event");
function target(graph: CardBindingAnalysis, suffix: string): string {
  const reference = graph.references.find(candidate => candidate.path.endsWith(suffix));
  assert.ok(reference && reference.binding === "bound", `${suffix}: ${JSON.stringify(reference)}`);
  return reference.binding === "bound" ? reference.target : "";
}

describe("optional instructions (CR 608.2d)", () => {
  it("binds a single optional instruction and records who decides", () => {
    const mine = analyze("You may draw a card.");
    assert.equal(mine.fullyBound, true, JSON.stringify(mine.diagnostics));
    const [draw] = events(mine);
    assert.equal(draw.optional, true);
    assert.equal(draw.decider, target(mine, "/steps/0/who"));
    const theirs = analyze("Target opponent may draw a card.");
    assert.equal(theirs.fullyBound, true, JSON.stringify(theirs.diagnostics));
    assert.equal(events(theirs)[0].decider, target(theirs, "/steps/0/decidedBy"));
  });

  it("performs an optional group in order under one decision", () => {
    const graph = analyze("You may search your library for a basic land card, reveal it, put it into your hand, then shuffle.");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    const found = target(graph, "/steps/0/steps/0/for");
    assert.equal(target(graph, "/steps/0/steps/1/what"), found);
    assert.equal(target(graph, "/steps/0/steps/2/what"), found);
    const decider = target(graph, "/steps/0/who");
    assert.ok(events(graph).every(event => event.optional && event.decider === decider));
  });
});

describe("modal and chosen instructions (CR 700.2, 608.2d)", () => {
  it("binds each mode's targets within that mode", () => {
    const graph = analyze("Choose one —\n• Destroy target artifact.\n• Destroy target enchantment.", "Instant");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.notEqual(target(graph, "/options/0/effects/0/steps/0/what"), target(graph, "/options/1/effects/0/steps/0/what"));
  });

  it("does not let a mode refer to another mode's result", () => {
    const graph = analyze("Choose one —\n• Create a 1/1 white Soldier creature token.\n• Put a +1/+1 counter on it.", "Instant");
    assert.equal(graph.fullyBound, false);
    const reference = graph.references.find(candidate => candidate.path.endsWith("/options/1/effects/0/steps/0/on"));
    assert.ok(reference && reference.binding === "unresolved" && reference.code === "missing-antecedent", JSON.stringify(reference));
  });

  it("keeps repeatable modes unsupported, since each repetition has its own targets", () => {
    const graph = analyze("Choose one or more. You may choose the same mode more than once.\n• Draw a card.\n• Create a Treasure token.");
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.message.startsWith("Nested, optional")));
  });

  it("uses one target selection for a target shared by both choices (Shaper Parasite)", () => {
    const graph = analyze("When this creature is turned face up, target creature gets +2/-2 or -2/+2 until end of turn.", "Creature — Illusion");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(target(graph, "/options/0/what"), target(graph, "/options/1/what"));
  });
});

describe("simultaneous instructions and for-each riders", () => {
  it("binds simultaneous steps from the same antecedents (Sagu Pummeler)", () => {
    const graph = analyze("Put two +1/+1 counters and a reach counter on target creature.");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(target(graph, "/steps/0/steps/0/on"), target(graph, "/steps/0/steps/1/on"));
  });

  it("scales a for-each instruction without fixing how many objects result (Elvish Promenade)", () => {
    const graph = analyze("Create a 1/1 green Elf Warrior creature token for each Elf you control.");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(events(graph)[0].repeated, "for-each");
    const result = graph.nodes.find(node => node.node === "event-result" && node.result === "objects");
    assert.ok(result && result.node === "event-result");
    assert.equal(result.cardinality, "unknown");
  });
});

describe("coin flips (CR 705)", () => {
  it("binds each outcome as a branch and keeps a target announced in an earlier outcome", () => {
    const graph = analyze("Flip a coin. If you win the flip, target creature gets +2/+0 until end of turn. If you lose the flip, it gets -0/-2 until end of turn.", "Instant");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(target(graph, "/outcomes/1/effects/0/steps/0/what"), target(graph, "/outcomes/0/effects/0/steps/0/what"));
  });
  it("keeps repeated flips unsupported", () => {
    assert.equal(analyze("Flip a coin until you lose a flip. Draw a card for each flip you won.").fullyBound, false);
  });
});
