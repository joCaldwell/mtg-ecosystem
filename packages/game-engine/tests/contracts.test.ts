import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import { captureJsonOutput } from "../../../scripts/capture-json-output.ts";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import { parseOracleText } from "../../oracle-parser/src/index.ts";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const fixtureDir = new URL("fixtures/", import.meta.url);
const sha = (value: string) => createHash("sha256").update(value).digest("hex");
const read = (name: string) => fs.readFileSync(new URL(name, fixtureDir), "utf8");
const IR_SCHEMA_VERSION = 1;
const preflight = process.env.MTG_ENGINE_CONTRACT_PREFLIGHT === "1";
// The normal acceptance run registers every existing test. Only the explicit
// early preflight omits runtime-backed cases; it repeats cheap source/schema
// contracts so new catalog negatives cannot wait behind the recovery demo.
function runtimeTest(name: string, body: () => void) {
  if (!preflight) test(name, body);
}
function run(command: string, ...args: string[]) {
  if (preflight && command !== "schema" && command !== "catalog") {
    throw new Error(`Runtime command ${command} is forbidden in contract preflight`);
  }
  // The package gate already runs all Rust tests in debug; use the optimized CLI
  // for these profile-independent contracts and the full recovery demonstration.
  return captureJsonOutput<any>("cargo", ["run", "--quiet", "--locked", "--release", "-p", "mtg-sim", "--", command, ...args], { cwd: root });
}

function loadCatalog(name: string) {
  const report = run("catalog", "build", `packages/game-engine/tests/fixtures/catalogs/${name}.json`);
  return JSON.parse(fs.readFileSync(report.expected_ir, "utf8"));
}

test("the instruction schema accepts KIR nodes and rejects retired instruction tags", () => {
  const validate = new Ajv2020({strict: false, validateFormats: false}).compile(run("schema"));
  const catalog = loadCatalog("action");
  const spell = catalog.definitions.find((d: any) => d.name === "Cruel Edict").spell;
  const leaf = spell.instructions[0];
  spell.instructions = [{ ...leaf, node: { seq: [leaf.node, leaf.node] } }];
  assert.ok(validate(catalog), JSON.stringify(validate.errors));
  spell.instructions = [{ op: "scry", count: "1", source_block: 0 }];
  assert.equal(validate(catalog), false);
});

test("retained archive hashes agree with JavaScript and source parser", () => {
  const provenance = JSON.parse(read("provenance.json"));
  const archives = JSON.parse(read("accepted-archives.json"));
  for (const archive of archives) {
    const raw = read(archive.manifest_file);
    const header = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), archive.manifest_sha256);
    assert.equal(header.parser.revision, archive.parser_revision);
    assert.equal(header.parser.sourceSha256, archive.parser_source_sha256);
    assert.equal(header.corpus.sha256, archive.corpus_sha256);
  }
  for (const witness of provenance.records) {
    const archive = archives.find((a: any) => a.manifest_file === witness.archive);
    assert.ok(archive, witness.file);
    const raw = read(witness.file);
    const record = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), witness.record_sha256);
    assert.equal(sha(JSON.stringify(record.source)), record.sourceSha256);
    if (record.binding != null) assert.equal(sha(JSON.stringify(record.binding)), record.bindingSha256);
    else assert.equal(record.bindingSha256, undefined);
    if (record.resolution != null) assert.equal(sha(JSON.stringify(record.resolution)), record.resolutionSha256);
    assert.equal(record.sourceSha256, witness.source_sha256);
    assert.equal(record.astSha256 ?? null, witness.ast_sha256);
    if (record.astSha256 != null) {
      assert.equal(sha(JSON.stringify(record.faces.map((face: any) => face.result.abilities))), record.astSha256);
    }
    // Extraction also preserves unparsed records for rejection witnesses. They
    // never become executable catalog definitions without complete lowering.
    if (record.status !== "parsed") continue;
    const parsed = parseOracleText(record.source.oracle_text, record.source.name, {
      layout: record.source.layout, typeLine: record.source.type_line, includeSourceProvenance: true,
    });
    assert.equal(parsed.ok, true, record.source.name);
    // Archive interchange is JSON: optional undefined TS properties are absent.
    if (parsed.ok) assert.deepEqual(JSON.parse(JSON.stringify(parsed.abilities)), record.faces[0].result.abilities, record.source.name);
  }
});

runtimeTest("player CLI projections keep private card data and recovery fields out", () => {
  const own = run("observe", "0");
  const other = run("observe", "1");
  const spell = JSON.parse(read("phase1-r4/unsummon.json")).oracleIdentity;
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

test("Card IR accepts only schema 1", () => {
  const catalog = loadCatalog("base");
  const validate = new Ajv2020({strict:false,formats:{uint32:{type:"number",validate:(n:number)=>Number.isInteger(n)&&n>=0&&n<=0xffff_ffff}}}).compile(run("schema"));
  assert.equal(catalog.schema_version, IR_SCHEMA_VERSION);
  assert.equal(validate(catalog), true, JSON.stringify(validate.errors));
  for (const schema_version of [0, 2, 33, "1", null]) {
    assert.equal(validate({...catalog, schema_version}), false);
  }
  const missing = structuredClone(catalog);
  delete missing.schema_version;
  assert.equal(validate(missing), false);
});

test("Rust lowering matches reviewed IR and its generated JSON Schema", () => {
  const catalog = loadCatalog("base");
  assert.deepEqual(catalog, JSON.parse(read("expected-ir.json")));
  assert.equal(catalog.schema_version, IR_SCHEMA_VERSION);
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

test("match catalog binds sole defender and the current finite Commander policy", () => {
  const catalog=loadCatalog("match");
  assert.deepEqual(catalog,JSON.parse(read("expected-match-ir.json")));
  assert.equal(catalog.schema_version,IR_SCHEMA_VERSION);

  const wall=catalog.definitions.find((d: any)=>d.name==="Wall of Wood");assert.deepEqual(wall.keywords,["defender"]);assert.ok(wall.capabilities.includes("defender_v1"));
  assert.deepEqual(wall.characteristics.mana_cost,{generic:"0",colored:["G"]});assert.deepEqual([wall.characteristics.power,wall.characteristics.toughness],["0","3"]);assert.deepEqual(wall.characteristics.subtypes,["Wall"]);
  const validate=new Ajv2020({strict:false,formats:{uint32:{type:"number",validate:(n:number)=>Number.isInteger(n)&&n>=0&&n<=0xffff_ffff}}}).compile(run("schema"));

  for(const mutate of [(c:any)=>{c.definitions.find((d:any)=>d.name==="Wall of Wood").keywords=[{keyword:"defender"}];},(c:any)=>{c.definitions.find((d:any)=>d.name==="Wall of Wood").keywords=[{defender:null}];}]){const bad=structuredClone(catalog);mutate(bad);assert.equal(validate(bad),false);}
  const policy=selectedPolicies(catalog)[0];assert.equal(policy.policy_version,1);assert.ok(policy.retrieved_date);assert.equal(policy.html_sha256.length,64);assert.ok(policy.named_bans.length > 0);
  assert.equal(policy.offensive_policy.html_sha256.length,64);assert.ok(policy.offensive_policy.records.length > 0);assert.equal(policy.categories.lutri,"companion_only");
  const banned=new Set<string>();
  for(const item of [...policy.named_bans,...policy.offensive_policy.records]){
    assert.equal(banned.has(item.oracle_id),false);banned.add(item.oracle_id);
    assert.equal(sha(item.record_json),item.record_sha256);const r=JSON.parse(item.record_json);
    assert.equal(r.oracleIdentity,item.oracle_id);assert.equal(r.sourceIndex,item.source_index);assert.equal(sha(JSON.stringify(r.source)),item.source_sha256);assert.equal(r.sourceSha256,item.source_sha256);
    assert.ok(r.source.name===item.name||r.source.card_faces?.some((f:any)=>f.name===item.name));
  }
  assert.equal(banned.size,policy.named_bans.length+policy.offensive_policy.records.length);assertPoolReview(catalog);
  for(const d of catalog.definitions){assert.equal(banned.has(d.oracle_id),false);const source=JSON.parse(d.provenance.source_json);assert.equal(source.type_line.includes("Conspiracy"),false);assert.equal(/\bante\b/i.test(source.oracle_text),false);assert.equal(policy.offensive_policy.names.includes(source.name),false);}
});
let demoResult: any;
function demo() { return demoResult ??= run("demo"); }

runtimeTest("response observations expose stack order without reviving historical targets", () => {
  const response = demo().response;
  const before = response.stacked_observation;
  const after = response.after_top_observation;
  assert.equal(before.observation_version, 55);
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

runtimeTest("casting mana witnesses expose the caster request and preserve keep/undo outcomes", () => {
  const report = demo();
  const casting = report.mana_casting;
  assert.equal(casting.mana_observation.observation_version, 55);
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

runtimeTest("turn progression and permanent casting expose only permitted public state", () => {
  const turns = demo().turns;
  assert.equal(turns.status, "turns_permanents_verified");
  assert.deepEqual(turns.turn, { number: 5, step: "precombat_main", lands_played: 1 });
  const own = turns.final_observations[0];
  assert.equal(own.observation_version, 55);
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
});

runtimeTest("complete scenario games retain rules outcomes and private combat projections", () => {
  const combat = demo().combat;
  assert.equal(combat.length, 4);
  assert.deepEqual(combat[0].result, { result: "won", winner: 0 });
  assert.equal(combat[0].turn.number, 8);
  assert.equal(combat[0].final_observations[0].view.players[0].life, "4");
  assert.equal(combat[0].final_observations[0].view.players[0].library_count, 1);
  assert.deepEqual(combat[1].result, { result: "won", winner: 0 });
  assert.equal(combat[1].turn.number, 4);
  for (const run of combat) {
    assert.equal(run.journal.concessions.length, 0);
    assert.ok(run.checkpoints > 0);
    for (const observation of run.final_observations) {
      assert.equal(observation.observation_version, 55);
      assert.equal(observation.view.objects.some((o: any) => o.zone.zone === "library"), false);
      assert.equal(observation.view.objects.filter((o: any) => o.zone.zone === "hand").every((o: any) => o.owner === observation.view.viewer), true);
      for (const forbidden of ["incarnation", "damage_choices", "libraries", "frame", "rule_steps"]) {
        assert.equal(JSON.stringify(observation).includes(`"${forbidden}"`), false, forbidden);
      }
    }
  }
});

test("all data catalog manifests regenerate their committed IR and digests", () => {
  for (const name of fs.readdirSync(new URL("catalogs/", fixtureDir)).filter(n => n.endsWith(".json"))) {
    const path = fileURLToPath(new URL(`catalogs/${name}`, fixtureDir));
    const manifest = JSON.parse(fs.readFileSync(path, "utf8"));
    const report = run("catalog", "build", path);
    assert.equal(report.definitions, manifest.records.length);
    assert.equal(report.schema_version, manifest.schema_version);
  }
});

// Each profile's list is resolved relative to its own data file, as in build.rs.
function selectedPolicies(catalog: any): any[] {
  const profiles = new URL("profiles/", fixtureDir);
  const policies = new Map<string, any>();
  for (const name of fs.readdirSync(profiles).filter(n => n.endsWith(".json"))) {
    const profileURL = new URL(name, profiles);
    const profile = JSON.parse(fs.readFileSync(profileURL, "utf8"));
    const manifestURL = new URL(profile.catalog_manifest, profileURL);
    const manifest = JSON.parse(fs.readFileSync(manifestURL, "utf8"));
    const expected = JSON.parse(fs.readFileSync(new URL(manifest.expected_ir, manifestURL), "utf8"));
    if (JSON.stringify(expected) !== JSON.stringify(catalog)) continue;
    const url = new URL(profile.commander_policy.banned_list, profileURL);
    policies.set(url.href, JSON.parse(fs.readFileSync(url, "utf8")));
  }
  assert.ok(policies.size > 0, "catalog must have a profile-selected policy");
  return [...policies.values()];
}
// Review derives from the selected dated ban data and each retained source.
function assertPoolReview(catalog: any) {
  for (const bans of selectedPolicies(catalog)) {
    const banned=new Set([...bans.named_bans,...bans.offensive_policy.records].map((b:any)=>b.oracle_id));
    for(const d of catalog.definitions) {
      const source=JSON.parse(d.provenance.source_json);
      assert.equal(source.legalities.commander,"legal",d.name);
      assert.equal(/\bConspiracy\b/.test(source.type_line)||/\bante\b/i.test(source.oracle_text),false,d.name);
      assert.equal(banned.has(d.oracle_id),false,d.name);
      assert.equal(sha(d.provenance.source_json),d.provenance.source_sha256,d.name);
    }
  }
}
