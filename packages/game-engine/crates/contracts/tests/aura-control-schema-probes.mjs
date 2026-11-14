// Synthetic typed IR schema controls, never parser-produced card evidence.
// --prepare-only constructs/freeze-checks probes without executing the schema.
// Otherwise: --schema ABSOLUTE_SCHEMA [--ajv-root NODE_MODULES_DIRECTORY].
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';

const legacy = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-attachments-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);
const capabilities = ['normal_layout_v1', 'cast_enchantment_v1', 'creature_target_v1', 'attachments_v1', 'aura_control_v1'];
function control(generic) {
  const d = clone(legacy.definitions.find(d => d.name === 'Pacifism'));
  d.name = `Synthetic control Aura ${generic}`;
  d.oracle_id = `synthetic-control-${generic}`;
  d.characteristics.mana_cost = { generic: String(generic), colored: ['U', 'U'] };
  d.characteristics.colors = ['U'];
  d.characteristics.color_identity = ['U'];
  d.static_abilities = [{ functioning_zone: 'battlefield', effect: 'control_enchanted_creature' }];
  d.capabilities = clone(capabilities);
  return { ...clone(legacy), definitions: [d] };
}
const probes = [];
const positives = [];
function positive(name, value) {
  positives.push({ name, value, exact: JSON.stringify(value) });
  probes.push({ name, value, expected: true });
}
positive('legacy attachments unchanged', clone(legacy));
for (const generic of ['0', '2', '3', '99999999999999999999999999']) positive(`reusable control ${generic}`, control(generic));
const explicit = control(2);
Object.assign(explicit.definitions[0], { x_mana_cost: false, activated: [], triggered: [], keywords: [], mana_abilities: [] });
Object.assign(explicit.definitions[0].characteristics, { additional_types: [], snow: false });
Object.assign(explicit.definitions[0].spell, { additional_sacrifice: null, modes: [] });
explicit.definitions[0].spell.targets[0].filter = null;
positive('explicit empty/default fields', explicit);
const reordered = control(3);
reordered.definitions[0].capabilities.reverse();
positive('capability set order independent', reordered);

function negative(name, mutate) {
  const value = control(3);
  mutate(value.definitions[0], value);
  probes.push({ name, value, expected: false });
}
for (const capability of capabilities) negative(`missing ${capability}`, d => { d.capabilities = d.capabilities.filter(c => c !== capability); });
for (const [name, mutate] of [
  ['spare capability', d => d.capabilities.push('flash_v1')],
  ['duplicate capability', d => { d.capabilities[0] = 'aura_control_v1'; }],
  ['absent capability set', d => { delete d.capabilities; }],
  ['absent control operation', d => { delete d.static_abilities; }],
  ['no control operation', d => { d.static_abilities = []; }],
  ['legacy operation with control capability', d => { d.static_abilities = clone(legacy.definitions.find(d => d.name === 'Pacifism').static_abilities); }],
  ['effect payload', d => { d.static_abilities[0].effect = { control_enchanted_creature: { duration: 'until_end_of_turn' } }; }],
  ['stack static', d => { d.static_abilities[0].functioning_zone = 'stack'; }],
  ['extra static', d => d.static_abilities.push({ functioning_zone: 'battlefield', effect: 'enters_tapped' })],
  ['wrong kind', d => { d.characteristics.kind = 'artifact'; }],
  ['extra type', d => { d.characteristics.additional_types = ['artifact']; }],
  ['no Aura subtype', d => { d.characteristics.subtypes = []; }],
  ['extra subtype', d => d.characteristics.subtypes.push('Role')],
  ['basic', d => { d.characteristics.basic = true; }],
  ['legendary', d => { d.characteristics.legendary = true; }],
  ['snow', d => { d.characteristics.snow = true; }],
  ['power', d => { d.characteristics.power = '1'; }],
  ['toughness', d => { d.characteristics.toughness = '1'; }],
  ['negative mana', d => { d.characteristics.mana_cost.generic = '-1'; }],
  ['variable mana', d => { d.x_mana_cost = true; }],
  ['intrinsic zone', d => { d.intrinsic_zone = 'battlefield'; }],
  ['keyword', d => { d.keywords = ['flash']; }],
  ['activation', d => { d.activated = [{ functioning_zone: 'battlefield', cost: 'tap_source', program: clone(d.spell) }]; }],
  ['mana ability', d => { d.mana_abilities = [{ costs: { tap: true, sacrifice: false }, production: { mana: 'fixed', symbols: ['U'] }, source_block: 2 }]; }],
  ['trigger', d => { d.triggered = clone(legacy.definitions.find(d => d.name === 'Rancor').triggered); }],
  ['no spell', d => { d.spell = null; }],
  ['instant timing', d => { d.spell.timing = 'instant'; }],
  ['instructions resolution', d => { d.spell.resolution = 'instructions'; }],
  ['battlefield spell', d => { d.spell.functioning_zone = 'battlefield'; }],
  ['additional sacrifice', d => { d.spell.additional_sacrifice = { types: ['creature'], subtypes: [] }; }],
  ['instruction', d => d.spell.instructions.push({ op: 'draw_cards', count: '1', source_block: 2 })],
  ['mode', d => { d.spell.modes = [{ program: clone(d.spell) }]; }],
  ['no target', d => { d.spell.targets = []; }],
  ['extra target', d => d.spell.targets.push(clone(d.spell.targets[0]))],
  ['target slot', d => { d.spell.targets[0].slot = 1; }],
  ['empty selection', d => { d.spell.targets[0].selection_id = ''; }],
  ['controlled target', d => { d.spell.targets[0].domain = 'controlled_battlefield_creature'; }],
  ['permanent target', d => { d.spell.targets[0].domain = 'battlefield_permanent'; }],
  ['target filter', d => { d.spell.targets[0].filter = { types: ['creature'], non_types: [], non_colors: [], opponent: false }; }],
  ['effect qualifier', d => { d.static_abilities[0].duration = 'until_end_of_turn'; }],
]) negative(name, mutate);
negative('schema version remains one', (_d, catalog) => { catalog.schema_version = 2; });
for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact, `mutated positive ${row.name}`);
const summary = {
  status: 'prepared_only', probes: probes.length,
  positive: probes.filter(p => p.expected).length,
  negative: probes.filter(p => !p.expected).length,
  controls_sha256: createHash('sha256').update(JSON.stringify(positives)).digest('hex'),
};
if (process.argv.includes('--prepare-only')) {
  console.log(JSON.stringify(summary));
} else {
  function option(name) { const i = process.argv.indexOf(name); return i < 0 ? undefined : process.argv[i + 1]; }
  const schemaPath = option('--schema');
  assert(schemaPath, '--schema PATH is required (or use --prepare-only)');
  const root = option('--ajv-root');
  const require = createRequire(root ? join(resolve(root), '__schema_probe__.cjs') : import.meta.url);
  const Ajv2020 = require('ajv/dist/2020.js').default;
  const bytes = readFileSync(schemaPath);
  const ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addFormat('uint32', { type: 'number', validate: n => Number.isInteger(n) && n >= 0 && n <= 4294967295 });
  const validate = ajv.compile(JSON.parse(bytes));
  const failures = [];
  for (const probe of probes) {
    const actual = validate(clone(probe.value));
    if (actual !== probe.expected) failures.push({ name: probe.name, expected: probe.expected, actual, errors: clone(validate.errors) });
  }
  for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact, `mutated positive ${row.name}`);
  console.log(JSON.stringify({ ...summary, status: failures.length ? 'failed' : 'passed', schema_sha256: createHash('sha256').update(bytes).digest('hex'), ajv_version: require('ajv/package.json').version, failures }, null, 2));
  if (failures.length) process.exitCode = 1;
}
