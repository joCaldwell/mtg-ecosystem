import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { analyzeCardBindings, parseOracleText, type BindingSourceCard, type ParseCardResult } from "../src/index.ts";

it("reparses the frozen Omnath trigger and binds both outcomes to one looked-at card", () => {
  const fixture = new URL("../../game-engine/tests/fixtures/ramos/26445dc7-8363-4205-aee8-1cafeb4ba4c0.json", import.meta.url);
  const { source, status, sourceSha256, oracleIdentity: identity, faces } = JSON.parse(readFileSync(fixture, "utf8")) as {
    source: BindingSourceCard & { oracle_text: string };
    status: string;
    sourceSha256: string;
    oracleIdentity: string;
    faces: { result: ParseCardResult }[];
  };
  assert.equal(status, "unsupported");
  assert.equal(createHash("sha256").update(JSON.stringify(source)).digest("hex"), sourceSha256);
  const [replacement, trigger] = faces[0].result.lines;
  assert.ok(replacement.ok && !trigger.ok);

  const parse = (text: string, name = source.name) => parseOracleText(text, name, {
    typeLine: source.type_line ?? undefined, layout: source.layout, includeSourceProvenance: true,
  });
  const result = parse(source.oracle_text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  const three = { amount: "fixed", value: 3 };
  const step = {
    effect: "look-top-may-reveal-for-mana", who: { player: "you" }, library: { zone: "library", owner: "your" },
    revealIf: { op: "ge", value: three }, mana: three, to: { zone: "hand", owner: "your" },
  };
  const expectedTrigger = { kind: "triggered", abilityWord: undefined,
    trigger: { trigger: "phase", phase: "precombat-main", whose: "your" },
    condition: undefined, effects: [{ steps: [step] }] };
  assert.equal(result.abilities.length, 2);
  assert.equal(JSON.stringify(result.abilities[0]), JSON.stringify(replacement.ability));
  assert.deepEqual(result.abilities[1], expectedTrigger);

  const graph = analyzeCardBindings({ identity, source, faces: [{ sourceFaceIndex: 0, result }] });
  const ability = "/faces/0/abilities/1";
  const path = `${ability}/effects/0/steps/0`;
  const id = (path: string, role: string) => `card:${identity}${path}#${role}`;
  const controller = id(ability, "you");
  const node = (suffix: string, role: string) => graph.nodes.find(n => n.id === id(`${path}/${suffix}`, role));
  const looked = node("library", "objects");
  const reveal = node("reveal", "event");
  const revealed = node("reveal", "objects");
  const eligibility = node("revealIf", "eligibility");
  const mana = node("mana", "event");
  const move = node("to", "event");
  assert.ok(looked?.node === "event-result" && looked.cardinality === "one" && looked.availability === "if-produced");
  assert.ok(reveal?.node === "event" && reveal.optional && reveal.decider === controller && reveal.requiresResult === looked.id);
  assert.ok(revealed?.node === "event-result" && revealed.event === reveal.id && revealed.subsetOf === looked.id);
  assert.ok(eligibility?.node === "colored-mana-symbol-count" && eligibility.card === looked.id && eligibility.guardedEvent === reveal.id);
  assert.deepEqual(eligibility.comparison, step.revealIf);
  assert.ok(mana?.node === "event" && mana.operation === "add-mana" && mana.requiresResult === revealed.id);
  assert.deepEqual(mana.inputs, [controller, revealed.id]);
  assert.ok(move?.node === "event" && move.operation === "move-zone" && !move.conditional && move.requiresResult === looked.id);
  assert.deepEqual(move.inputs, [looked.id, controller]);
  assert.deepEqual(graph.references.filter(r => r.path.startsWith(ability)).map(r => [r.path, r.binding === "bound" && r.target]), [
    [`${ability}/trigger/whose`, controller], [`${path}/who`, controller],
    [`${path}/library/owner`, controller], [`${path}/to/owner`, controller],
  ]);
  assert.deepEqual(graph.diagnostics.filter(d => d.path.startsWith(ability)), []);
  assert.deepEqual(graph.engineFeatures.map(need => [need.feature, need.path]), [[step.effect, path]]);
  assert.equal(graph.executable, false);

  const variant = parse(source.oracle_text.replace("first main phase", "precombat main phase")
    .replace("add three mana", "add two mana"), "Another Card");
  assert.ok(variant.ok);
  assert.deepEqual(variant.abilities[1], { ...expectedTrigger,
    effects: [{ steps: [{ ...step, mana: { amount: "fixed", value: 2 } }] }] });
  for (const [original, mutation] of [
    ["You may reveal", "You must reveal"],
    ["colored mana symbols", "different colors of mana symbols"],
    ["of its colors", "of any colors"],
    ["If you don't reveal it, put it into your hand.", "If you don't reveal it, exile it."],
    ["If you don't reveal it, put it into your hand.", ""],
  ]) {
    const mutated = source.oracle_text.replace(original, mutation);
    assert.notEqual(mutated, source.oracle_text, original);
    assert.equal(parse(mutated).ok, false, mutation);
  }
});
