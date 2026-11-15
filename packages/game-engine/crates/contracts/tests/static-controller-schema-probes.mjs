// Production-compiled complete Wanderer plus explicitly synthetic IR shape probes.
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
assert.equal(d.oracle_id, 'ad9b7fbc-61c8-43ee-a65c-99206fd1e4df');
assert.equal(d.triggered.length, 2);
const validate = new Ajv({ strict: false, formats: { uint32: { type: 'number', validate: n => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff } } }).compile(JSON.parse(schemaBytes));
const report = { ajv: require('./package.json').version, schema_sha256: createHash('sha256').update(schemaBytes).digest('hex'), catalog_sha256: createHash('sha256').update(catalogBytes).digest('hex'), positives: 0, negatives: 0, failures: [] };
function check(name, value, expected) {
  if (validate(value) !== expected) report.failures.push({ name, expected, errors: structuredClone(validate.errors) });
  else report[expected ? 'positives' : 'negatives']++;
}
check('complete production Wanderer', catalog, true);
check('existing enchantment and attachment family', JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-attachments-ir.json', import.meta.url))), true);
for (const count of [1, 2, 5, 17, 257]) {
  const x = structuredClone(catalog);
  x.definitions[0].triggered = Array.from({ length: count }, () => structuredClone(d.triggered[0]));
  check(`synthetic generic repeated vector ${count}`, x, true);
}
for (let index = 0; index < 2; index++) {
  for (const [name, change] of [
    ['wrong functioning zone', a => { a.functioning_zone = 'battlefield'; }],
    ['wrong program zone', a => { a.program.functioning_zone = 'battlefield'; }],
    ['wrong timing', a => { a.program.timing = 'main_phase'; }],
    ['empty program', a => { a.program.instructions = []; }],
    ['mixed program', a => { a.program.instructions.push({ op: 'draw_cards', recipient: null, count: '1', source_block: 0 }); }],
    ['duplicate opcode', a => { a.program.instructions.push(structuredClone(a.program.instructions[0])); }],
    ['wrong threshold', a => { a.program.instructions[0].threshold = 'paid_mana'; }],
    ['extra threshold field', a => { a.program.instructions[0].paid_threshold = '0'; }],
    ['optional cost', a => { a.program.optional_additional_costs = [{ identity: 'synthetic', source_block: 0, mana: { generic: '2', colored: ['U'] } }]; }],
    ['target', a => { a.program.targets = [{ slot: 0, selection_id: 'synthetic', chooser: 'controller', stage: 'announcement', filter: null, domain: 'battlefield_creature' }]; }],
    ['mode', a => { a.program.modes = [{ program: structuredClone(a.program) }]; }],
    ['ordinary battlefield observer', a => { a.functioning_zone = 'battlefield'; a.condition = 'self_enters_battlefield'; }],
  ]) {
    const x = structuredClone(catalog); change(x.definitions[0].triggered[index]); check(`instance ${index}: ${name}`, x, false);
  }
}
for (const cap of ['cascade_v1', 'resolution_cast_v1', 'random_bottom_v1']) {
  const x = structuredClone(catalog); x.definitions[0].capabilities = d.capabilities.filter(c => c !== cap); check(`missing ${cap}`, x, false);
}
const orphan = structuredClone(catalog);
orphan.definitions[0].spell.instructions.push(structuredClone(d.triggered[0].program.instructions[0]));
check('orphan spell cascade', orphan, false);
// Controller source schema closure; all mutations are synthetic IR controls.
for (const [name, change] of [
  ['artifact source', d => { d.characteristics.kind = 'artifact'; }],
  ['instant source', d => { d.characteristics.kind = 'instant'; }],
  ['sorcery source', d => { d.characteristics.kind = 'sorcery'; }],
  ['land source', d => { d.characteristics.kind = 'land'; }],
  ['Aura source', d => { d.characteristics.subtypes = ['Aura']; }],
  ['enchanted scope on creature', d => { d.static_abilities[0].effect.modify_creatures.scope = 'enchanted'; }],
  ['equipped scope on creature', d => { d.static_abilities[0].effect.modify_creatures.scope = 'equipped'; }],
  ['source Haste scope', d => { d.static_abilities[0].effect.modify_creatures.scope = 'source'; }],
  ['attacking Haste scope', d => { d.static_abilities[0].effect.modify_creatures.scope = 'attacking_controller'; }],
  ['stack static', d => { d.static_abilities[0].functioning_zone = 'stack'; }],
  ['negative power', d => { d.static_abilities[0].effect.modify_creatures.power = '-1'; }],
  ['negative toughness', d => { d.static_abilities[0].effect.modify_creatures.toughness = '-1'; }],
  ['duplicate keyword', d => { d.static_abilities[0].effect.modify_creatures.keywords = ['haste', 'haste']; }],
  ['duplicate restriction', d => { d.static_abilities[0].effect.modify_creatures.restrictions = ['cant_block', 'cant_block']; }],
  ['Flash', d => { d.static_abilities[0].effect.modify_creatures.keywords = ['flash']; }],
  ['Indestructible', d => { d.static_abilities[0].effect.modify_creatures.keywords = ['indestructible']; }],
  ['empty modifier', d => { d.static_abilities[0].effect.modify_creatures.keywords = []; }],
  ['extra modifier operand', d => { d.static_abilities[0].effect.modify_creatures.filter = {}; }],
  ['missing attachments capability', d => { d.capabilities = d.capabilities.filter(c => c !== 'attachments_v1'); }],
]) {
  const x = structuredClone(catalog); change(x.definitions[0]); check(name, x, false);
}
for (const [power, keywords] of [['7', ['flying', 'trample']], ['3', []], ['0', ['reach', 'lifelink']]]) {
  const x = structuredClone(catalog);
  Object.assign(x.definitions[0].static_abilities[0].effect.modify_creatures, { power, keywords });
  check('synthetic reusable controller modifier', x, true);
}
for (const count of [1, 2, 5, 257]) {
  const x = structuredClone(catalog);
  x.definitions[0].static_abilities = Array.from({ length: count }, () => structuredClone(d.static_abilities[0]));
  check(`synthetic static vector ${count}`, x, true);
}
report.probes = report.positives + report.negatives + report.failures.length;
report.status = report.failures.length ? 'failed' : 'passed';
console.log(JSON.stringify(report, null, 2));
assert.equal(report.failures.length, 0);
