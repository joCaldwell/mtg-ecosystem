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
function run(command: string, ...args: string[]) {
  // The package gate already runs all Rust tests in debug; use the optimized CLI
  // for these profile-independent contracts and the full recovery demonstration.
  return captureJsonOutput<any>("cargo", ["run", "--quiet", "--locked", "--release", "-p", "mtg-sim", "--", command, ...args], { cwd: root });
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
    (c: any) => { c.schema_version = 10; },
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

test("Commander catalog adds exact archived Plains without changing card costs", () => {
  const original = run("compile");
  const catalog = run("compile-commander");
  assert.deepEqual(catalog, JSON.parse(read("expected-commander-ir.json")));
  assert.equal(catalog.schema_version, original.schema_version);
  assert.equal(catalog.definitions.length, original.definitions.length + 1);
  for (const definition of original.definitions) {
    assert.deepEqual(catalog.definitions.find((d: any) => d.oracle_id === definition.oracle_id), definition);
  }
  const plains = catalog.definitions.find((d: any) => d.oracle_id === "bc71ebf6-2056-41f7-be35-b2e5c34afa99");
  const source = JSON.parse(read("plains.json"));
  assert.equal(plains.provenance.source_json, JSON.stringify(source.source));
  assert.equal(plains.provenance.source_sha256, source.sourceSha256);
  assert.equal(plains.provenance.source_index, 28278);
  assert.equal(plains.characteristics.kind, "land");
  assert.equal(plains.characteristics.basic, true);
  assert.deepEqual(plains.characteristics.subtypes, ["Plains"]);
  assert.deepEqual(plains.characteristics.colors, []);
  assert.deepEqual(plains.characteristics.color_identity, ["W"]);
  assert.equal(plains.intrinsic_zone, "battlefield");
  assert.equal(plains.spell, null);
  assert.deepEqual(plains.capabilities, ["normal_layout_v1", "basic_land_mana_v1", "land_play_v1"]);
  const commander = catalog.definitions.find((d: any) => d.oracle_id === "cfb756dd-6fe1-430a-8f04-8b10cac62a86");
  assert.deepEqual(commander.characteristics.mana_cost, { generic: "0", colored: ["W"] });
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(run("schema"));
  assert.equal(validate(catalog), true, JSON.stringify(validate.errors));
});

test("library and damage catalogs retain exact source semantics and explicit IR vocabularies", () => {
  const previous = run("compile-commander");
  const library = run("compile-library");
  const damage = run("compile-damage");
  assert.deepEqual(library, JSON.parse(read("expected-library-ir.json")));
  assert.deepEqual(damage, JSON.parse(read("expected-damage-ir.json")));
  assert.equal(library.schema_version, 4);
  assert.equal(damage.schema_version, 3);
  for (const catalog of [library, damage]) {
    assert.equal(catalog.definitions.length, previous.definitions.length + 1);
    for (const definition of previous.definitions) {
      assert.deepEqual(catalog.definitions.find((d: any) => d.oracle_id === definition.oracle_id), definition);
    }
  }
  const ebb = library.definitions.find((d: any) => d.oracle_id === "30cc8f7b-3c28-40f5-8f8f-157e8212280b");
  const ebbSource = JSON.parse(read("time-ebb.json"));
  assert.equal(ebb.provenance.source_json, JSON.stringify(ebbSource.source));
  assert.equal(ebb.characteristics.kind, "sorcery");
  assert.deepEqual(ebb.characteristics.mana_cost, { generic: "2", colored: ["U"] });
  assert.deepEqual(ebb.spell, {
    timing: "main_phase", resolution: "instructions", functioning_zone: "stack",
    targets: [{ slot: 0, selection_id: "line:0:1", chooser: "controller", stage: "announcement", domain: "battlefield_creature" }],
    instructions: [{ op: "put_on_top_owner_library", target_slot: 0, source_block: 0 }],
  });
  assert.equal(ebb.capabilities.includes("cast_sorcery_v1"), true);
  assert.equal(ebb.capabilities.includes("owner_library_top_move_v1"), true);
  assert.equal(ebb.capabilities.includes("owner_hand_move_v1"), false);
  const yargle = damage.definitions.find((d: any) => d.oracle_id === "980e1721-3717-4425-95a4-938dbf2ac661");
  const yargleSource = JSON.parse(read("yargle-and-multani.json"));
  assert.equal(yargle.provenance.source_json, JSON.stringify(yargleSource.source));
  assert.equal(yargle.characteristics.legendary, true);
  assert.equal(yargle.characteristics.power, "18");
  assert.equal(yargle.characteristics.toughness, "6");
  assert.deepEqual(yargle.characteristics.mana_cost, { generic: "3", colored: ["B", "B", "G"] });
  assert.equal(yargle.provenance.abilities_json, "[]");
  assert.deepEqual(yargle.spell.instructions, []);
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(run("schema"));
  for (const catalog of [previous, library, damage]) assert.equal(validate(catalog), true, JSON.stringify(validate.errors));
  for (const mutate of [
    (c: any) => { c.schema_version = 3; },
    (c: any) => { c.schema_version = 10; },
    (c: any) => { c.definitions.find((d: any) => d.characteristics.kind === "sorcery").spell.instructions[0].op = "put_on_bottom_owner_library"; },
    (c: any) => { c.definitions.find((d: any) => d.characteristics.kind === "sorcery").spell.instructions[0].position = "top"; },
  ]) {
    const bad = structuredClone(library); mutate(bad);
    assert.equal(validate(bad), false);
  }
  const wrongVocabulary = structuredClone(previous);
  wrongVocabulary.definitions[0].capabilities.push("owner_library_top_move_v1");
  assert.equal(validate(wrongVocabulary), false);
});

test("action catalog preserves complete activated costs, opponent choice and schema version", () => {
  const catalog = run("compile-action");
  assert.deepEqual(catalog, JSON.parse(read("expected-action-ir.json")));
  assert.equal(catalog.schema_version, 5);
  assert.equal(catalog.definitions.length, 11);
  for (const name of ["expected-ir.json", "expected-commander-ir.json", "expected-damage-ir.json", "expected-library-ir.json"]) {
    for (const definition of JSON.parse(read(name)).definitions) {
      assert.deepEqual(catalog.definitions.find((d: any) => d.oracle_id === definition.oracle_id), definition);
      assert.equal(Object.hasOwn(definition, "activated"), false);
    }
  }
  const researcher = catalog.definitions.find((d: any) => d.oracle_id === "240a349a-79f8-4e11-9a05-a78148e70a62");
  assert.equal(researcher.provenance.source_json, JSON.stringify(JSON.parse(read("hapless-researcher.json")).source));
  assert.deepEqual(researcher.characteristics.mana_cost, { generic: "0", colored: ["U"] });
  assert.deepEqual(researcher.spell.instructions, []);
  assert.equal(researcher.spell.resolution, "permanent");
  assert.deepEqual(researcher.activated, [{
    functioning_zone: "battlefield", cost: "sacrifice_source",
    program: { functioning_zone: "stack", timing: "instant", resolution: "instructions", targets: [],
      instructions: [{ op: "draw_cards", count: "1", source_block: 0 }, { op: "discard_cards", count: "1", source_block: 0 }] },
  }]);
  const edict = catalog.definitions.find((d: any) => d.oracle_id === "10c585c4-bf5b-4d8f-94a9-e9a5036a688f");
  assert.deepEqual(edict.characteristics.mana_cost, { generic: "1", colored: ["B"] });
  assert.deepEqual(edict.spell.targets, [{ slot: 0, selection_id: "line:0:0", chooser: "controller", stage: "announcement", domain: "opponent_player" }]);
  assert.deepEqual(edict.spell.instructions, [{ op: "target_player_sacrifice_creature", target_slot: 0, source_block: 0 }]);
  assert.equal(edict.capabilities.includes("opponent_target_v1"), true);
  assert.equal(edict.capabilities.includes("creature_target_v1"), false);
  const swamp = catalog.definitions.find((d: any) => d.oracle_id === "56719f6a-1a6c-4c0a-8d21-18f7d7350b68");
  assert.deepEqual(swamp.characteristics.subtypes, ["Swamp"]);
  assert.deepEqual(swamp.characteristics.color_identity, ["B"]);
  assert.equal(swamp.intrinsic_zone, "battlefield");
  assert.equal(catalog.definitions.some((d: any) => d.oracle_id === "310f141c-7f37-4729-aed6-dd9c09db448d"), false);
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(run("schema"));
  assert.equal(validate(catalog), true, JSON.stringify(validate.errors));
  for (const mutate of [
    (c: any) => { c.schema_version = 3; },
    (c: any) => { c.schema_version = 4; },
    (c: any) => { c.schema_version = 10; },
    (c: any) => { c.definitions.find((d: any) => d.activated).activated[0].cost = "tap_source"; },
    (c: any) => { c.definitions.find((d: any) => d.activated).activated[0].program.instructions[0].count = "01"; },
    (c: any) => { c.definitions.find((d: any) => d.activated).activated[0].program.instructions[1].optional = true; },
  ]) {
    const bad = structuredClone(catalog); mutate(bad);
    assert.equal(validate(bad), false);
  }
});

test("stack catalog preserves complete counter domains and distinct tap and mana costs", () => {
  const catalog = run("compile-stack");
  const previous = run("compile-action");
  assert.deepEqual(catalog, JSON.parse(read("expected-stack-ir.json")));
  assert.equal(catalog.schema_version, 6);
  assert.equal(catalog.definitions.length, 15);
  for (const definition of previous.definitions) {
    assert.deepEqual(catalog.definitions.find((d: any) => d.oracle_id === definition.oracle_id), definition);
  }
  for (const [name, domain, colored] of [
    ["counterspell", "stack_spell", ["U", "U"]],
    ["stifle", "stack_activated_or_triggered_ability", ["U"]],
  ] as const) {
    const source = JSON.parse(read(`${name}.json`));
    const d = catalog.definitions.find((d: any) => d.oracle_id === source.oracleIdentity);
    assert.equal(d.provenance.source_json, JSON.stringify(source.source));
    assert.deepEqual(d.characteristics.mana_cost, { generic: "0", colored });
    assert.equal(d.characteristics.kind, "instant");
    assert.deepEqual(d.spell, {
      functioning_zone: "stack", timing: "instant", resolution: "instructions",
      targets: [{ slot: 0, selection_id: "line:0:1", chooser: "controller", stage: "announcement", domain }],
      instructions: [{ op: "counter_stack_object", target_slot: 0, source_block: 0 }],
    });
    assert.equal(d.capabilities.includes("counter_stack_object_v1"), true);
  }
  for (const [name, cost, instructions] of [
    ["merfolk-looter", "tap_source", ["draw_cards", "discard_cards"]],
    ["azure-mage", { mana: { cost: { generic: "3", colored: ["U"] } } }, ["draw_cards"]],
  ] as const) {
    const source = JSON.parse(read(`${name}.json`));
    const d = catalog.definitions.find((d: any) => d.oracle_id === source.oracleIdentity);
    assert.equal(d.provenance.source_json, JSON.stringify(source.source));
    assert.deepEqual(d.characteristics.mana_cost, { generic: "1", colored: ["U"] });
    assert.deepEqual(d.activated, [{ functioning_zone: "battlefield", cost,
      program: { functioning_zone: "stack", timing: "instant", resolution: "instructions", targets: [],
        instructions: instructions.map(op => ({ op, count: "1", source_block: 0 })) } }]);
  }
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(run("schema"));
  assert.equal(validate(catalog), true, JSON.stringify(validate.errors));
  for (const mutate of [
    (c: any) => { c.schema_version = 3; },
    (c: any) => { c.schema_version = 4; },
    (c: any) => { c.schema_version = 5; },
    (c: any) => { c.schema_version = 10; },
    (c: any) => { c.definitions.find((d: any) => d.name === "Stifle").spell.targets[0].domain = "stack_activated_ability"; },
    (c: any) => { c.definitions.find((d: any) => d.name === "Counterspell").spell.instructions[0].destination = "graveyard"; },
    (c: any) => { c.definitions.find((d: any) => d.name === "Azure Mage").activated[0].cost.mana.cost.extra = true; },
    (c: any) => { c.definitions.find((d: any) => d.name === "Merfolk Looter").activated[0].cost = ["tap_source", "sacrifice_source"]; },
  ]) {
    const bad = structuredClone(catalog); mutate(bad);
    assert.equal(validate(bad), false, JSON.stringify(validate.errors));
  }
  // Version downgrade rejects each new family independently; a counter must
  // not merely hide an otherwise accepted v5 mana/tap activation cost.
  for (const name of ["Counterspell", "Stifle", "Merfolk Looter", "Azure Mage"]) {
    const bad = structuredClone(catalog);
    bad.definitions = bad.definitions.filter((d: any) => d.name === name);
    bad.schema_version = 5;
    assert.equal(validate(bad), false, name);
  }
});

test("damage and life catalog preserves complete sources and explicit IR7 target domains", () => {
  const catalog = run("compile-damage-life");
  assert.deepEqual(catalog, JSON.parse(read("expected-damage-life-ir.json")));
  assert.equal(catalog.schema_version,7);
  assert.equal(catalog.definitions.length,21);
  for (const old of run("compile-stack").definitions) assert.deepEqual(catalog.definitions.find((d: any) => d.oracle_id === old.oracle_id),old);
  for (const name of ["lightning-bolt","prodigal-sorcerer","soul-feast","healing-hands","sacred-nectar","mountain"]) {
    const source=JSON.parse(read(`${name}.json`));
    const d=catalog.definitions.find((d: any) => d.oracle_id===source.oracleIdentity);
    assert.equal(d.provenance.source_json,JSON.stringify(source.source));
    assert.equal(d.provenance.source_sha256,source.sourceSha256);
    assert.equal(source.binding.fullyBound,true);
  }
  const get=(name: string) => catalog.definitions.find((d: any) => d.name===name);
  const bolt=get("Lightning Bolt");
  assert.deepEqual(bolt.characteristics.mana_cost,{generic:"0",colored:["R"]});
  assert.deepEqual(bolt.spell.targets,[{slot:0,selection_id:"line:0:6",chooser:"controller",stage:"announcement",domain:"any_damage_target"}]);
  assert.deepEqual(bolt.spell.instructions,[{op:"deal_damage",target_slot:0,amount:"3",source_block:0}]);
  const sorcerer=get("Prodigal Sorcerer");
  assert.deepEqual(sorcerer.characteristics.mana_cost,{generic:"2",colored:["U"]});
  assert.equal(sorcerer.characteristics.power,"1"); assert.equal(sorcerer.characteristics.toughness,"1");
  assert.deepEqual(sorcerer.activated,[{functioning_zone:"battlefield",cost:"tap_source",program:{functioning_zone:"stack",timing:"instant",resolution:"instructions",targets:[{slot:0,selection_id:"line:0:9",chooser:"controller",stage:"announcement",domain:"any_damage_target"}],instructions:[{op:"deal_damage",target_slot:0,amount:"1",source_block:0}]}}]);
  const soul=get("Soul Feast");
  assert.deepEqual(soul.characteristics.mana_cost,{generic:"3",colored:["B","B"]});
  assert.equal(soul.spell.targets[0].domain,"any_player");
  assert.deepEqual(soul.spell.instructions,[{op:"lose_life",recipient:{target:{target_slot:0}},amount:"4",source_block:0},{op:"gain_life",recipient:"controller",amount:"4",source_block:0}]);
  const hands=get("Healing Hands");
  assert.deepEqual(hands.characteristics.mana_cost,{generic:"2",colored:["W"]});
  assert.equal(hands.spell.targets[0].domain,"any_player");
  assert.deepEqual(hands.spell.instructions,[{op:"gain_life",recipient:{target:{target_slot:0}},amount:"4",source_block:0},{op:"draw_cards",count:"1",source_block:1}]);
  const nectar=get("Sacred Nectar");
  assert.deepEqual(nectar.characteristics.mana_cost,{generic:"1",colored:["W"]});
  assert.deepEqual(nectar.spell.targets,[]);
  assert.deepEqual(nectar.spell.instructions,[{op:"gain_life",recipient:"controller",amount:"4",source_block:0}]);
  assert.deepEqual(get("Mountain").characteristics.color_identity,["R"]);
  const flame=JSON.parse(read("flame-rift.json"));
  assert.equal(flame.binding.fullyBound,false);
  assert.equal(catalog.definitions.some((d: any) => d.oracle_id===flame.oracleIdentity),false);
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(run("schema"));
  assert.equal(validate(catalog),true,JSON.stringify(validate.errors));
  for (const name of ["Lightning Bolt","Prodigal Sorcerer","Soul Feast","Healing Hands","Sacred Nectar"]) {
    for (const version of [3,4,5,6]) {
      const bad=structuredClone(catalog);bad.schema_version=version;bad.definitions=bad.definitions.filter((d: any) => d.name===name);
      assert.equal(validate(bad),false,`${name} IR${version}`);
    }
  }
  for (const mutate of [
    (c: any) => {c.schema_version=10;},
    (c: any) => {c.definitions.find((d: any) => d.name==="Lightning Bolt").spell.targets[0].domain="creature_or_player";},
    (c: any) => {c.definitions.find((d: any) => d.name==="Soul Feast").spell.instructions[0].recipient={opponent:{target_slot:0}};},
    (c: any) => {c.definitions.find((d: any) => d.name==="Sacred Nectar").spell.instructions[0].recipient={controller:{extra:true}};},
    (c: any) => {c.definitions.find((d: any) => d.name==="Healing Hands").spell.instructions[1].count="01";},
    (c: any) => {c.definitions.find((d: any) => d.name==="Lightning Bolt").spell.instructions[0].amount=3;},
    (c: any) => {c.definitions.find((d: any) => d.name==="Lightning Bolt").spell.instructions[0].amount="0";},
    (c: any) => {c.definitions.find((d: any) => d.name==="Soul Feast").spell.instructions[0].amount="-4";},
  ]) {const bad=structuredClone(catalog);mutate(bad);assert.equal(validate(bad),false,JSON.stringify(validate.errors));}
  // Old vocabulary with a newly allowed spell-side draw also requires v7;
  // rejecting new capability names alone would miss this attempted downgrade.
  const legacy=run("compile");legacy.schema_version=6;
  const bounce=legacy.definitions.find((d: any) => d.name==="Unsummon");
  bounce.spell.instructions.push({op:"draw_cards",count:"1",source_block:1});
  bounce.capabilities.push("controller_draw_v1");
  assert.equal(validate(legacy),false);
});

test("trigger catalog binds mandatory self and other events without trusting archive diagnostics", () => {
  const catalog=run("compile-triggers");
  const previous=run("compile-damage-life");
  assert.deepEqual(catalog,JSON.parse(read("expected-trigger-ir.json")));
  assert.equal(catalog.schema_version,8);assert.equal(catalog.definitions.length,24);
  for (const d of previous.definitions) {
    assert.equal(d.triggered,undefined);
    assert.deepEqual(catalog.definitions.find((next: any) => next.oracle_id===d.oracle_id),d);
  }
  for (const name of ["blood-artist","elvish-visionary","essence-warden"]) {
    const record=JSON.parse(read(`${name}.json`));
    const d=catalog.definitions.find((d: any) => d.oracle_id===record.oracleIdentity);
    assert.equal(d.provenance.source_json,JSON.stringify(record.source));
    assert.equal(d.provenance.abilities_json,JSON.stringify(record.faces[0].result.abilities));
    assert.equal(d.provenance.binding_sha256,record.bindingSha256);
    assert.equal(d.triggered.length,1);
    assert.equal(d.triggered[0].functioning_zone,"battlefield");
    assert.equal(d.triggered[0].program.functioning_zone,"stack");
    assert.equal(d.triggered[0].program.timing,"instant");
    assert.equal(d.triggered[0].program.resolution,"instructions");
  }
  const get=(name: string) => catalog.definitions.find((d: any) => d.name===name);
  const visionary=get("Elvish Visionary");
  assert.deepEqual(visionary.characteristics.mana_cost,{generic:"1",colored:["G"]});
  assert.equal(visionary.triggered[0].condition,"self_enters_battlefield");
  assert.deepEqual(visionary.triggered[0].program.targets,[]);
  assert.deepEqual(visionary.triggered[0].program.instructions,[{op:"draw_cards",count:"1",source_block:0}]);
  const warden=get("Essence Warden");
  assert.deepEqual(warden.characteristics.mana_cost,{generic:"0",colored:["G"]});
  assert.equal(warden.triggered[0].condition,"other_creature_enters_battlefield");
  assert.deepEqual(warden.triggered[0].program.instructions,[{op:"gain_life",recipient:"controller",amount:"1",source_block:0}]);
  const artist=get("Blood Artist");
  assert.deepEqual(artist.characteristics.mana_cost,{generic:"1",colored:["B"]});
  assert.equal(artist.characteristics.power,"0");assert.equal(artist.characteristics.toughness,"1");
  assert.equal(artist.triggered[0].condition,"any_creature_dies");
  assert.deepEqual(artist.triggered[0].program.targets,[{slot:0,selection_id:"line:0:8",chooser:"controller",stage:"announcement",domain:"any_player"}]);
  assert.deepEqual(artist.triggered[0].program.instructions,[{op:"lose_life",recipient:{target:{target_slot:0}},amount:"1",source_block:0},{op:"gain_life",recipient:"controller",amount:"1",source_block:0}]);
  for (const name of ["blood-artist","essence-warden"]) assert.equal(JSON.parse(read(`${name}.json`)).binding.fullyBound,false);
  assert.equal(JSON.parse(read("elvish-visionary.json")).binding.fullyBound,true);
  for (const name of ["Deathgreeter","Flame Rift"]) assert.equal(catalog.definitions.some((d: any) => d.name===name),false);
  const optional=JSON.parse(read("deathgreeter.json"));
  assert.equal(optional.status,"parsed");assert.equal(optional.faces[0].result.abilities[0].effects[0].steps[0].optional,true);
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(run("schema"));
  assert.equal(validate(catalog),true,JSON.stringify(validate.errors));
  for (const name of ["Blood Artist","Essence Warden","Elvish Visionary"]) for (const version of [3,4,5,6,7]) {
    const bad=structuredClone(catalog);bad.definitions=bad.definitions.filter((d: any) => d.name===name);bad.schema_version=version;
    assert.equal(validate(bad),false,`${name} IR${version}`);
  }
  for (const mutate of [
    (c: any) => {c.schema_version=10;},
    (c: any) => {c.definitions.find((d: any) => d.name==="Blood Artist").triggered[0].optional=true;},
    (c: any) => {c.definitions.find((d: any) => d.name==="Blood Artist").triggered[0].cost="tap_source";},
    (c: any) => {c.definitions.find((d: any) => d.name==="Blood Artist").triggered[0].condition="other_creature_dies";},
    (c: any) => {c.definitions.find((d: any) => d.name==="Essence Warden").triggered[0].lookup="before_event";},
    (c: any) => {c.definitions.find((d: any) => d.name==="Elvish Visionary").triggered[0].program.instructions[0].count="01";},
  ]) {const bad=structuredClone(catalog);mutate(bad);assert.equal(validate(bad),false,JSON.stringify(validate.errors));}
  // A newer capability removed from the list cannot hide a triggered field
  // inside an older envelope: the version gate also constrains that field.
  const bad=structuredClone(catalog);bad.schema_version=7;
  for (const d of bad.definitions) d.capabilities=d.capabilities.filter((cap: string) => !["triggered_ability_v1","self_enters_battlefield_v1","other_creature_enters_battlefield_v1","creature_dies_v1"].includes(cap));
  assert.equal(validate(bad),false);
});

test("effects catalog preserves signed modifiers and complete prevention/replacement scopes", () => {
  const catalog=run("compile-effects");
  assert.deepEqual(catalog,JSON.parse(read("expected-effects-ir.json")));
  assert.equal(catalog.schema_version,9);assert.equal(catalog.definitions.length,30);
  for (const d of run("compile-triggers").definitions) {
    assert.equal(d.static_abilities,undefined);
    assert.deepEqual(catalog.definitions.find((next: any) => next.oracle_id===d.oracle_id),d);
  }
  const get=(name: string) => catalog.definitions.find((d: any) => d.name===name);
  for (const [file,name,generic,colored] of [
    ["giant-growth","Giant Growth","0",["G"]], ["disfigure","Disfigure","0",["B"]],
    ["fog","Fog","0",["G"]], ["samite-healer","Samite Healer","1",["W"]],
    ["furnace-of-rath","Furnace of Rath","1",["R","R","R"]], ["yoked-ox","Yoked Ox","0",["W"]],
  ] as const) {
    const record=JSON.parse(read(`${file}.json`));const d=get(name);
    assert.equal(record.status,"parsed");
    assert.equal(d.provenance.source_json,JSON.stringify(record.source));
    assert.equal(d.provenance.abilities_json,JSON.stringify(record.faces[0].result.abilities));
    assert.equal(d.provenance.source_sha256,record.sourceSha256);
    assert.equal(d.provenance.ast_sha256,record.astSha256);
    assert.equal(d.provenance.binding_sha256,record.bindingSha256);
    assert.deepEqual(d.characteristics.mana_cost,{generic,colored});
  }
  for (const [name,amount] of [["Giant Growth","3"],["Disfigure","-2"]] as const) {
    assert.deepEqual(get(name).spell.instructions,[{op:"modify_power_toughness",target_slot:0,power:amount,toughness:amount,duration:"until_end_of_turn",source_block:0}]);
    assert.deepEqual(get(name).spell.targets,[{slot:0,selection_id:"line:0:0",chooser:"controller",stage:"announcement",domain:"battlefield_creature"}]);
  }
  assert.deepEqual(get("Fog").spell.instructions,[{op:"prevent_combat_damage",duration:"until_end_of_turn",source_block:0}]);
  assert.deepEqual(get("Fog").spell.targets,[]);
  const healer=get("Samite Healer");
  assert.equal(healer.characteristics.power,"1");assert.equal(healer.characteristics.toughness,"1");
  assert.deepEqual(healer.activated,[{functioning_zone:"battlefield",cost:"tap_source",program:{timing:"instant",resolution:"instructions",functioning_zone:"stack",
    targets:[{slot:0,selection_id:"line:0:13",chooser:"controller",stage:"announcement",domain:"any_damage_target"}],
    instructions:[{op:"prevent_next_damage",target_slot:0,amount:"1",duration:"until_end_of_turn",source_block:0}]}}]);
  const furnace=get("Furnace of Rath");
  assert.equal(furnace.characteristics.kind,"enchantment");
  assert.deepEqual(furnace.static_abilities,[{functioning_zone:"battlefield",effect:"double_damage_to_permanent_or_player"}]);
  assert.deepEqual(furnace.spell,{timing:"main_phase",resolution:"permanent",functioning_zone:"stack",targets:[],instructions:[]});
  assert.equal(get("Yoked Ox").characteristics.power,"0");assert.equal(get("Yoked Ox").characteristics.toughness,"4");
  assert.deepEqual(JSON.parse(read("yoked-ox.json")).faces[0].result.abilities,[]);
  for (const file of ["fog","samite-healer","furnace-of-rath"]) assert.equal(JSON.parse(read(`${file}.json`)).binding.fullyBound,false);
  for (const file of ["giant-growth","disfigure","yoked-ox"]) assert.equal(JSON.parse(read(`${file}.json`)).binding.fullyBound,true);
  for (const name of ["Flame Rift","Deathgreeter","Righteousness","Healing Salve","Gratuitous Violence"]) assert.equal(catalog.definitions.some((d: any) => d.name===name),false);
  // The negative sources retain their distinguishing full-card syntax.
  assert.deepEqual(JSON.parse(read("righteousness.json")).faces[0].result.abilities[0].effects[0].steps[0].what.filter.status,["blocking"]);
  assert.equal(JSON.parse(read("healing-salve.json")).faces[0].result.abilities[0].effects[0].steps[0].effect,"modal");
  assert.deepEqual(JSON.parse(read("gratuitous-violence.json")).faces[0].result.abilities[0].effect.replacement.event.source.filter,{types:["creature"],control:"you"});
  const validate=new Ajv2020({strict:false,formats:{uint32:{type:"number",validate:(n: number)=>Number.isInteger(n)&&n>=0&&n<=0xffff_ffff}}}).compile(run("schema"));
  for (const fixture of ["expected-ir.json","expected-commander-ir.json","expected-damage-ir.json","expected-library-ir.json","expected-action-ir.json","expected-stack-ir.json","expected-damage-life-ir.json","expected-trigger-ir.json","expected-effects-ir.json"]) {
    assert.equal(validate(JSON.parse(read(fixture))),true,fixture+JSON.stringify(validate.errors));
  }
  const newCapabilities=new Set(["cast_enchantment_v1","temporary_power_toughness_v1","combat_damage_prevention_v1","damage_budget_prevention_v1","static_damage_replacement_v1"]);
  for (const name of ["Giant Growth","Disfigure","Fog","Samite Healer","Furnace of Rath"]) for (let version=3;version<9;version++) {
    const bad=structuredClone(catalog);bad.schema_version=version;bad.definitions=[structuredClone(get(name))];
    assert.equal(validate(bad),false,`${name} IR${version}`);
    bad.definitions[0].capabilities=bad.definitions[0].capabilities.filter((cap: string)=>!newCapabilities.has(cap));
    assert.equal(validate(bad),false,`stripped ${name} IR${version}`);
  }
  for (const mutate of [
    (c: any)=>{c.schema_version=10;},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Giant Growth").spell.instructions[0].power="-0";},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Disfigure").spell.instructions[0].toughness=-2;},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Fog").spell.instructions[0].duration="this_turn";},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Fog").spell.instructions[0].to=[];},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Samite Healer").activated[0].program.instructions[0].amount="0";},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Samite Healer").activated[0].program.instructions[0].amount="01";},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Samite Healer").activated[0].program.instructions[0].amount="-1";},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Furnace of Rath").static_abilities[0].effect="triple_damage";},
    (c: any)=>{c.definitions.find((d: any)=>d.name==="Furnace of Rath").static_abilities[0].optional=true;},
  ]) {const bad=structuredClone(catalog);mutate(bad);assert.equal(validate(bad),false,JSON.stringify(validate.errors));}
});

function currentProjectionVersions(value: any): any {
  if (Array.isArray(value)) return value.map(currentProjectionVersions);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, child]) =>
      [key, key === "observation_version" || key === "context_version" ? 13 : currentProjectionVersions(child)]));
  }
  return value;
}

// The deterministic CLI demo verifies all retained families; share that expensive
// execution across projection assertions in this process.
let demoResult: any;
function demo() { return demoResult ??= run("demo"); }

test("effects witnesses retain signed characteristics, actual damage and affected-player choices", () => {
  const reports=demo().effects;
  assert.deepEqual(reports,JSON.parse(read("expected-effects-scenario.json")));
  const policies=["growth","growth_countered","additive_modifiers","multi_caster_modifiers","zero_toughness_return","zero_toughness_decline","negative_power","growth_reincarnation","growth_caster_leaves","growth_cleanup","growth_active_leaves_cleanup","pumped_pinger","pumped_pinger_bounced","pumped_pinger_borrowed_owner_leaves","shield_player","shield_creature","shield_next_ping","shield_stifled","shield_source_bounced","shield_borrowed_owner_leaves","shield_target_reincarnation","shield_fizzle","furnace_cast","furnace_bolt","two_furnaces_bolt","furnace_source_leaves","double_then_prevent","prevent_then_double","fog_combat","fog_noncombat","fog_then_shield","shield_then_fog","shield_simultaneous_first","shield_simultaneous_last","prevented_death","amplified_death","commander_amplified","fog_caster_leaves","accepted_source_leaves","applied_prefix_source_leaves","bolt_eliminates_player"];
  assert.deepEqual(reports.map((r: any)=>r.policy),policies);
  const by=Object.fromEntries(reports.map((r: any)=>[r.policy,r])) as Record<string,any>;
  const actuals=(r: any,kind: string)=>r.events.filter((e: any)=>e.actual?.outcome===kind).map((e: any)=>e.actual);
  const events=(r: any,kind: string)=>r.events.filter((e: any)=>e.event===kind);
  const commits=(r: any)=>actuals(r,"damage_committed");
  const applications=(r: any)=>actuals(r,"damage_effect_applied").map((a: any)=>a.application);
  const damage=(r: any)=>commits(r).flatMap((c: any)=>c.actual);
  const effects=(r: any)=>actuals(r,"effect_created").map((a: any)=>a.effect);
  const choices=(r: any)=>r.choices.filter((o: any)=>o.decision.request==="damage_effect");
  const lives=(r: any)=>r.final_observations[3].view.players.map((p: any)=>p.life);
  const finalView=(r: any)=>r.final_observations[3].view;
  const ox=(r: any)=>finalView(r).objects.find((o: any)=>o.oracle_id==="19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8" && o.zone.zone==="battlefield");
  for (const [name,p,t] of [["growth","3","7"],["additive_modifiers","1","5"],["multi_caster_modifiers","4","8"],["negative_power","-2","2"],["growth_reincarnation","0","4"],["growth_countered","0","4"]] as const) {
    assert.equal(ox(by[name]).characteristics.power,p);assert.equal(ox(by[name]).characteristics.toughness,t);
  }
  const multi=effects(by.multi_caster_modifiers);
  assert.deepEqual(multi.map((e: any)=>[e.origin.source.card,e.origin.controller,e.kind.power,e.kind.toughness]),[[5,0,"3","3"],[24,1,"3","3"],[7,0,"-2","-2"]]);
  assert.equal(new Set(multi.map((e: any)=>e.id)).size,3);
  assert.equal(commits(by.negative_power).length,0);
  assert.ok(actuals(by.negative_power,"attackers_declared").length>0);
  assert.equal(effects(by.growth_countered).length,0);
  assert.equal(actuals(by.growth_countered,"stack_object_countered")[0].source.card,5);
  assert.equal(actuals(by.growth_countered,"stack_object_countered")[0].spell,true);
  for (const [name,zone] of [["zero_toughness_return","command"],["zero_toughness_decline","graveyard"]] as const) {
    const r=by[name];const death=actuals(r,"state_based_moved");
    assert.equal(death.length,1);assert.equal(death[0].reason,"zero_toughness");
    assert.deepEqual(death[0].movement.previous,{card:0,incarnation:0});
    assert.equal(death[0].movement.destination.zone,"graveyard");
    assert.equal(finalView(r).objects.find((o: any)=>o.oracle_id==="cfb756dd-6fe1-430a-8f04-8b10cac62a86").zone.zone,zone);
    assert.deepEqual(events(r,"trigger_collected").map((e: any)=>e.subject),Array(4).fill({card:0,incarnation:0}));
    assert.deepEqual(lives(r),["41","41","41","37"]);
  }
  const lethal=actuals(by.amplified_death,"state_based_moved");
  assert.equal(lethal.length,1);assert.equal(lethal[0].reason,"lethal_damage");
  assert.equal(lethal[0].movement.previous.card,16);
  assert.equal(damage(by.amplified_death)[0].amount,"2");
  assert.equal(events(by.amplified_death,"trigger_collected").length,4);
  assert.equal(events(by.prevented_death,"trigger_collected").length,0);
  assert.deepEqual(commits(by.prevented_death)[0].actual,[]);
  for (const name of ["pumped_pinger","pumped_pinger_bounced","pumped_pinger_borrowed_owner_leaves"]) {
    const hit=damage(by[name]);assert.equal(hit.length,1);assert.equal(hit[0].amount,"1");
    assert.deepEqual(hit[0].source.object,{card:3,incarnation:0});
    assert.equal(hit[0].source.characteristics.power,"4");assert.equal(hit[0].source.characteristics.toughness,"4");
    assert.equal(hit[0].source.tapped,true);assert.equal(hit[0].source.zone.zone,"battlefield");
    assert.equal(hit[0].source.owner,name.endsWith("borrowed_owner_leaves")?2:0);assert.equal(hit[0].controller,0);
    assert.deepEqual(hit[0].recipient,{recipient:"player",player:1});assert.equal(lives(by[name])[1],"39");
  }
  const bounced=by.pumped_pinger_bounced.final_observations[0].view.objects.find((o: any)=>o.oracle_id==="5e961d15-5972-4e4b-9385-1cd7cd7c6bbe");
  assert.equal(bounced.zone.zone,"hand");assert.equal(bounced.characteristics.power,"1");assert.equal(bounced.characteristics.toughness,"1");
  for (const name of ["growth_cleanup","growth_active_leaves_cleanup"]) {
    const r=by[name];const cleaned=actuals(r,"effects_cleaned").find((a: any)=>a.expired.length>0);
    assert.deepEqual(cleaned.expired,[0]);assert.equal(cleaned.previous.length,1);assert.equal(cleaned.previous[0][1],"3");
    const target=effects(r)[0].target_handles[3];const survivor=finalView(r).objects.find((o: any)=>o.handle===target);
    const stat=name==="growth_cleanup"?"1":"2";
    assert.equal(survivor.characteristics.power,stat);assert.equal(survivor.characteristics.toughness,stat);assert.equal(survivor.marked_damage,"0");
    assert.equal(finalView(r).effects?.some((e: any)=>e.effect==="modify_power_toughness")??false,false);
    assert.equal(actuals(r,"state_based_moved").length,0);
  }
  assert.equal(finalView(by.growth_caster_leaves).players[0].status,"left");
  const retained=finalView(by.growth_caster_leaves).effects.find((e: any)=>e.effect==="modify_power_toughness");
  assert.equal(retained.origin.controller,0);assert.equal(retained.origin.source.current,null);assert.notEqual(retained.target.current,null);
  for (const name of ["growth_reincarnation","shield_target_reincarnation"]) {
    const stale=finalView(by[name]).effects.find((e: any)=>e.effect===(name==="growth_reincarnation"?"modify_power_toughness":"prevent_next_damage"));
    const reference=name==="growth_reincarnation"?stale.target:stale.target.object;
    assert.equal(reference.current,null);assert.notEqual(reference.announced,ox(by[name]).handle);
  }
  assert.equal(effects(by.shield_fizzle).length,0);assert.equal(effects(by.shield_stifled).length,0);
  assert.equal(actuals(by.shield_stifled,"stack_object_countered")[0].spell,false);
  assert.deepEqual(actuals(by.shield_stifled,"stack_object_countered")[0].movement,null);
  for (const name of ["shield_player","shield_source_bounced","shield_borrowed_owner_leaves","accepted_source_leaves"]) {
    assert.equal(damage(by[name])[0].amount,"2");assert.equal(lives(by[name])[1],"38");
  }
  assert.deepEqual(commits(by.shield_next_ping).map((c: any)=>c.actual.map((d: any)=>d.amount)),[[],["3"]]);
  assert.deepEqual(applications(by.double_then_prevent).map((a: any)=>[a.before,a.after]),[["3","6"],["6","5"]]);
  assert.deepEqual(applications(by.prevent_then_double).map((a: any)=>[a.before,a.after]),[["3","2"],["2","4"]]);
  assert.equal(lives(by.double_then_prevent)[1],"35");assert.equal(lives(by.prevent_then_double)[1],"36");
  assert.deepEqual(applications(by.two_furnaces_bolt).map((a: any)=>[a.before,a.after]),[["3","6"],["6","12"]]);
  assert.deepEqual(applications(by.two_furnaces_bolt).map((a: any)=>a.effect.source.card),[31,32]);
  assert.equal(lives(by.two_furnaces_bolt)[1],"28");
  const furnaceCost=actuals(by.furnace_cast,"mana_paid_selected").find((a: any)=>a.cost.colored.length===3).cost;
  assert.deepEqual(furnaceCost,{generic:"1",colored:["R","R","R"]});
  assert.equal(finalView(by.furnace_cast).effects.filter((e: any)=>e.effect==="double_damage_to_permanent_or_player").length,1);
  assert.equal(applications(by.furnace_source_leaves).length,0);assert.equal(damage(by.furnace_source_leaves)[0].amount,"3");
  assert.deepEqual(commits(by.fog_combat)[0].actual,[]);assert.equal(damage(by.fog_noncombat)[0].amount,"3");
  const fogProof=by.fog_combat.effects_proofs.find((p: any)=>p.actual.outcome==="damage_committed");
  assert.equal(fogProof.before.damage_transaction.kind,"combat");
  assert.deepEqual(fogProof.before.damage_transaction.entries.map((e: any)=>[e.source.object.card,e.original_amount,e.amount]),[[0,"2","0"]]);
  const preventedProof=by.prevented_death.effects_proofs.find((p: any)=>p.actual.outcome==="damage_committed");
  assert.equal(preventedProof.before.damage_transaction.kind,"noncombat");
  assert.deepEqual(preventedProof.before.damage_transaction.entries.map((e: any)=>[e.source.object.card,e.recipient.object.card,e.original_amount,e.amount]),[[3,2,"1","0"]]);
  assert.deepEqual(commits(by.fog_then_shield).map((c: any)=>c.actual.map((d: any)=>d.amount)),[[],[]]);
  assert.deepEqual(commits(by.shield_then_fog).map((c: any)=>c.actual.map((d: any)=>d.amount)),[[],["1"]]);
  assert.deepEqual(applications(by.fog_then_shield).map((a: any)=>[a.effect.id,a.before,a.after,a.budget_before,a.budget_after]),[[1,"2","0",null,null],[0,"1","0","1","0"]]);
  assert.deepEqual(applications(by.shield_then_fog).map((a: any)=>[a.effect.id,a.before,a.after,a.budget_before,a.budget_after]),[[0,"2","1","1","0"],[1,"1","0",null,null]]);
  for (const [name,id,amounts] of [["shield_simultaneous_first",2,["1","2"]],["shield_simultaneous_last",3,["2","1"]]] as const) {
    const r=by[name];const choice=choices(r)[0];const candidates=choice.decision.candidates;
    assert.equal(choice.view.viewer,0);assert.equal(new Set(candidates.map((c: any)=>c.effect)).size,1);
    assert.equal(new Set(candidates.map((c: any)=>c.damage)).size,2);assert.equal(new Set(candidates.map((c: any)=>c.damage_source.announced)).size,2);
    assert.equal(applications(r)[0].damage,id);
    assert.deepEqual(damage(r).filter((d: any)=>d.recipient.object.card===0).map((d: any)=>d.amount),amounts);
    assert.equal(commits(r)[0].results.find((v: any)=>v.recipient.object.card===0).current,"3");
  }
  const accepted=by.accepted_source_leaves;
  assert.equal(accepted.journal.decisions.some((d: any)=>d.response.response==="damage_effect" && d.response.effect.effect==="static"),true);
  assert.equal(applications(accepted).every((a: any)=>a.effect.effect==="resolved"),true);
  const departed=accepted.events.findIndex((e: any)=>e.event==="player_departed" && e.player===2);
  const applied=accepted.events.findIndex((e: any)=>e.actual?.outcome==="damage_effect_applied");assert.ok(departed>=0 && applied>departed);
  const prefix=by.applied_prefix_source_leaves;
  assert.deepEqual(applications(prefix).map((a: any)=>[a.effect.effect,a.before,a.after]),[["static","3","6"],["resolved","6","5"]]);
  assert.equal(damage(prefix)[0].amount,"5");assert.equal(lives(prefix)[1],"35");
  const firstApplied=prefix.events.findIndex((e: any)=>e.actual?.outcome==="damage_effect_applied" && e.actual.application.effect.effect==="static");
  const prefixDeparture=prefix.events.findIndex((e: any)=>e.event==="player_departed" && e.player===2);
  const lastApplied=prefix.events.findIndex((e: any)=>e.actual?.outcome==="damage_effect_applied" && e.actual.application.effect.effect==="resolved");
  assert.ok(firstApplied>=0 && prefixDeparture>firstApplied && lastApplied>prefixDeparture);
  assert.equal(choices(prefix).some((o: any)=>o.decision.candidates.every((c: any)=>c.amount_before==="6")),true);
  assert.deepEqual(by.commander_amplified.commander_ledger.records["0"].combat_damage,{"1":"4"});
  assert.deepEqual(lives(by.bolt_eliminates_player),["4","-2","4","4"]);assert.equal(finalView(by.bolt_eliminates_player).players[1].status,"left");
  const privateFields=new Set(["incarnation","state_sha256","state","frame","trace","journal","effects_proofs","source_facts","source_handles","target_handles","transaction","next_effect_id","effect_id","damage_id","stack_id","applied","applications","source_block","cursor"]);
  function privacy(value: any) {
    if (Array.isArray(value)) value.forEach(privacy);
    else if (value!==null && typeof value==="object") for (const [key,child] of Object.entries(value)) {assert.equal(privateFields.has(key),false,key);privacy(child);}
  }
  for (const r of reports) {
    const proofKinds=new Set(["effect_created","damage_effect_applied","damage_committed","effects_cleaned"]);
    assert.ok(Array.isArray(r.effects_proofs));
    assert.deepEqual(r.effects_proofs.map((p: any)=>p.actual),r.events.filter((e: any)=>proofKinds.has(e.actual?.outcome)).map((e: any)=>e.actual));
    for (const p of r.effects_proofs.filter((p: any)=>p.actual.outcome==="damage_committed")) {
      const tx=p.before.damage_transaction;const outcome=p.actual;
      assert.ok(tx);assert.equal(tx.id,outcome.transaction);
      assert.equal(new Set(tx.entries.map((e: any)=>e.id)).size,tx.entries.length);
      const expectedIds=tx.entries.filter((e: any)=>BigInt(e.amount)>0n && !outcome.cancelled.includes(e.id)).map((e: any)=>e.id).sort((a: number,b: number)=>a-b);
      assert.deepEqual(outcome.actual.map((e: any)=>e.id).sort((a: number,b: number)=>a-b),expectedIds);
      for (const e of tx.entries) {
        assert.ok(BigInt(e.original_amount)>0n);assert.ok(BigInt(e.amount)>=0n);
        assert.equal(new Set(e.applied.map((key: any)=>JSON.stringify(key))).size,e.applied.length);
        const actual=outcome.actual.find((a: any)=>a.id===e.id);
        if (actual) {assert.equal(actual.amount,e.amount);assert.deepEqual(actual.source,e.source);assert.deepEqual(actual.recipient,e.recipient);assert.equal(actual.controller,e.controller);}
      }
      for (const result of outcome.results) {
        const sum=outcome.actual.filter((a: any)=>JSON.stringify(a.recipient)===JSON.stringify(result.recipient)).reduce((sum: bigint,a: any)=>sum+BigInt(a.amount),0n);
        assert.ok(sum>0n);
        assert.equal(BigInt(result.current)-BigInt(result.previous),result.recipient.recipient==="player"?-sum:sum);
      }
      if (!outcome.actual.length) {assert.deepEqual(outcome.results,[]);assert.deepEqual(outcome.commander_damage,[]);}
    }
    if (r.policy!=="commander_amplified") assert.deepEqual(r.commander_ledger.records["0"].combat_damage,{});
    for (const o of [...r.choices,...r.final_observations]) {
      privacy(o);assert.equal(o.observation_version,13);
      assert.equal(o.view.objects.some((obj: any)=>obj.zone.zone==="library"),false);
      assert.equal(o.view.objects.filter((obj: any)=>obj.zone.zone==="hand").every((obj: any)=>obj.owner===o.view.viewer),true);
      for (const e of o.view.effects??[]) assert.match(e.handle,new RegExp(`^v${o.view.viewer}-e\\d+$`));
      if (o.decision?.request!=="damage_effect") continue;
      const decision=o.decision;assert.equal(o.view.priority,null);assert.equal(decision.player,o.view.viewer);
      if (decision.recipient.entity==="player") assert.equal(decision.recipient.player,o.view.viewer);
      else assert.equal(o.view.objects.find((obj: any)=>obj.handle===decision.recipient.object).controller,o.view.viewer);
      const pairs=new Set<string>();
      for (const c of decision.candidates) {
        const pair=`${c.effect}:${c.damage}`;assert.equal(pairs.has(pair),false);pairs.add(pair);
        assert.match(c.effect,new RegExp(`^v${o.view.viewer}-e\\d+$`));assert.match(c.damage,new RegExp(`^v${o.view.viewer}-d\\d+$`));
        const visible=o.view.effects.find((e: any)=>e.handle===c.effect);assert.ok(visible);
        assert.equal(visible.origin.controller,c.effect_controller);assert.equal(visible.origin.oracle_id,c.effect_oracle_id);
        assert.ok(BigInt(c.amount_before)>0n);assert.ok(BigInt(c.amount_after)>=0n);
        if (c.kind==="double_damage") {assert.equal(BigInt(c.amount_after),BigInt(c.amount_before)*2n);assert.equal(c.budget_remaining,null);}
        else if (c.kind==="prevent_combat_damage") {assert.equal(c.amount_after,"0");assert.equal(c.budget_remaining,null);}
        else {assert.equal(c.kind,"prevent_next_damage");assert.equal(c.budget_remaining,"1");assert.equal(BigInt(c.amount_after),BigInt(c.amount_before)-1n);}
      }
    }
  }
});

test("ordinary trigger witnesses retain APNAP order, countered programs and historical sources", () => {
  const reports = demo().triggers;
  assert.deepEqual(reports, currentProjectionVersions(JSON.parse(read("expected-trigger-scenario.json"))));
  const expected: Record<string,number[]> = {
    // Collected, placed, completed, fizzled, life losses, life gains.
    visionary_etb: [4,4,4,0,0,3], visionary_stifled: [4,4,3,0,0,3],
    visionary_bounced: [4,4,4,0,0,3], warden_bounced: [4,4,4,0,0,3],
    warden_stifled: [4,4,3,0,0,2], visionary_countered: [0,0,0,0,0,0],
    empty_draw_gain_first: [4,4,4,0,0,3], empty_draw_draw_first: [4,4,3,0,0,2],
    artist_self_death: [5,5,5,0,5,5], artist_sacrifice: [5,5,5,0,5,5],
    artist_stifled: [5,5,4,0,4,4], artist_bounced: [5,5,5,0,5,5],
    artist_borrowed_owner_leaves: [5,4,4,0,4,4], artist_controller_leaves: [5,3,3,0,3,3],
    artist_retarget_accepted: [5,4,4,0,4,4], artist_target_leaves: [5,5,4,4,0,0],
    simultaneous_forward: [10,10,10,0,10,10], simultaneous_reverse: [10,10,10,0,10,10],
    commander_return: [5,5,5,0,5,5], commander_decline: [5,5,5,0,5,5],
    nonactive_activation: [5,5,5,0,5,5], researcher_empty_draw: [5,5,5,0,5,5],
    trigger_eliminates_active: [10,10,6,4,2,2],
  };
  assert.deepEqual(reports.map((r: any) => r.policy), Object.keys(expected));
  assert.equal(reports.reduce((sum: number,r: any) => sum+r.checkpoints,0),2960);
  const by = Object.fromEntries(reports.map((r: any) => [r.policy,r])) as Record<string,any>;
  const events = (r: any, kind: string) => r.events.filter((e: any) => e.event === kind);
  const actuals = (r: any, kind: string) => r.events.filter((e: any) => e.actual?.outcome === kind).map((e: any) => e.actual);
  const completedSubjects = (r: any) => {
    const collected = new Map(events(r,"trigger_collected").map((e: any) => [e.trigger_id,e]));
    const placed = new Map(events(r,"trigger_placed").map((e: any) => [e.stack_id,collected.get(e.trigger_id)]));
    return events(r,"triggered_ability_completed").map((e: any) => {
      const trigger = placed.get(e.stack_id) as any;
      return [trigger.source.card,trigger.subject.card];
    });
  };
  for (const r of reports) {
    const collected = events(r,"trigger_collected");
    const placed = events(r,"trigger_placed");
    const completed = events(r,"triggered_ability_completed");
    assert.deepEqual([collected.length,placed.length,completed.length,completed.filter((e: any) => !e.resolved).length,
      actuals(r,"life_lost").length,actuals(r,"life_gained").length],expected[r.policy],r.policy);
    for (const e of collected) {
      assert.deepEqual(e.source_facts.object,e.source);
      assert.deepEqual(e.source_facts.zone,{zone:"battlefield"});
      assert.equal(e.source_facts.controller,e.player);
      if ([3,4,17,29].includes(e.source.card)) assert.deepEqual(e.subject,{card:4,incarnation:2});
      else assert.equal(e.subject.incarnation,0);
    }
    for (const kind of ["life_lost","life_gained"]) for (const a of actuals(r,kind)) {
      assert.equal(a.amount,"1");
      assert.equal(BigInt(a.current)-BigInt(a.previous),kind === "life_lost" ? -1n : 1n);
    }
    assert.deepEqual(r.commander_ledger.records["0"].combat_damage,{});
  }
  for (const [policy,source,gains,losses] of [
    ["visionary_stifled",4,3,0], ["warden_stifled",3,2,0], ["artist_stifled",34,4,4],
  ] as const) {
    const r=by[policy];const counters=actuals(r,"stack_object_countered");
    assert.equal(counters.length,1);
    assert.equal(counters[0].source.card,source);
    assert.equal(counters[0].spell,false);assert.equal(counters[0].movement,null);
    assert.equal(events(r,"triggered_ability_completed").some((e: any) => e.stack_id===counters[0].target),false);
    assert.equal(actuals(r,"life_gained").length,gains);assert.equal(actuals(r,"life_lost").length,losses);
  }
  for (const name of ["simultaneous_forward","simultaneous_reverse"]) {
    const r=by[name];const c=events(r,"trigger_collected");
    for (const card of [1,2]) {
      assert.equal(c.filter((e: any) => e.source.card===card).length,2);
      assert.equal(c.filter((e: any) => e.subject.card===card).length,5);
    }
    assert.deepEqual(events(r,"trigger_placed").map((e: any) => e.player),[0,0,0,0,1,1,2,2,3,3]);
    assert.deepEqual(events(r,"triggered_ability_completed").map((e: any) => e.stack_id),events(r,"trigger_placed").map((e: any) => e.stack_id).reverse());
  }
  assert.deepEqual(completedSubjects(by.simultaneous_forward),[[34,2],[34,1],[28,2],[28,1],[16,2],[16,1],[2,2],[1,2],[2,1],[1,1]]);
  assert.deepEqual(completedSubjects(by.simultaneous_reverse),[[34,1],[34,2],[28,1],[28,2],[16,1],[16,2],[1,1],[2,1],[1,2],[2,2]]);
  // The chosen own trigger order determines whether the gain finishes before
  // the failed draw's SBA removes the controller and its remaining invocation.
  assert.deepEqual(completedSubjects(by.empty_draw_gain_first),[[29,4],[17,4],[3,4],[4,4]]);
  assert.deepEqual(completedSubjects(by.empty_draw_draw_first),[[29,4],[17,4],[4,4]]);
  for (const [name,life] of [["empty_draw_gain_first","41"],["empty_draw_draw_first","40"]] as const) {
    const r=by[name];const failed=r.events.findIndex((e: any) => e.actual?.outcome==="draw_failed");
    const complete=r.events.findIndex((e: any,index: number) => index>failed && e.event==="triggered_ability_completed");
    const lost=r.events.findIndex((e: any) => e.event==="players_lost");
    assert.ok(failed>=0 && complete>failed && lost>complete);
    assert.equal(r.final_observations[3].view.players[0].life,life);
    assert.equal(r.final_observations[3].view.players[0].status,"left");
  }
  const borrowed=events(by.artist_borrowed_owner_leaves,"trigger_collected").find((e: any) => e.source.card===1);
  assert.equal(borrowed.source_facts.owner,2);assert.equal(borrowed.source_facts.controller,0);
  assert.equal(by.artist_borrowed_owner_leaves.final_observations[3].view.players[0].life,"42");
  assert.equal(by.artist_borrowed_owner_leaves.final_observations[3].view.players[2].status,"left");
  for (const name of ["commander_return","commander_decline"]) {
    const choices=by[name].choices;
    const owner=choices.findIndex((o: any) => o.decision.request==="commander_return");
    const order=choices.findIndex((o: any) => o.decision.request==="order_triggers");
    assert.ok(owner>=0 && order>owner);
    assert.equal(choices[owner].view.stack.some((s: any) => s.triggered),false);
  }
  const nonactive=by.nonactive_activation.choices.find((o: any) => o.decision.request==="priority");
  assert.equal(nonactive.view.turn_owner,0);assert.equal(nonactive.view.priority,1);
  assert.equal(nonactive.view.stack.filter((s: any) => s.triggered).length,5);
  const privateFields=new Set(["incarnation","trigger_id","pending_triggers","source_handles","source_facts","state","state_sha256","frame","trace","journal","source_json"]);
  function checkProjection(value: any) {
    if (Array.isArray(value)) value.forEach(checkProjection);
    else if (value !== null && typeof value === "object") for (const [key,child] of Object.entries(value)) {
      assert.equal(privateFields.has(key),false,key);checkProjection(child);
    }
  }
  for (const r of reports) for (const o of [...r.choices,...r.final_observations]) {
    checkProjection(o);assert.equal(o.observation_version,13);
    assert.equal(o.view.objects.some((x: any) => x.zone.zone==="library"),false);
    assert.equal(o.view.objects.filter((x: any) => x.zone.zone==="hand").every((x: any) => x.owner===o.view.viewer),true);
    if (o.decision?.request==="order_triggers") {
      assert.equal(o.view.priority,null);
      assert.equal(new Set(o.decision.candidates.map((t: any) => t.handle)).size,o.decision.candidates.length);
      for (const t of o.decision.candidates) assert.match(t.handle,new RegExp(`^v${o.view.viewer}-t\\d+$`));
    }
    if (o.decision?.request==="player_targets") {
      assert.equal(o.view.priority,null);
      assert.equal(o.decision.candidates.includes(o.view.viewer),true);
      assert.equal(o.decision.occurrences[0].domain,"any_player");
      assert.equal(o.view.stack.at(-1).cast_complete,false);
    }
    for (const s of o.view.stack.filter((s: any) => s.triggered)) {
      assert.equal(s.spell,null);assert.equal(s.triggered.handle,s.handle);
      assert.match(s.handle,new RegExp(`^v${o.view.viewer}-s\\d+$`));
    }
  }
  const historical=by.artist_self_death.choices.find((o: any) => o.decision.request==="player_targets");
  assert.equal(historical.view.stack.at(-1).triggered.source.current,null);
});

test("damage and life witnesses retain ordered effects, mixed targets and historical sources", () => {
  const reports = demo().damage_life;
  assert.deepEqual(reports, currentProjectionVersions(JSON.parse(read("expected-damage-life-scenario.json"))));
  assert.equal(reports.length, 28);
  const by = Object.fromEntries(reports.map((r: any) => [r.policy, r])) as Record<string,any>;
  const actuals = (r: any, kind: string) => r.events.filter((e: any) => e.actual?.outcome === kind).map((e: any) => e.actual);
  const life = (r: any, player: number) => r.final_observations[3].view.players[player].life;
  assert.equal(life(by.bolt_player, 1), "37");
  assert.equal(life(by.bolt_self, 0), "37");
  assert.equal(life(by.pinger_player, 1), "39");
  assert.equal(life(by.soul_opponent, 0), "8");
  assert.equal(life(by.soul_self, 0), "4");
  assert.equal(life(by.healing_other, 1), "44");
  assert.equal(life(by.healing_failed_draw, 0), "44");
  assert.deepEqual(actuals(by.soul_self, "life_lost").map((e: any) => [e.player,e.previous,e.current]), [[0,"4","0"]]);
  assert.deepEqual(actuals(by.soul_self, "life_gained").map((e: any) => [e.player,e.previous,e.current]), [[0,"0","4"]]);
  for (const name of ["bolt_target_bounce","bolt_target_departure","bolt_countered","pinger_target_departure","pinger_stifled"]) {
    assert.deepEqual(actuals(by[name], "noncombat_damage_dealt"), [], name);
  }
  for (const name of ["soul_departed_target","soul_countered","healing_departed_target"]) {
    assert.deepEqual(actuals(by[name], "life_gained"), [], name);
    assert.deepEqual(actuals(by[name], "life_lost"), [], name);
  }
  for (const name of ["pinger_source_bounce","pinger_source_killed","pinger_borrowed_departure"]) {
    const hit = actuals(by[name], "noncombat_damage_dealt").find((e: any) => e.damage.source.object.card === 0);
    assert.equal(hit.damage.source.object.incarnation, 0);
    assert.deepEqual(hit.damage.source.zone, {zone:"battlefield"});
    assert.equal(hit.damage.source.tapped, true);
    assert.equal(hit.damage.controller, 0);
    assert.deepEqual(hit.damage.recipient, { recipient: "player", player: 1 });
    assert.equal(hit.damage.source.owner, name === "pinger_borrowed_departure" ? 2 : 0);
  }
  assert.equal(actuals(by.pinger_source_killed,"noncombat_damage_dealt").find((e: any) => e.damage.source.object.card === 0).damage.source.marked_damage,"3");
  assert.equal(actuals(by.pinger_stifled,"stack_object_countered")[0].spell,false);
  assert.equal(actuals(by.healing_failed_draw,"draw_failed")[0].player,0);
  assert.equal(actuals(by.healing_failed_draw,"lost")[0].reason,"failed_draw");
  const complete = by.healing_failed_draw.events.findIndex((e: any) => e.event === "spell_completed");
  const lost = by.healing_failed_draw.events.findIndex((e: any) => e.event === "players_lost");
  assert.ok(lost > complete);
  for (const name of ["healing_commander_return","healing_commander_decline"]) {
    const choices = by[name].choices.filter((o: any) => o.decision?.request === "commander_return");
    const draw = choices.find((o: any) => o.decision.destination.zone === "hand");
    assert.ok(draw, name);
    assert.equal(draw.decision.object,null);
    assert.equal(draw.view.objects.some((o: any) => o.zone.zone === "library"),false);
  }
  const privateFields = new Set(["incarnation","state_sha256","state","source_facts","pending","frame","trace","journal","source_json","next_stack_id"]);
  function checkProjection(value: any) {
    if (Array.isArray(value)) value.forEach(checkProjection);
    else if (value !== null && typeof value === "object") {
      for (const [key,child] of Object.entries(value)) {
        assert.equal(privateFields.has(key),false,key);
        checkProjection(child);
      }
    }
  }
  for (const r of reports) {
    r.choices.forEach(checkProjection);
    r.final_observations.forEach(checkProjection);
    assert.deepEqual(r.commander_ledger.records["14"].combat_damage,{});
    for (const o of r.choices) {
      if (o.decision?.request !== "entity_targets") continue;
      assert.equal(o.decision.candidates.some((t: any) => t.entity === "player" && t.player === o.view.viewer),true);
      for (const t of o.decision.candidates) {
        if (t.entity === "object") {
          assert.equal(typeof t.object,"string");
          assert.ok(o.view.objects.some((x: any) => x.handle === t.object && x.zone.zone === "battlefield"));
        } else assert.equal(t.entity,"player");
      }
    }
  }
});

test("stack and cost witnesses preserve instance targets, paid costs and nested choices", () => {
  const reports = demo().stack_costs;
  assert.deepEqual(reports, currentProjectionVersions(JSON.parse(read("expected-stack-costs-scenario.json"))));
  assert.equal(reports.length, 18);
  const by = Object.fromEntries(reports.map((r: any) => [r.policy,r])) as Record<string,any>;
  const countered = (r: any) => r.events.filter((e: any) => e.actual?.outcome === "stack_object_countered").map((e: any) => e.actual);
  const mana = (r: any, seat = 0) => Object.values(r.final_observations[seat].view.players[seat].mana).reduce<bigint>((sum,n) => sum + BigInt(n as string),0n);
  assert.equal(countered(by.counter_creature)[0].source.card,10);
  assert.equal(countered(by.counter_war)[0].source.card,11);
  assert.equal(by.counter_war.commander_ledger.records["0"].command_casts["0"],"1");
  assert.equal(by.counter_commander_recast.commander_ledger.records["0"].command_casts["0"],"2");
  assert.equal(by.counter_commander_recast.turn.number,5);
  assert.equal(by.counter_departed_target.events.some((e: any) => e.event === "spell_completed" && !e.resolved),true);
  assert.equal(mana(by.counter_departed_target,1),2n);
  for (const report of reports) for (const o of report.choices.filter((o: any) => o.decision.request === "stack_targets")) {
    assert.equal(o.view.priority,null);
    assert.equal(o.view.stack.at(-1).cast_complete,false);
    assert.equal(o.decision.candidates.includes(o.view.stack.at(-1).handle),false);
    for (const candidate of o.decision.candidates) {
      assert.match(candidate,new RegExp(`^v${o.view.viewer}-s\\d+$`));
      const entry = o.view.stack.find((s: any) => s.handle === candidate);
      assert.equal(entry.cast_complete,true);
      assert.equal(entry.ability !== undefined,o.decision.occurrences[0].domain === "stack_activated_or_triggered_ability");
    }
  }
  const duplicate = by.stifle_duplicate_source;
  const targeted = duplicate.choices.find((o: any) => o.decision.request === "stack_targets");
  const abilities = targeted.view.stack.filter((e: any) => e.ability);
  assert.equal(abilities.length,2);
  assert.notEqual(abilities[0].handle,abilities[1].handle);
  assert.equal(abilities[0].ability.source.announced,abilities[1].ability.source.announced);
  for (const a of abilities) {
    assert.equal(a.handle,a.ability.handle);
    assert.equal(a.ability.source.current,a.ability.source.announced);
  }
  const activated = duplicate.events.filter((e: any) => e.event === "ability_activated");
  assert.equal(countered(duplicate)[0].target,activated[0].stack_id);
  assert.deepEqual(duplicate.events.filter((e: any) => e.event === "ability_completed").map((e: any) => e.stack_id),[activated[1].stack_id]);
  assert.equal(mana(duplicate),0n);
  assert.equal(mana(by.stifle_paid_ability),4n);
  assert.equal(countered(by.stifle_hapless)[0].movement,null);
  assert.equal(by.stifle_hapless.events.some((e: any) => e.event === "ability_completed"),false);
  assert.equal(mana(by.mana_ability_keep),1n);
  assert.equal(mana(by.mana_ability_undo),0n);
  assert.equal(by.mana_ability_repeat.events.filter((e: any) => e.event === "ability_completed").length,2);
  assert.equal(mana(by.mana_ability_repeat),0n);
  assert.equal(by.borrowed_announcement_departure.final_observations[0].view.players[2].status,"left");
  const bounced = by.tap_ability_source_bounce.choices.find((o: any) => o.decision.request === "resolve_discard");
  assert.equal(bounced.view.stack[0].ability.source.current,null);
  const tapChoices = by.tap_commander_draw_discard.choices.filter((o: any) => o.decision.request === "commander_return");
  assert.deepEqual(tapChoices.map((o: any) => o.decision.destination.zone),["library","hand","graveyard"]);
  assert.equal(tapChoices[1].decision.object,null);
  const manaChoices = by.mana_commander_draw_return.choices.filter((o: any) => o.decision.request === "commander_return");
  assert.deepEqual(manaChoices.map((o: any) => o.decision.destination.zone),["library","hand"]);
  assert.equal(manaChoices[1].decision.object,null);
  assert.equal(by.mana_commander_draw_return.choices.some((o: any) => o.decision.request === "resolve_discard"),false);
  const failed = by.tap_failed_draw.events;
  const discarded = failed.findIndex((e: any) => e.actual?.outcome === "card_discarded");
  const completed = failed.findIndex((e: any) => e.event === "ability_completed");
  const lost = failed.findIndex((e: any) => e.event === "players_lost");
  assert.ok(discarded >= 0 && discarded < completed && completed < lost);
  for (const report of reports) for (const o of [...report.choices,...report.final_observations]) {
    assert.equal(o.observation_version,13);
    assert.equal(o.view.objects.some((x: any) => x.zone.zone === "library"),false);
    assert.equal(o.view.objects.filter((x: any) => x.zone.zone === "hand").every((x: any) => x.owner === o.view.viewer),true);
    for (const forbidden of ["incarnation","stack_id","source_handles","mana_activations","rule_steps"]) assert.equal(JSON.stringify(o).includes(`"${forbidden}"`),false);
  }
});

test("action witnesses preserve explicit spending, player sacrifice and noncard ability choices", () => {
  const reports = demo().actions_payments;
  assert.deepEqual(reports, currentProjectionVersions(JSON.parse(read("expected-actions-payments-scenario.json"))));
  assert.equal(reports.length, 14);
  const byPolicy = Object.fromEntries(reports.map((r: any) => [r.policy, r])) as Record<string, any>;
  const preserve = byPolicy.mixed_preserve_black;
  const spent = byPolicy.mixed_spend_black;
  assert.equal(preserve.final_observations[0].view.players[0].mana.B, "1");
  assert.equal(spent.final_observations[0].view.players[0].mana.B, "0");
  assert.equal(spent.final_observations[0].view.players[0].mana.G, "3");
  assert.deepEqual(preserve.journal.decisions.filter((d: any) => d.response.response === "spend_mana").map((d: any) => d.response.spending), [{B:"2",G:"4"},{B:"1",G:"1"}]);
  assert.equal(byPolicy.mixed_tax_recast.turn.number, 5);
  assert.equal(byPolicy.mixed_tax_recast.commander_ledger.records["0"].command_casts["0"], "2");
  assert.equal(byPolicy.edict_departure_before.events.some((e: any) => e.event === "spell_completed" && e.resolved === false), true);
  assert.equal(byPolicy.edict_departure_choice.events.some((e: any) => e.event === "spell_completed" && e.resolved === true), true);
  assert.equal(byPolicy.edict_empty.choices.some((o: any) => o.decision.request === "sacrifice"), false);
  for (const policy of ["ability_ordinary","ability_borrowed_owner_leaves","ability_empty_draw","ability_commander_return","ability_commander_discard"]) {
    const report = byPolicy[policy];
    const discard = report.choices.find((o: any) => o.decision.request === "resolve_discard");
    assert.equal(discard.view.viewer, 1);
    assert.equal(discard.view.priority, null);
    assert.equal(discard.view.stack.length, 1);
    const ability = discard.view.stack[0];
    assert.equal(ability.spell, null);
    assert.match(ability.ability.handle, /^v1-a\d+$/);
    assert.equal(ability.ability.source.current, null);
    assert.equal(ability.ability.oracle_id, "240a349a-79f8-4e11-9a05-a78148e70a62");
    assert.equal(report.events.filter((e: any) => e.event === "ability_activated").length, 1);
    assert.equal(report.events.filter((e: any) => e.event === "ability_completed").length, 1);
  }
  const nested = byPolicy.ability_commander_discard.choices.filter((o: any) => o.decision.request === "commander_return");
  assert.deepEqual(nested.map((o: any) => o.decision.destination.zone), ["library","hand","graveyard"]);
  assert.equal(nested[1].decision.object, null);
  assert.equal(nested[1].view.objects.some((o: any) => o.zone.zone === "library"), false);
  const failed = byPolicy.ability_empty_draw.events;
  const completed = failed.findIndex((e: any) => e.event === "ability_completed");
  const lost = failed.findIndex((e: any) => e.event === "players_lost");
  assert.ok(completed >= 0 && lost > completed);
  for (const report of reports) for (const o of [...report.choices,...report.final_observations]) {
    assert.equal(o.observation_version, 13);
    assert.equal(o.view.objects.some((x: any) => x.zone.zone === "library"), false);
    assert.equal(o.view.objects.filter((x: any) => x.zone.zone === "hand").every((x: any) => x.owner === o.view.viewer), true);
    for (const forbidden of ["incarnation","stack_id","source_handles","mana_activations","rule_steps"]) assert.equal(JSON.stringify(o).includes(`"${forbidden}"`),false);
  }
});

test("response observations expose stack order without reviving historical targets", () => {
  const response = demo().response;
  const before = response.stacked_observation;
  const after = response.after_top_observation;
  assert.equal(before.observation_version, 13);
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
  assert.equal(casting.mana_observation.observation_version, 13);
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
  assert.equal(own.observation_version, 13);
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
  const expected = JSON.parse(read("expected-combat-games.json"));
  assert.equal(turns.state_sha256, expected.turns.state_sha256);
  assert.deepEqual(turns.journal, expected.turns.journal);
});

test("complete scenario games retain rules outcomes and private combat projections", () => {
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
      assert.equal(observation.observation_version, 13);
      assert.equal(observation.view.objects.some((o: any) => o.zone.zone === "library"), false);
      assert.equal(observation.view.objects.filter((o: any) => o.zone.zone === "hand").every((o: any) => o.owner === observation.view.viewer), true);
      for (const forbidden of ["incarnation", "damage_choices", "libraries", "frame", "rule_steps"]) {
        assert.equal(JSON.stringify(observation).includes(`"${forbidden}"`), false, forbidden);
      }
    }
  }
  const expected = JSON.parse(read("expected-combat-games.json"));
  for (let n = 0; n < combat.length; n++) {
    assert.deepEqual(combat[n].journal, expected.combat[n].journal);
    assert.deepEqual(combat[n].result, expected.combat[n].result);
    assert.equal(combat[n].state_sha256, expected.combat[n].state_sha256);
  }
});

test("Commander witnesses preserve tax, actual return destinations and private projections", () => {
  const runs = demo().commander;
  assert.deepEqual(runs.map((r: any) => r.policy), [
    "tax_keep", "tax_undo", "hand_recast", "grave_return", "grave_decline", "exile_return", "exile_decline",
  ]);
  assert.deepEqual(runs, currentProjectionVersions(JSON.parse(read("expected-commander-scenario.json"))));
  for (const run of runs) {
    const payments = run.events.filter((e: any) => e.actual?.outcome === "mana_paid" && e.actual.player === 0)
      .map((e: any) => e.actual.cost);
    const taxed = run.policy === "tax_keep" || run.policy === "tax_undo";
    const expectedGeneric = taxed ? ["0", "2"] : run.policy === "hand_recast" ? ["0", "0"]
      : run.policy.endsWith("decline") ? [] : ["0"];
    assert.deepEqual(payments, expectedGeneric.map((generic: string) => ({ generic, colored: ["W"] })));
    assert.equal(run.events.filter((e: any) => e.event === "cast_reversed").length, taxed ? 1 : 0);
    const moves = run.events.map((e: any) => e.actual?.movement).filter((m: any) => m?.previous.card === 0);
    const zones = moves.map((m: any) => m.destination.zone);
    if (taxed) assert.deepEqual(zones, ["stack", "battlefield", "command", "stack", "stack", "battlefield"]);
    if (run.policy === "hand_recast") assert.deepEqual(zones, ["stack", "battlefield", "hand", "stack", "battlefield"]);
    if (run.policy === "grave_return") assert.deepEqual(zones, ["graveyard", "command", "stack", "battlefield"]);
    if (run.policy === "grave_decline") assert.deepEqual(zones, ["graveyard"]);
    if (run.policy === "exile_return") assert.deepEqual(zones, ["exile", "command", "stack", "battlefield"]);
    if (run.policy === "exile_decline") assert.deepEqual(zones, ["exile"]);
    const choices = run.journal.decisions.filter((d: any) => d.response.response === "commander_return");
    assert.equal(choices.length, 1);
    assert.equal(choices[0].player, 0);
    for (const observation of run.final_observations) {
      assert.equal(observation.observation_version, 13);
      assert.equal(observation.view.objects.filter((o: any) => o.zone.zone === "hand").every((o: any) => o.owner === observation.view.viewer), true);
      for (const key of ["incarnation", "frame", "commander_returns", "total_cost", "continuation", "rule_steps"]) {
        assert.equal(JSON.stringify(observation).includes(`"${key}"`), false, key);
      }
      assert.equal(observation.view.commanders.length, 1);
    }
  }
});

test("Commander damage and library witnesses expose actual results without hidden-library handles", () => {
  const runs = demo().damage_library;
  assert.deepEqual(runs, currentProjectionVersions(JSON.parse(read("expected-damage-library-scenario.json"))));
  assert.deepEqual(runs.map((r: any) => r.policy), [
    "commander_damage", "borrowed_return", "borrowed_decline", "tuck_return", "draw_return", "draw_hand_cast", "ordinary_tuck",
  ]);
  for (const run of runs.slice(0, 3)) {
    assert.equal(run.first_hit.view.players[2].life, "22");
    assert.equal(run.first_hit.view.players[2].status, "live");
    assert.equal(run.first_hit.view.commanders[0].history.combat_damage[2], "18");
    for (const observation of run.final_observations) {
      assert.equal(observation.view.players[2].life, "4");
      assert.equal(observation.view.players[2].status, "left");
      assert.equal(observation.view.commanders[0].history.combat_damage[2], "36");
    }
    assert.deepEqual(run.events.filter((e: any) => e.event === "players_lost").map((e: any) => e.losses), [[[2, "commander_damage"]]]);
    const changes = run.events.flatMap((e: any) => e.actual?.commander_damage ?? []);
    assert.deepEqual(changes.map((c: any) => [c.commander, c.recipient, c.amount, c.previous, c.current]),
      [[0, 2, "18", "0", "18"], [0, 2, "18", "18", "36"]]);
    assert.equal(run.journal.concessions.length, run.policy === "commander_damage" ? 0 : 1);
  }
  for (const run of runs.slice(3)) {
    const payments = run.events.filter((e: any) => e.actual?.outcome === "mana_paid").map((e: any) => e.actual.cost);
    assert.deepEqual(payments[0], { generic: "2", colored: ["U"] });
    if (run.policy !== "ordinary_tuck") assert.deepEqual(payments[1], { generic: "0", colored: ["W"] });
    const topMoves = run.events.filter((e: any) => e.actual?.outcome === "library_top_moved");
    assert.equal(topMoves.length, run.policy === "tuck_return" ? 0 : 1);
    if (topMoves.length) assert.equal(topMoves[0].actual.previous_top, run.policy === "ordinary_tuck" ? 12 : 10);
    for (const choice of run.choices) {
      assert.equal(choice.view.viewer, 1);
      assert.equal(choice.view.priority, null);
      assert.equal(choice.decision.designation, choice.view.commanders[0].designation);
    }
    if (run.policy === "draw_return" || run.policy === "draw_hand_cast") {
      const draw = run.choices[1];
      assert.equal(run.choices.length, 2);
      assert.equal(typeof run.choices[0].decision.object, "string");
      assert.equal(draw.decision.object, null);
      assert.equal(draw.view.turn.step, "draw");
      assert.equal(draw.view.players[1].library_count, 3);
      assert.equal(run.final_observations[1].view.players[1].library_count, 2);
    }
  }
  for (const run of runs) {
    for (const observation of [...run.final_observations, ...run.choices]) {
      assert.equal(observation.observation_version, 13);
      assert.equal(observation.view.objects.some((o: any) => o.zone.zone === "library"), false);
      assert.equal(observation.view.objects.filter((o: any) => o.zone.zone === "hand").every((o: any) => o.owner === observation.view.viewer), true);
      for (const key of ["incarnation", "frame", "commander_returns", "libraries", "continuation", "rule_steps", "previous_top"]) {
        assert.equal(JSON.stringify(observation).includes(`"${key}"`), false, key);
      }
    }
  }
});
