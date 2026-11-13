import assert from "node:assert/strict";

// Synthetic IR shape probes; exact Burst source lowering is checked in Rust.
export function assertUntapSchema(validate: (value: any) => unknown, catalog: any) {
  const base = structuredClone(catalog);
  const source = base.definitions.find((d: any) => d.name === "Naturalize");
  assert.ok(source);
  base.definitions = [source];
  source.spell.targets[0].filter = { types: [], non_types: [], non_colors: [], opponent: false };
  source.spell.instructions = [{ op: "untap_permanent", target_slot: 0, source_block: 0 }];
  source.capabilities.push("untap_permanent_v1");
  assert.equal(validate(base), true, "complete ordinary untap shape");
  const changes: ((d: any) => void)[] = [
    d => { d.capabilities = d.capabilities.filter((c: string) => c !== "untap_permanent_v1"); },
    d => { d.spell.instructions[0].target_slot = 1; },
    d => { d.spell.instructions.push(structuredClone(d.spell.instructions[0])); },
    d => { d.spell.targets[0].filter.types = ["creature"]; },
    d => { d.spell.targets[0].filter.tapped = true; },
    d => { d.spell.targets[0].filter.opponent = true; },
    d => { d.spell.targets[0].filter.non_colors = ["B"]; },
    d => { delete d.spell.targets[0].filter; },
    d => { d.spell.targets[0].domain = "battlefield_creature"; },
    d => { d.spell.functioning_zone = "battlefield"; },
    d => {
      const child = structuredClone(d.spell);
      d.spell.targets = []; d.spell.instructions = [];
      d.spell.modes = [{ program: child }, { program: child }];
    },
  ];
  for (const [index, change] of changes.entries()) {
    const bad = structuredClone(base); change(bad.definitions[0]);
    assert.equal(validate(bad), false, `untap restriction ${index}`);
  }
  for (const [name, field] of [["Viridian Zealot", "activated"], ["Reclamation Sage", "triggered"]] as const) {
    const bad = structuredClone(catalog);
    const d = bad.definitions.find((item: any) => item.name === name);
    assert.ok(d);
    d[field][0].program = structuredClone(source.spell);
    d.capabilities.push("untap_permanent_v1");
    bad.definitions = [d];
    assert.equal(validate(bad), false, `${field} untap`);
  }
  return { positive: 1, negative: changes.length + 2 };
}
