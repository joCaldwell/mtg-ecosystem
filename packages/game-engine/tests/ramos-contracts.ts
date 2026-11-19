import assert from "node:assert/strict";

/** External JSON Schema checks on complete production-compiled source. */
export function assertRamosSchema(validate: any, catalog: any, ordinary: any): number {
  let probes = 0;
  const check = (label: string, value: any, expected: boolean) => {
    probes++;
    assert.equal(validate(value), expected, `${label}: ${JSON.stringify(validate.errors)}`);
  };
  check("complete frozen Ramos", catalog, true);
  assert.equal(catalog.definitions.length, 1);
  const d = catalog.definitions[0];
  assert.equal(d.oracle_id, "3ed41d2d-211b-4013-8562-8c64d54cc43a");
  assert.equal(d.mana_abilities[0].costs.remove_source_plus_one_counters, "5");
  assert.deepEqual(d.mana_abilities[0].production.symbols, ["W", "W", "U", "U", "B", "B", "R", "R", "G", "G"]);
  for (const [label, mutate] of [
    ["missing cast-color capability", (x: any) => { x.capabilities = x.capabilities.filter((c: string) => c !== "cast_spell_color_counters_v1"); }],
    ["opponent cast", (x: any) => { x.triggered[0].condition.spell_cast.caster = "opponent"; }],
    ["qualified spell", (x: any) => { x.triggered[0].condition.spell_cast.spell = { predicate: "type", kind: "creature" }; }],
    ["missing counter-mana capability", (x: any) => { x.capabilities = x.capabilities.filter((c: string) => c !== "counter_removal_mana_v1"); }],
    ["zero cost", (x: any) => { x.mana_abilities[0].costs.remove_source_plus_one_counters = "0"; }],
    ["negative cost", (x: any) => { x.mana_abilities[0].costs.remove_source_plus_one_counters = "-1"; }],
    ["null cost", (x: any) => { x.mana_abilities[0].costs.remove_source_plus_one_counters = null; }],
    ["missing cost", (x: any) => { delete x.mana_abilities[0].costs.remove_source_plus_one_counters; }],
    ["missing restriction", (x: any) => { delete x.mana_abilities[0].restriction; }],
    ["null restriction", (x: any) => { x.mana_abilities[0].restriction = null; }],
    ["tap combination", (x: any) => { x.mana_abilities[0].costs.tap = true; }],
    ["extra mana", (x: any) => { x.mana_abilities[0].costs.mana = { generic: "1", colored: [] }; }],
    ["sacrifice combination", (x: any) => { x.mana_abilities[0].costs.sacrifice = true; }],
    ["empty production", (x: any) => { x.mana_abilities[0].production.symbols = []; }],
  ] as const) {
    const bad = structuredClone(catalog); mutate(bad.definitions[0]); check(label, bad, false);
  }
  for (const [cost, symbols] of [["1", ["C"]], ["3", ["R", "G", "G"]], ["7", ["U", "U", "U", "U"]]] as const) {
    const variant = structuredClone(catalog);
    variant.definitions[0].mana_abilities[0].costs.remove_source_plus_one_counters = cost;
    variant.definitions[0].mana_abilities[0].production.symbols = symbols;
    check(`parameterized family ${cost}`, variant, true);
  }
  check("ordinary mana catalog", ordinary, true);
  for (const field of ["restriction", "remove_source_plus_one_counters", "both"] as const) {
    const nullable = structuredClone(ordinary);
    for (const source of nullable.definitions) for (const a of source.mana_abilities ?? []) {
      if (field !== "remove_source_plus_one_counters") a.restriction = null;
      if (field !== "restriction") a.costs.remove_source_plus_one_counters = null;
    }
    check(`ordinary optional None ${field}`, nullable, true);
  }
  return probes;
}
