import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { analyzeCardBindings, parseOracleText, type BindingSourceCard } from "../src/index.ts";

it("binds the frozen Sakashima's Protege entry copy separately from Flash and Cascade", () => {
  const fixture = new URL("../../game-engine/tests/fixtures/ramos/cf127842-5fb1-44df-8d91-8cc0f48c559a.json", import.meta.url);
  const { source, oracleIdentity: identity, sourceSha256, astSha256 } = JSON.parse(readFileSync(fixture, "utf8")) as {
    source: BindingSourceCard & { oracle_text: string }; oracleIdentity: string; sourceSha256: string; astSha256: string;
  };
  const digest = (value: unknown) => createHash("sha256").update(JSON.stringify(value)).digest("hex");
  const parse = (text: string, name = source.name) => parseOracleText(text, name, {
    typeLine: source.type_line ?? undefined, layout: source.layout, includeSourceProvenance: true,
  });
  const result = parse(source.oracle_text);
  assert.equal(digest(source), sourceSha256);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(digest([result.abilities]), astSha256);

  const graph = analyzeCardBindings({ identity, source, faces: [{ sourceFaceIndex: 0, result }] });
  const ability = "/faces/0/abilities/2";
  const path = `${ability}/effect`;
  const what = `${path}/what`;
  const model = `${path}/model`;
  const id = (path: string, role: string) => `card:${identity}${path}#${role}`;
  const scope = id(ability, "ability");
  const chooser = id(ability, "you");
  const proposal = id(what, "proposal");
  const choice = id(model, "choice");
  assert.deepEqual(graph.nodes.filter(n => n.node === "keyword-ability").map(n => [n.path, n.keyword, n.abilities, n.rules]), [
    ["/faces/0/abilities/0/keywords/0", "flash", ["static"], ["702.8"]],
    ["/faces/0/abilities/1/keywords/0", "cascade", ["triggered"], ["702.85"]],
  ]);
  assert.deepEqual(graph.nodes.find(n => n.id === chooser), {
    id: chooser, path: ability, node: "runtime-player", scope, role: "entering-controller", rules: ["109.5", "110.2", "614.12"],
  });
  assert.deepEqual(graph.nodes.find(n => n.id === proposal), {
    id: proposal, path: what, node: "battlefield-entry-proposal", scope,
    object: id("/source", "object"), to: "battlefield", controller: chooser, rules: ["110.2", "614.12"],
  });
  assert.deepEqual(graph.nodes.find(n => n.id === choice), {
    id: choice, path: model, node: "entry-copy-model-choice", scope, proposal, chooser,
    timing: "before-entry", selection: "non-target", count: "zero-or-one", zone: "battlefield",
    filter: { cls: "permanent", zoneChangeHistory: { event: "enters", to: { zone: "battlefield" }, during: "this-turn" } },
    historyIdentity: "current-incarnation", rules: ["110.1", "115.10a", "400.7", "614.12a"],
  });
  assert.deepEqual(graph.nodes.find(n => n.id === id(path, "replacement")), {
    id: id(path, "replacement"), path, node: "entry-copy-replacement", scope,
    entry: proposal, model: choice, timing: "as-enters", values: "model-copiable-values",
    onDecline: "leave-entry-unchanged", rules: ["614.12", "707.2", "707.2b", "707.5"],
  });
  assert.deepEqual(graph.references.map(r => [r.path, r.binding === "bound" && r.target]), [
    [what, proposal],
    [`${model}/who`, chooser],
    [`${path}/optional/who`, chooser],
  ]);
  assert.deepEqual(graph.diagnostics, []);
  assert.equal(graph.referencesBound, true);
  assert.deepEqual(graph.engineFeatures.map(need => [need.feature, need.path]), [["entry-copy-replacement", path]]);
  assert.equal(graph.fullyBound, false);
  assert.equal(graph.executable, false);

  for (const [original, mutation] of [
    ["any permanent", "any creature"],
    ["permanent that entered", "permanent you control that entered"],
    ["You may have this creature enter", "This creature enters"],
    ["enter as a copy", "enter tapped as a copy"],
    ["entered this turn.", "entered this turn until end of turn."],
    ["entered this turn.", "entered this turn, except it isn't legendary."],
  ]) {
    const text = source.oracle_text.replace(original, mutation);
    assert.notEqual(text, source.oracle_text);
    const changed = parse(text);
    assert.ok(changed.ok, JSON.stringify(changed.lines));
    const rejected = analyzeCardBindings({ identity, source, faces: [{ sourceFaceIndex: 0, result: changed }] });
    assert.equal(rejected.referencesBound, false, mutation);
    assert.ok(rejected.diagnostics.some(d => d.path === path && d.code === "unsupported-scope"), mutation);
    assert.ok(rejected.nodes.every(n => n.node !== "entry-copy-replacement"), mutation);
  }
  for (const [original, mutation] of [
    ["any permanent", "target permanent"],
    ["entered this turn", "entered last turn"],
    ["You may have this creature enter as", "When this creature enters, you may have it become"],
    ["entered this turn.", "entered this turn. You may choose new targets for the copy."],
  ]) {
    const text = source.oracle_text.replace(original, mutation);
    assert.notEqual(text, source.oracle_text);
    assert.equal(parse(text).ok, false, mutation);
  }

  const renamed = { ...source, name: "Renamed Source" };
  const renamedGraph = analyzeCardBindings({ identity: "renamed-source", source: renamed,
    faces: [{ sourceFaceIndex: 0, result: parse(source.oracle_text, renamed.name) }] });
  assert.equal(renamedGraph.referencesBound, true);
  assert.deepEqual(renamedGraph.engineFeatures, graph.engineFeatures);
});
