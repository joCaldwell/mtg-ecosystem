import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Effect } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { STRUCTURAL_STEPS, isStructuralStep, stepEventOperation, stepObligation } from "../src/semantic/step-vocabulary.ts";

// Exact Oracle text from Scryfall oracle corpus SHA256
// f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738 (archive R3).
// CR 608.2c, 701.13, 701.20, 700.2:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const LEAP = "{G}, Sacrifice a creature: Reveal cards from the top of your library until you reveal a creature card. Put that card into your hand and the rest on the bottom of your library in a random order.";
const GOODS = "Target opponent exiles cards from the top of their library until they exile a nonland card. Until end of turn, you may cast that card without paying its mana cost.";
const STRINGS = "{2}, {T}: You may tap or untap target creature.";

function bind(text: string, name: string, typeLine: string) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  const graph = analyzeCardBindings({ identity: "fixture", source: { name, layout: "normal", type_line: typeLine },
    faces: [{ sourceFaceIndex: 0, result }] });
  return { abilities: result.abilities, graph };
}

/** `(path, step)` for every printed sentence step, the unit the Rust
 * composition lowerer checks against the archived graph. */
function steps(abilities: unknown[]): [string, Effect][] {
  return abilities.flatMap((ability, block) => {
    const effects = (ability as { effects?: { steps: Effect[] }[] }).effects ?? [];
    return effects.flatMap((sentence, index) => sentence.steps.map((step, at): [string, Effect] =>
      [`/faces/0/abilities/${block}/effects/${index}/steps/${at}`, step]));
  });
}

function eventsAt(graph: ReturnType<typeof bind>["graph"], path: string): string[] {
  return graph.nodes.flatMap(node => node.node === "event" && node.path === path ? [node.operation] : []);
}

describe("binder step vocabulary", () => {
  it("declares the structural steps once, keyed by AST discriminants", () => {
    // The Rust lowerer's grouping exemptions are member-step scopes here.
    for (const group of ["modal", "simultaneous", "optional-group"] as const) {
      assert.deepEqual(stepObligation(group), { kind: "scope", scope: "member-steps" });
    }
    assert.deepEqual(stepObligation("create-delayed-trigger"), { kind: "scope", scope: "nested-ability" });
    assert.deepEqual(stepObligation("for-each"), { kind: "scope", scope: "iteration" });
    assert.deepEqual(stepObligation("self-replacement"), { kind: "scope", scope: "replacement" });
    assert.deepEqual(stepObligation("modify-characteristics"), { kind: "scope", scope: "continuous-effect" });
    // Every other step is an instruction evented under its own discriminant.
    assert.deepEqual(stepObligation("scan-library"), { kind: "event", operation: "scan-library" });
    assert.deepEqual(stepObligation("choice"), { kind: "event", operation: "choice" });
    assert.equal(stepEventOperation("destroy"), "destroy");
    assert.equal(isStructuralStep("draw"), false);
    assert.equal(Object.keys(STRUCTURAL_STEPS).length, 9);
  });

  it("events a revealing or exiling library scan as scan-library and keeps its operation on the scan", () => {
    for (const [text, name, typeLine, operation] of [
      [LEAP, "Evolutionary Leap", "Enchantment", "reveal"],
      [GOODS, "Stolen Goods", "Sorcery", "exile"],
    ] as const) {
      const { abilities, graph } = bind(text, name, typeLine);
      assert.equal(graph.referencesBound, true, JSON.stringify(graph.diagnostics));
      const scans = steps(abilities).filter(([, step]) => step.effect === "scan-library");
      assert.equal(scans.length, 1, name);
      const [path] = scans[0];
      assert.deepEqual(eventsAt(graph, path), ["scan-library"], name);
      const scan = graph.nodes.find(node => node.node === "library-scan" && node.path === path);
      assert.equal(scan?.node === "library-scan" && scan.operation, operation, name);
      // The old prefixed spelling is gone from live output.
      assert.equal(graph.nodes.some(node => node.node === "event" && node.operation.startsWith("library-scan:")), false, name);
    }
  });

  it("events an optional choice as choice", () => {
    const { abilities, graph } = bind(STRINGS, "Puppet Strings", "Artifact");
    assert.equal(graph.referencesBound, true, JSON.stringify(graph.diagnostics));
    const [[path, step]] = steps(abilities);
    assert.equal(step.effect, "choice");
    assert.deepEqual(eventsAt(graph, path), ["choice"]);
    assert.equal(graph.nodes.some(node => node.node === "event" && node.operation === "choose-one"), false);
  });

  it("accounts for every printed step of a bound graph by its obligation", () => {
    for (const [text, name, typeLine] of [
      [LEAP, "Evolutionary Leap", "Enchantment"],
      [GOODS, "Stolen Goods", "Sorcery"],
      [STRINGS, "Puppet Strings", "Artifact"],
    ] as const) {
      const { abilities, graph } = bind(text, name, typeLine);
      for (const [path, step] of steps(abilities)) {
        const obligation = stepObligation(step.effect);
        if (obligation.kind === "event") assert.ok(eventsAt(graph, path).includes(obligation.operation), `${name} ${path}`);
      }
    }
    // A step that is not structural and has no event at its path is a gap the
    // check reports, not one it accepts.
    const missing = bind(STRINGS, "Puppet Strings", "Artifact");
    const [[path, step]] = steps(missing.abilities);
    const graph = { ...missing.graph, nodes: missing.graph.nodes.filter(node => !(node.node === "event" && node.path === path)) };
    const obligation = stepObligation(step.effect);
    assert.equal(obligation.kind === "event" && eventsAt(graph, path).includes(obligation.operation), false);
  });
});
