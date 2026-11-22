// Consumes create-new compiler-test exports; never launches native work itself.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import Ajv2020 from "ajv/dist/2020.js";

test("typed_fetchland exported schema closes cost and complete activation shape", () => {
  const directory = process.env.MTG_TYPED_FETCHLAND_EXPORT_DIR;
  assert.ok(directory, "export with typed_fetchland_complete_frozen_sources_and_exact_cost_export first");
  const read = (name: string) => JSON.parse(fs.readFileSync(path.join(directory, name), "utf8"));
  const catalog = read("catalog.json");
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(read("schema.json"));
  assert.ok(validate(catalog), JSON.stringify(validate.errors));
  for (const life of ["0", "2", "18446744073709551615", "999999999999999999999999999999999999999999999"]) {
    const next = structuredClone(catalog);
    next.definitions[0].activated[0].cost.tap_pay_life_sacrifice_source.life = life;
    assert.ok(validate(next), JSON.stringify(validate.errors));
  }
  const rejects = [
    (d: any) => { d.activated[0].cost.tap_pay_life_sacrifice_source.life = "-1"; },
    (d: any) => { d.activated[0].cost.tap_pay_life_sacrifice_source.life = null; },
    (d: any) => { d.activated[0].cost.tap_pay_life_sacrifice_source.life = 1; },
    (d: any) => { d.activated[0].cost.tap_pay_life_sacrifice_source.life = "01"; },
    (d: any) => { delete d.activated[0].cost.tap_pay_life_sacrifice_source.life; },
    (d: any) => { d.activated[0].cost.tap_pay_life_sacrifice_source.payer = "owner"; },
    (d: any) => { d.activated[0].cost = "tap_source"; },
    (d: any) => { d.activated[0].program.instructions.push({ op: "kernel", node: { do: { verb: "draw", player: { player: "is", who: { ref: "controller", role: "ability_controller" } }, amount: { amount: "fixed", value: "1" } } }, source_block: 0 }); },
    (d: any) => { d.activated[0].program.optional_additional_costs = [{ identity: "foreign", source_block: 0, mana: { generic: "1", colored: [] } }]; },
    (d: any) => { d.activated[0].program.instructions.push({ op: "draw_cards_if_additional_cost", count: "1", cost_identity: "foreign", source_block: 0 }); },
    (d: any) => { d.activated[0].program.timing = "main_phase"; },
    (d: any) => { d.activated[0].program.instructions[0].source_block = 1; },
    (d: any) => { d.activated.push(structuredClone(d.activated[0])); },
    (d: any) => { d.characteristics.kind = "creature"; },
    (d: any) => { d.characteristics.subtypes = ["Forest"]; },
    (d: any) => { d.characteristics.basic = true; },
    (d: any) => { d.static_abilities = [{ functioning_zone: "battlefield", effect: "enters_tapped" }]; },
    (d: any) => { d.capabilities = d.capabilities.filter((c: string) => c !== "pay_life_cost_v1"); },
    (d: any) => { d.capabilities.push("life_loss_v1"); },
    (d: any) => { d.activated = []; },
  ];
  for (const [index, mutate] of rejects.entries()) {
    const next = structuredClone(catalog);
    mutate(next.definitions[0]);
    assert.equal(validate(next), false, `schema mutation ${index}`);
  }
});
