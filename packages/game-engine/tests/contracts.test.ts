import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import { parseOracleText } from "../../oracle-parser/src/index.ts";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const fixtureDir = new URL("fixtures/", import.meta.url);
const sha = (value: string) => createHash("sha256").update(value).digest("hex");
const read = (name: string) => fs.readFileSync(new URL(name, fixtureDir), "utf8");
function run(command: string, ...args: string[]) {
  const result = spawnSync("cargo", ["run", "--quiet", "--locked", "-p", "mtg-sim", "--", command, ...args], { cwd: root, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("retained archive hashes agree with JavaScript and source parser", () => {
  const provenance = JSON.parse(read("provenance.json"));
  assert.equal(sha(read("archive-manifest.json").trimEnd()), provenance.archive_manifest_sha256);
  for (const witness of provenance.records) {
    const raw = read(witness.file);
    const record = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), witness.record_sha256);
    assert.equal(sha(JSON.stringify(record.source)), record.sourceSha256);
    assert.equal(sha(JSON.stringify(record.binding)), record.bindingSha256);
    assert.equal(sha(JSON.stringify(record.faces.map((face: any) => face.result.abilities))), record.astSha256);
    const parsed = parseOracleText(record.source.oracle_text, record.source.name, {
      layout: record.source.layout, typeLine: record.source.type_line, includeSourceProvenance: true,
    });
    assert.equal(parsed.ok, true, record.source.name);
    // Archive interchange is JSON: optional undefined TS properties are absent.
    if (parsed.ok) assert.deepEqual(JSON.parse(JSON.stringify(parsed.abilities)), record.faces[0].result.abilities, record.source.name);
  }
});

test("player CLI projections keep private card data and recovery fields out", () => {
  const own = run("observe", "0");
  const other = run("observe", "1");
  const spell = JSON.parse(read("unsummon.json")).oracleIdentity;
  assert.equal(own.view.objects.some((o: any) => o.oracle_id === spell), true);
  assert.equal(other.view.objects.some((o: any) => o.oracle_id === spell), false);
  assert.equal(other.view.players[0].hand_count, 1);
  const context = run("context", "1", "independent-seed");
  assert.equal(context.sampling_seed, "independent-seed");
  const forbidden = new Set(["state_sha256", "state", "initial_snapshot", "journal", "incarnation", "rule_steps", "source_json", "frame", "game_seed", "trace", "verification", "batch_id", "next_batch_id", "next_event_sequence", "before"]);
  function check(value: unknown) {
    if (Array.isArray(value)) value.forEach(check);
    else if (value !== null && typeof value === "object") {
      for (const [key, child] of Object.entries(value)) { assert.equal(forbidden.has(key), false, key); check(child); }
    }
  }
  check(own); check(other); check(context);
  const ownHandles = new Set(own.view.objects.map((o: any) => o.handle));
  assert.equal(other.view.objects.some((o: any) => ownHandles.has(o.handle)), false);
});

test("Rust lowering matches reviewed IR and its generated JSON Schema", () => {
  const catalog = run("compile");
  assert.deepEqual(catalog, JSON.parse(read("expected-ir.json")));
  assert.equal(catalog.schema_version, 3);
  for (const definition of catalog.definitions) {
    if (definition.characteristics.kind === "creature") {
      assert.deepEqual(definition.spell, {
        timing: "main_phase", resolution: "permanent", functioning_zone: "stack",
        targets: [], instructions: [],
      });
      assert.equal(definition.capabilities.includes("cast_creature_v1"), true);
    } else if (definition.characteristics.kind === "land") {
      assert.equal(definition.spell, null);
      assert.equal(definition.capabilities.includes("land_play_v1"), true);
    }
  }
  const schema = run("schema");
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(schema);
  assert.equal(validate(catalog), true, JSON.stringify(validate.errors));
  for (const mutate of [
    (c: any) => { c.unknownSemanticField = true; },
    (c: any) => { c.schema_version = 4; },
    (c: any) => { c.schema_version = 2; },
    (c: any) => { delete c.definitions.find((d: any) => d.spell).spell.timing; },
    (c: any) => { delete c.definitions.find((d: any) => d.spell).spell.resolution; },
    (c: any) => { c.definitions.find((d: any) => d.spell).spell.timing = "anytime"; },
    (c: any) => { c.definitions.find((d: any) => d.spell).spell.resolution = "execute_text"; },
    (c: any) => { c.definitions[0].characteristics.power = 2; },
    (c: any) => { c.definitions[0].characteristics.power = "02"; },
    (c: any) => { c.definitions.find((d: any) => d.characteristics.kind === "instant").spell.instructions[0].op = "execute_text"; },
    (c: any) => { c.definitions.find((d: any) => d.characteristics.kind === "instant").spell.instructions[0].extra = true; },
  ]) {
    const bad = structuredClone(catalog); mutate(bad);
    assert.equal(validate(bad), false);
  }
});

// The deterministic CLI demo verifies all retained families; share that expensive
// execution across projection assertions in this process.
let demoResult: any;
function demo() { return demoResult ??= run("demo"); }

test("response observations expose stack order without reviving historical targets", () => {
  const response = demo().response;
  const before = response.stacked_observation;
  const after = response.after_top_observation;
  assert.equal(before.observation_version, 5);
  assert.deepEqual(before.view.stack.map((s: any) => s.controller), [0, 2]);
  assert.equal(before.view.stack.every((s: any) => s.cast_complete), true);
  assert.equal(after.view.stack.length, 1);
  assert.equal(after.view.stack[0].targets[0].announced, before.view.stack[0].targets[0].announced);
  assert.equal(after.view.stack[0].targets[0].current, null);
  assert.equal(after.view.priority, 0);
  for (const observation of response.final_observations) {
    assert.equal(observation.view.objects.filter((o: any) => o.zone.zone === "hand").every((o: any) => o.owner === observation.view.viewer), true);
    assert.equal(JSON.stringify(observation).includes('"incarnation"'), false);
    assert.equal(JSON.stringify(observation).includes('"stack_id"'), false);
    assert.equal(observation.view.stack.length, 0);
  }
});

test("casting mana witnesses expose the caster request and preserve keep/undo outcomes", () => {
  const report = demo();
  const casting = report.mana_casting;
  assert.equal(casting.mana_observation.observation_version, 5);
  assert.equal(casting.mana_observation.decision.request, "mana");
  assert.deepEqual(casting.mana_observation.decision.cost, { generic: "0", colored: ["U"] });
  assert.equal(casting.mana_observation.view.priority, null);
  assert.equal(casting.mana_observation.view.stack[0].cast_complete, false);
  assert.equal(casting.journal.decisions[0].response.response, "cast");
  assert.equal(casting.journal.decisions[2].response.response, "activate_mana");
  for (const [name, retained, tapped] of [["mana_undo", "1", false], ["mana_keep", "2", true]] as const) {
    const witness = report[name];
    const own = witness.final_observations[0];
    assert.equal(own.view.players[0].mana.G, retained);
    assert.equal(own.view.stack.length, 0);
    // Both sources are Forest; the first was floated before casting.
    const forests = own.view.objects.filter((o: any) => o.zone.zone === "battlefield" && o.owner === 0);
    assert.deepEqual(forests.map((o: any) => o.tapped), [true, tapped]);
    assert.equal(witness.events.filter((e: any) => e.actual?.outcome === "mana_reversed").length, name === "mana_undo" ? 1 : 0);
    for (const observation of witness.final_observations) {
      assert.equal(JSON.stringify(observation).includes('"mana_activations"'), false);
      assert.equal(JSON.stringify(observation).includes('"incarnation"'), false);
      assert.equal(observation.view.objects.filter((o: any) => o.zone.zone === "hand").every((o: any) => o.owner === observation.view.viewer), true);
    }
  }
});


test("turn progression and permanent casting expose only permitted public state", () => {
  const turns = demo().turns;
  assert.equal(turns.status, "turns_permanents_verified");
  assert.deepEqual(turns.turn, { number: 5, step: "precombat_main", lands_played: 1 });
  const own = turns.final_observations[0];
  assert.equal(own.observation_version, 5);
  assert.deepEqual(own.view.turn, turns.turn);
  assert.equal(own.view.priority, 0);
  assert.equal(own.view.stack.length, 0);
  assert.equal(own.view.objects.filter((o: any) => o.zone.zone === "battlefield").length, 4);
  for (const view of turns.final_observations.map((o: any) => o.view)) {
    assert.equal(view.objects.every((o: any) => !o.tapped), true);
    assert.equal(view.objects.filter((o: any) => o.zone.zone === "hand").every((o: any) => o.owner === view.viewer), true);
    assert.equal(view.objects.some((o: any) => o.zone.zone === "library"), false);
    assert.equal(view.players.every((p: any) => p.library_count === 0 && Object.keys(p.mana).length === 0 && p.life === "40"), true);
    assert.equal(JSON.stringify(view).includes('"incarnation"'), false);
  }
  const expected = JSON.parse(read("expected-turns-scenario.json"));
  assert.equal(turns.state_sha256, expected.turns.state_sha256);
  assert.deepEqual(turns.journal, expected.turns.journal);
});
