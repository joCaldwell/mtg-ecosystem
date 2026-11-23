import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { analyzeCardBindings, parseOracleText, type BindingSourceCard } from "../src/index.ts";

it("binds the frozen protection and graveyard replacement source", () => {
  const fixture = new URL("./fixtures/ramos/2d7e00b6-12f0-4b03-82a6-50e3d1b5395e.json", import.meta.url);
  const { source, oracleIdentity: identity, sourceSha256, astSha256 } = JSON.parse(readFileSync(fixture, "utf8")) as {
    source: BindingSourceCard & { oracle_text: string }; oracleIdentity: string; sourceSha256: string; astSha256: string;
  };
  const digest = (value: unknown) => createHash("sha256").update(JSON.stringify(value)).digest("hex");
  assert.equal(digest(source), sourceSha256);
  const parse = (text: string, name = source.name) => parseOracleText(text, name, {
    typeLine: source.type_line ?? undefined, layout: source.layout, includeSourceProvenance: true,
  });
  const result = parse(source.oracle_text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(digest([result.abilities]), astSha256);
  const graph = analyzeCardBindings({ identity, source, faces: [{ sourceFaceIndex: 0, result }] });

  const keyword = "/faces/0/abilities/0/keywords/0";
  const ability = "/faces/0/abilities/1";
  const path = `${ability}/effect/replacement`;
  const event = `${path}/event`;
  const replacement = `${path}/replace`;
  const reveal = `${replacement}/effects/0/steps/0`;
  const shuffle = `${replacement}/effects/0/steps/1`;
  const ownerPath = `${shuffle}/library/owner`;
  const id = (path: string, role: string) => `card:${identity}${path}#${role}`;
  const object = id("/source", "object");
  const scope = id(ability, "ability");
  const actor = id(ability, "you");
  const owner = id(ownerPath, "owner");
  const proposal = id(event, "proposal");

  assert.ok(graph.nodes.some(n => n.id === id(keyword, "keyword") && n.node === "keyword-ability" &&
    n.keyword === "protection" && n.protectionFrom === "everything" && n.abilities.length === 1 && n.abilities[0] === "static"));
  assert.ok(graph.nodes.some(n => n.id === actor && n.node === "runtime-player" &&
    n.scope === scope && n.role === "replacement-source-controller-or-owner"));
  assert.deepEqual(graph.nodes.find(n => n.id === owner), {
    id: owner, path: ownerPath, node: "derived-player", scope, role: "owner", of: object, rules: ["108.3", "400.3"],
  });
  assert.deepEqual(graph.nodes.find(n => n.id === proposal), {
    id: proposal, path: event, node: "zone-change-proposal", scope, object,
    from: "anywhere", to: { zone: "graveyard", owner }, rules: ["400.3", "400.6", "614.1a"],
  });
  assert.deepEqual(graph.nodes.find(n => n.id === id(replacement, "replacement")), {
    id: id(replacement, "replacement"), path: replacement, node: "reveal-and-shuffle-replacement", scope,
    replaces: proposal, occurrence: "each", reveal: { actor, object }, shuffleInto: { actor, object, library: { zone: "library", owner } },
    rules: ["614.6", "701.20a", "701.20b", "701.24a", "701.24c"],
  });
  assert.deepEqual(graph.references.map(r => [r.path, r.binding === "bound" && r.target]), [
    [`${event}/what`, object], [`${reveal}/who`, actor], [`${reveal}/what`, object],
    [`${shuffle}/who`, actor], [`${shuffle}/what/0`, object], [ownerPath, owner],
  ]);
  assert.deepEqual(graph.diagnostics, []);
  assert.equal(graph.referencesBound, true);
  assert.deepEqual(graph.engineFeatures.map(n => [n.feature, n.path]), [
    ["protection-from-everything", keyword], ["graveyard-reveal-and-shuffle", path],
  ]);
  assert.equal(graph.fullyBound, false);
  assert.equal(graph.executable, false);

  for (const changedText of [
    source.oracle_text.replace("Protection from everything", "Protection from red"),
    source.oracle_text.replace("from anywhere", "from the battlefield"),
    source.oracle_text.replace("its owner's library", "your library"),
    source.oracle_text.replace(`reveal ${source.name} and shuffle it`, `shuffle ${source.name}`),
    `${source.oracle_text}\nTrample`,
  ]) {
    assert.notEqual(changedText, source.oracle_text);
    const changed = parse(changedText);
    assert.ok(changed.ok, JSON.stringify(changed.lines));
    const rejected = analyzeCardBindings({ identity, source, faces: [{ sourceFaceIndex: 0, result: changed }] });
    assert.equal(rejected.referencesBound, false, changedText);
    assert.ok(rejected.diagnostics.some(d => d.code === "unsupported-scope" && d.path === ability), changedText);
    assert.ok(rejected.nodes.every(n => n.node !== "reveal-and-shuffle-replacement"), changedText);
  }

  const renamed = { ...source, name: "Renamed Source", oracle_text: source.oracle_text.replaceAll(source.name, "Renamed Source") };
  const renamedGraph = analyzeCardBindings({ identity: "renamed-source", source: renamed,
    faces: [{ sourceFaceIndex: 0, result: parse(renamed.oracle_text, renamed.name) }] });
  assert.equal(renamedGraph.referencesBound, true);
  assert.deepEqual(renamedGraph.engineFeatures, graph.engineFeatures);
});
