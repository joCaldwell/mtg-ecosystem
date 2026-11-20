// UNRUN source checkpoint attachment: complete Imoti plus synthetic IR probes.
// No Cargo, archive rehash, parser import or installation. Use existing Ajv.
// --schema FILE --catalog FILE --ajv-root NODE_MODULES_DIRECTORY
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
const args = process.argv.slice(2);
const option = key => { const i = args.indexOf(key); assert.ok(i >= 0 && args[i + 1], key); return resolve(args[i + 1]); };
const require = createRequire(join(option('--ajv-root'), 'ajv/package.json'));
const Ajv = require('./dist/2020.js').default;
const schemaBytes = readFileSync(option('--schema'));
const catalogBytes = readFileSync(option('--catalog'));
const catalog = JSON.parse(catalogBytes);
assert.equal(catalog.schema_version, 1);
assert.equal(catalog.definitions.length, 1);
const d = catalog.definitions[0];
assert.equal(d.oracle_id, 'eca32dcd-6845-433e-a631-ed1f0ee78f25');
assert.equal(d.triggered.length, 1);
assert.equal(d.static_abilities.length, 1);
const validate = new Ajv({ strict: false, formats: { uint32: { type: 'number', validate: n => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff } } }).compile(JSON.parse(schemaBytes));
const report = { ajv: require('./package.json').version, schema_sha256: createHash('sha256').update(schemaBytes).digest('hex'), catalog_sha256: createHash('sha256').update(catalogBytes).digest('hex'), positives: 0, negatives: 0, failures: [] };
function check(name, value, expected) {
  if (validate(value) !== expected) report.failures.push({ name, expected, errors: structuredClone(validate.errors) });
  else report[expected ? 'positives' : 'negatives']++;
}
check('complete production Imoti', catalog, true);
for (const minimum of ['0', '2', '9', '999999999999999999999999999999999999999999999']) {
  for (const count of [1, 2, 17, 257]) {
    const x = structuredClone(catalog), card = x.definitions[0];
    card.triggered = []; card.capabilities = card.capabilities.filter(c => c !== 'triggered_ability_v1');
    card.static_abilities = Array.from({ length: count }, () => structuredClone(d.static_abilities[0]));
    for (const a of card.static_abilities) a.effect.grant_spell_cascade.eligibility.minimum = minimum;
    check(`synthetic grantor with no intrinsic, min ${minimum}, count ${count}`, x, true);
  }
}
for (const [name, change] of [
  ['negative', a => { a.effect.grant_spell_cascade.eligibility.minimum = '-1'; }],
  ['noncanonical', a => { a.effect.grant_spell_cascade.eligibility.minimum = '06'; }],
  ['numeric', a => { a.effect.grant_spell_cascade.eligibility.minimum = 6; }],
  ['null', a => { a.effect.grant_spell_cascade.eligibility.minimum = null; }],
  ['missing', a => { delete a.effect.grant_spell_cascade.eligibility.minimum; }],
  ['extra property', a => { a.effect.grant_spell_cascade.first = true; }],
  ['zone', a => { a.functioning_zone = 'stack'; }],
  ['missing program', a => { delete a.effect.grant_spell_cascade.program; }],
  ['timing', a => { a.effect.grant_spell_cascade.program.timing = 'main_phase'; }],
  ['program zone', a => { a.effect.grant_spell_cascade.program.functioning_zone = 'battlefield'; }],
  ['resolution', a => { a.effect.grant_spell_cascade.program.resolution = 'permanent'; }],
  ['empty', a => { a.effect.grant_spell_cascade.program.instructions = []; }],
  ['duplicate cascade', a => { const p = a.effect.grant_spell_cascade.program; p.instructions.push(structuredClone(p.instructions[0])); }],
  ['wrong threshold', a => { a.effect.grant_spell_cascade.program.instructions[0].threshold = 'paid_mana'; }],
  ['extra instruction property', a => { a.effect.grant_spell_cascade.program.instructions[0].grant_threshold = '6'; }],
  ['optional cost', a => { a.effect.grant_spell_cascade.program.optional_additional_costs = [{ identity: 'synthetic', source_block: 0, mana: { generic: '0', colored: [] } }]; }],
  ['target', a => { a.effect.grant_spell_cascade.program.targets = [{ slot: 0, selection_id: 'synthetic', chooser: 'controller', stage: 'announcement', filter: null, domain: 'battlefield_creature' }]; }],
  ['mode', a => { const p = a.effect.grant_spell_cascade.program; p.modes = [{ program: structuredClone(p) }]; }],
]) {
  const x = structuredClone(catalog); change(x.definitions[0].static_abilities[0]); check(name, x, false);
}
for (const cap of ['static_spell_cascade_grant_v1', 'cascade_v1', 'resolution_cast_v1', 'random_bottom_v1']) {
  const x = structuredClone(catalog); x.definitions[0].capabilities = d.capabilities.filter(c => c !== cap); check(`missing ${cap}`, x, false);
}
for (const predicate of ['first_cast_each_turn','first_cast_from_exile_each_turn']) {
  const x = structuredClone(catalog), card = x.definitions[0];
  card.static_abilities[0].effect.grant_spell_cascade.eligibility = { eligibility: predicate };
  card.capabilities.push('cast_history_v1');
  check(predicate, x, true);
  for (const cap of ['cast_history_v1','static_spell_cascade_grant_v1','cascade_v1','resolution_cast_v1','random_bottom_v1']) {
    const bad = structuredClone(x); bad.definitions[0].capabilities = card.capabilities.filter(c => c !== cap);
    check(`${predicate} missing ${cap}`,bad,false);
  }
  for (const descriptor of [null,{}, {eligibility:predicate,minimum:'1'}, {eligibility:predicate,from:'exile'}, {eligibility:'first_cast'}, {eligibility:'mana_value_at_least'}, {eligibility:'mana_value_at_least',minimum:'-1'}]) {
    const bad = structuredClone(x); bad.definitions[0].static_abilities[0].effect.grant_spell_cascade.eligibility = descriptor;
    check(`${predicate} malformed ${JSON.stringify(descriptor)}`,bad,false);
  }
}
const spare = structuredClone(catalog); spare.definitions[0].capabilities.push('cast_history_v1'); check('spare history capability',spare,false);
const orphan = structuredClone(catalog); orphan.definitions[0].static_abilities = [];
check('orphan static grant capability', orphan, false);
// Legacy unit-valued statics must not satisfy the history-object predicate.
const ordinary = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-mana-ir.json', import.meta.url), 'utf8'));
check('ordinary complete mana catalog', ordinary, true);
for (const card of ordinary.definitions.filter(d => (d.static_abilities ?? []).some(a => typeof a.effect === 'string'))) {
  const value = structuredClone(ordinary); value.definitions = [structuredClone(card)];
  check('legacy unit static ' + card.name, value, true);
  value.definitions[0].capabilities.push('cast_history_v1');
  check('unearned history capability on ' + card.name, value, false);
}
report.probes = report.positives + report.negatives + report.failures.length;
report.status = report.failures.length ? 'failed' : 'passed';
console.log(JSON.stringify(report, null, 2));
assert.equal(report.failures.length, 0);
