import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { analyzeCardBindings, parseOracleText, type BindingSourceCard } from "../src/index.ts";

it("binds the frozen Yidris trigger to later hand casts by its controller", () => {
  // Physical archive row 23979 (source index 23977).
  const fixture = new URL("../../game-engine/tests/fixtures/ramos/9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f.json", import.meta.url);
  const { source, oracleIdentity: identity } = JSON.parse(readFileSync(fixture, "utf8")) as {
    source: BindingSourceCard & { oracle_text: string }; oracleIdentity: string;
  };
  const parse = (text: string) => parseOracleText(text, source.name, {
    typeLine: source.type_line ?? undefined, layout: source.layout, includeSourceProvenance: true,
  });
  const result = parse(source.oracle_text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.deepEqual(result.abilities, [
    { kind: "keywords", keywords: [{ keyword: "trample" }] },
    { kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "self" }, combat: true, to: "any" },
      effects: [{ steps: [{ effect: "grant-cast-keyword", timing: "as-cast", caster: { player: "you" },
        from: { zone: "hand", owner: "your" }, what: { recipient: "they" },
        duration: { duration: "this-turn" }, keyword: "cascade" }] }] },
  ]);

  const graph = analyzeCardBindings({ identity, source, faces: [{ sourceFaceIndex: 0, result }] });
  const ability = "/faces/0/abilities/1";
  const path = `${ability}/effects/0/steps/0`;
  const id = (path: string, role: string) => `card:${identity}${path}#${role}`;
  const controller = id(ability, "you");
  const grant = id(path, "grant");
  const spells = id(`${path}/what`, "spells");
  assert.ok(graph.nodes.some(n => n.node === "runtime-player" && n.id === controller && n.role === "trigger-controller"));
  assert.ok(graph.nodes.some(n => n.node === "cast-keyword-grant" && n.id === grant &&
    n.scope === id(ability, "ability") && n.created === "resolution" && n.duration === "this-turn" && n.keyword === "cascade"));
  assert.deepEqual(graph.nodes.find(n => n.id === spells), {
    id: spells, path: `${path}/what`, node: "future-cast-spells", grant, caster: controller,
    from: { zone: "hand", owner: controller }, timing: "as-cast", cardinality: "many",
    rules: ["601.2a", "601.2i", "611.2f"],
  });
  assert.deepEqual(graph.references.map(r => [r.path, r.binding === "bound" && r.target]), [
    [`${ability}/trigger/what`, id("/source", "object")],
    [`${path}/caster`, controller],
    [`${path}/from/owner`, controller],
    [`${path}/what`, spells],
  ]);
  assert.deepEqual(graph.diagnostics, []);
  assert.equal(graph.referencesBound, true);
  assert.deepEqual(graph.engineFeatures.map(n => [n.feature, n.path]), [["cast-keyword-grant", path]]);
  assert.equal(graph.fullyBound, false);
  assert.equal(graph.executable, false);

  for (const [original, mutation] of [
    ["as you cast", "after you cast"],
    ["to a player", "to a creature"],
    ["from your hand", "from your graveyard"],
  ]) {
    assert.ok(source.oracle_text.includes(original));
    assert.equal(parse(source.oracle_text.replace(original, mutation)).ok, false, mutation);
  }
});
