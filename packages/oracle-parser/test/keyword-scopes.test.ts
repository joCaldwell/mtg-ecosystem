// Reference binding of keyword abilities (CR 702). A keyword's intrinsic text
// is fixed by the rules; its printed parameters belong to the ability the
// keyword represents, so "you" and a cost's payer are that ability's player.
// Edge of Autumn, Graveyard Trespasser and Primal Growth print the cycling,
// ward and kicker costs used here.
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { PARSED_KEYWORDS } from "../src/parser/keywords.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { KEYWORD_SCOPES } from "../src/semantic/keyword-scopes.ts";
import type { BindingNode, CardBindingAnalysis } from "../src/semantic/types.ts";

function analyze(text: string, typeLine = "Creature"): CardBindingAnalysis {
  const result = parseOracleText(text, "Fixture", { includeSourceProvenance: true, typeLine });
  assert.equal(result.ok, true, `${text}: ${JSON.stringify(result.lines)}`);
  return analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: typeLine }, faces: [{ sourceFaceIndex: 0, result }] });
}
function node(graph: CardBindingAnalysis, id: string): BindingNode {
  const found = graph.nodes.find(candidate => candidate.id === id);
  assert.ok(found, id);
  return found;
}
/** The role of the player a reference ending in `suffix` is bound to. */
function roleOf(graph: CardBindingAnalysis, suffix: string): string {
  const reference = graph.references.find(candidate => candidate.path.endsWith(suffix));
  assert.ok(reference && reference.binding === "bound", `${suffix}: ${JSON.stringify(reference)}`);
  const target = node(graph, reference.target);
  assert.equal(target.node, "runtime-player");
  return target.node === "runtime-player" ? target.role : "";
}

describe("keyword ability scopes", () => {
  it("has a scope decision for every keyword the parser produces", () => {
    for (const keyword of PARSED_KEYWORDS) assert.ok(Object.hasOwn(KEYWORD_SCOPES, keyword), keyword);
    for (const [keyword, scope] of Object.entries(KEYWORD_SCOPES))
      if (!("unsupported" in scope)) assert.ok(scope.rules.every(rule => /^702\.\d+$/.test(rule)) && scope.abilities.length > 0, keyword);
  });

  it("binds a bare keyword with nothing left unresolved and records the ability it represents", () => {
    const graph = analyze("Flying");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    const keyword = graph.nodes.find(candidate => candidate.node === "keyword-ability");
    assert.ok(keyword && keyword.node === "keyword-ability");
    assert.deepEqual([keyword.keyword, keyword.abilities, keyword.rules], ["flying", ["static"], ["702.9"]]);
  });

  it("binds a printed cost's payer to the player of the ability the keyword represents", () => {
    for (const [text, typeLine, role] of [
      ["Equip {2}", "Artifact — Equipment", "activator"],
      ["Cycling—Sacrifice a land.", "Creature", "activator"],
      ["Kicker—Sacrifice a creature.", "Sorcery", "prospective-caster"],
      ["Flashback {2}{R}", "Sorcery", "prospective-caster"],
      ["Echo {2}{R}", "Creature", "trigger-controller"],
      ["Morph {3}{G}", "Creature", "special-action-player"],
      ["Suspend 4—{G}", "Creature", "special-action-player"],
      // CR 702.21a: the controller of the targeting spell or ability pays ward.
      ["Ward {2}", "Creature", "targeting-player"],
      ["Ward—Discard a card.", "Creature", "targeting-player"],
    ] as const) {
      const graph = analyze(text, typeLine);
      assert.equal(graph.fullyBound, true, `${text}: ${JSON.stringify(graph.diagnostics)}`);
      assert.equal(roleOf(graph, "/keywords/0/cost/0/payer"), role, text);
    }
  });

  it("does not bind a keyword's payer to the static ability line's controller", () => {
    const graph = analyze("Ward {2}");
    const line = graph.nodes.find(candidate => candidate.node === "ability");
    const payer = graph.references.find(reference => reference.path.endsWith("/cost/0/payer"));
    assert.ok(line && line.node === "ability" && payer && payer.binding === "bound");
    assert.notEqual(payer.target, line.you);
  });

  it("binds implicit players and X definitions inside the keyword's parameters", () => {
    assert.equal(roleOf(analyze("Enchant creature you control", "Enchantment — Aura"), "/keywords/0/filter/control"), "static-source-controller");
    const firebending = analyze("Firebending X, where X is this creature's power.");
    assert.equal(firebending.fullyBound, true, JSON.stringify(firebending.diagnostics));
    const x = firebending.references.find(reference => reference.path.endsWith("/keywords/0/amount"));
    assert.ok(x && x.binding === "bound");
    assert.equal(node(firebending, x.target).node, "defined-number");
  });

  it("keeps a keyword that belongs to another object unbound only when its parameters have references", () => {
    assert.equal(analyze("When this creature enters, create a 1/1 white Spirit creature token with flying.").fullyBound, true);
    const ward = analyze("When this creature enters, create a 2/2 black Zombie creature token with ward {2}.");
    assert.equal(ward.fullyBound, false);
    assert.ok(ward.diagnostics.some(d => d.code === "unsupported-scope" && d.path.endsWith("/token/keywords/0")));
  });

  it("does not treat conditions and filters that name a keyword as keyword abilities", () => {
    const graph = analyze("Whenever a creature with flying attacks, draw a card.");
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.ok(!graph.nodes.some(candidate => candidate.node === "keyword-ability"));
  });

  it("keeps an unmodeled keyword scope as an obligation", () => {
    assert.ok("unsupported" in KEYWORD_SCOPES.companion);
  });
});
