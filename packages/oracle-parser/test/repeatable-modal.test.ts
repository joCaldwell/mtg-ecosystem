import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { analyzeCardBindings, parseOracleText, type BindingSourceCard } from "../src/index.ts";

it("preserves all five printed Unite modes and their announcement target templates", () => {
  const fixture = new URL("../../game-engine/tests/fixtures/ramos/164651c5-b831-4070-8491-16d30ac8395b.json", import.meta.url);
  const { source, oracleIdentity, faces } = JSON.parse(readFileSync(fixture, "utf8")) as {
    source: BindingSourceCard & { oracle_text: string }; oracleIdentity: string;
    faces: { result: ReturnType<typeof parseOracleText> }[];
  };
  const parse = (text: string) => parseOracleText(text, source.name, {
    layout: source.layout, typeLine: source.type_line ?? undefined, includeSourceProvenance: true,
  });
  const result = parse(source.oracle_text);
  assert.ok(result.ok);
  const expected = faces[0].result;
  assert.ok(expected.ok);
  assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), expected.abilities);
  const graph = analyzeCardBindings({ identity: oracleIdentity, source, faces: [{ sourceFaceIndex: 0, result }] });
  const modal = "/faces/0/abilities/0/effects/0/steps/0";
  const targets = graph.nodes.filter(node => node.node === "selection" && node.timing === "stack-announcement");
  assert.deepEqual(targets.map(node => node.path), ["what", "who", "what/owner", "to", "what"].map(
    (suffix, index) => `${modal}/options/${index}/effects/0/steps/0/${suffix}`));
  assert.equal(new Set(targets.map(node => node.id)).size, 5);
  assert.deepEqual(graph.diagnostics, []);
  assert.deepEqual(graph.engineFeatures.map(entry => [entry.feature, entry.path]), [["repeatable-modal-selection", modal]]);
  assert.equal(graph.fullyBound, false);
  assert.equal(parse(source.oracle_text.replace("or enchantment.", "or enchantment plus nonsense.")).ok, false);
});
