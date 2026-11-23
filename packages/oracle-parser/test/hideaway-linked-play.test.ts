import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { analyzeCardBindings, parseOracleText, type BindingSourceCard } from "../src/index.ts";

it("binds frozen Mosswort Bridge's conditional play to its own hideaway incarnation", () => {
  const fixture = new URL("./fixtures/ramos/7cb9e29f-835f-4155-a2a5-4b778866c773.json", import.meta.url);
  const bytes = readFileSync(fixture);
  assert.equal(createHash("sha256").update(bytes).digest("hex"), "4aa052ccabe42cbb7088633b898dd00c99ab9fbb7a267120d5bee93e1de04a06");
  const { source, oracleIdentity: identity, faces } = JSON.parse(bytes.toString()) as {
    source: BindingSourceCard & { oracle_text: string }; oracleIdentity: string;
    faces: { result: ReturnType<typeof parseOracleText> }[];
  };
  const parse = (text: string, name = source.name) => parseOracleText(text, name, {
    typeLine: source.type_line ?? undefined, layout: source.layout, includeSourceProvenance: true,
  });
  const bind = (result: ReturnType<typeof parseOracleText>, name = source.name) =>
    analyzeCardBindings({ identity, source: { ...source, name }, faces: [{ sourceFaceIndex: 0, result }] });
  const result = parse(source.oracle_text);
  assert.ok(result.ok);
  // All four serialized abilities, including the separate tapped entry, mana
  // ability, activation costs and final instruction, retain the historical AST.
  assert.ok(faces[0].result.ok);
  assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), faces[0].result.abilities);
  const graph = bind(result);
  const id = (path: string, role: string) => `card:${identity}${path}#${role}`;
  const ability = "/faces/0/abilities";
  const keyword = `${ability}/0/keywords/0`;
  const activation = `${ability}/3`;
  const play = `${activation}/effects/0/steps/0`;
  const at = `${play}/instruction`;
  const condition = `${at}/condition`;
  const among = `${condition}/value/among`;
  const pool = id(keyword, "pool");
  const activator = id(activation, "you");
  const node = (id: string) => graph.nodes.find(n => n.id === id);
  const target = (path: string) => {
    const reference = graph.references.find(r => r.path === path);
    assert.ok(reference?.binding === "bound", path);
    return reference.target;
  };
  assert.deepEqual(node(pool), {
    id: pool, path: keyword, node: "hideaway-exiled-pool",
    keyword: id(keyword, "keyword"), source: id("/source", "object"), key: "source-incarnation",
    libraryOwner: id(keyword, "you"), lookCount: 4, exile: "one-face-down", remainder: "bottom-random",
    lookPermission: "controller-of-exiling-permanent", membership: "still-in-exile", cardinality: "many",
    rules: ["400.7", "406.3", "607.2a", "607.3", "702.75a"],
  });
  const triggerPlayer = node(id(keyword, "you"));
  assert.ok(triggerPlayer?.node === "runtime-player" && triggerPlayer.role === "trigger-controller");
  const player = node(activator);
  assert.ok(player?.node === "runtime-player" && player.role === "activator");
  assert.equal(target(`${at}/what`), pool);
  for (const path of [`${at}/who`, `${at}/alternativeCost/payer`, `${among}/filter/control`,
    `${activation}/costs/0/payer`, `${activation}/costs/1/payer`]) assert.equal(target(path), activator);
  assert.equal(target(`${ability}/1/effect/what`), id("/source", "object"));
  assert.equal(target(`${ability}/2/effects/0/steps/0/who`), id(`${ability}/2`, "you"));
  const query = node(target(among));
  assert.ok(query?.node === "selection" && query.timing === "resolution" && query.scope === id(activation, "ability"));
  assert.deepEqual(query.expression, { ref: "each", filter: { types: ["creature"], control: "you" } });
  const gate = node(id(condition, "condition"));
  assert.ok(gate?.node === "total-power-condition" && gate.timing === "resolution" && gate.creatures === query.id);
  assert.deepEqual(gate.comparison, { op: "ge", value: { amount: "fixed", value: 10 } });
  const event = node(id(play, "event"));
  assert.ok(event?.node === "event" && event.operation === "play-card" && event.optional && event.conditional);
  assert.equal(event.decider, activator);
  assert.equal(event.condition, gate.id);
  assert.deepEqual(event.inputs, [pool, activator]);
  assert.deepEqual(graph.diagnostics, []);
  assert.deepEqual(graph.counts, { obligations: 12, bound: 12, unresolved: 0 });
  assert.equal(graph.referencesBound, true);
  assert.deepEqual(graph.engineFeatures.map(n => [n.feature, n.path]), [["hideaway-resolution", keyword], ["linked-exile-play", play]]);
  assert.equal(graph.fullyBound, false);
  assert.equal(graph.executable, false);

  const [hideaway, ...others] = source.oracle_text.split("\n");
  const renamed = "Another Land";
  const reordered = bind(parse([...others, hideaway].join("\n"), renamed), renamed);
  assert.equal(reordered.referencesBound, true);
  assert.ok(reordered.references.some(r => r.path.endsWith("/instruction/what") && r.binding === "bound" &&
    r.target === id(`${ability}/3/keywords/0`, "pool")));

  for (const mutation of [
    others.join("\n"), [hideaway, source.oracle_text].join("\n"),
    source.oracle_text + "\n{T}: Exile target creature.",
    source.oracle_text.replace("Hideaway 4", "Hideaway 5"),
    source.oracle_text.replace("{G}, {T}:", "{T}:"),
    source.oracle_text.replace("play the exiled card", "cast the exiled card"),
    source.oracle_text.replace("you control", "an opponent controls"),
    source.oracle_text.replace("power 10", "power 9"),
  ]) {
    const rejected = bind(parse(mutation));
    assert.equal(rejected.referencesBound, false, mutation);
    assert.ok(!rejected.engineFeatures.some(n => n.feature === "linked-exile-play"), mutation);
  }
});
