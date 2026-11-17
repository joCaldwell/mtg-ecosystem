// Synthetic contextual IR controls; --prepare-only constructs without schema or native execution.
// Optional --catalog is the five complete-source export, never generated here.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
const clone = x => structuredClone(x);
const frozen = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-ir.json', import.meta.url)));
const effects = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-effects-ir.json', import.meta.url)));
const option = name => { const i = process.argv.indexOf(name); return i < 0 ? undefined : resolve(process.argv[i + 1]); };
const cap = 'optional_life_land_entry_v1';
const d = clone(frozen.definitions.find(d => d.characteristics.basic));
delete d.source_labels;
d.characteristics.basic = false;
d.characteristics.subtypes = ['Forest', 'Island'];
d.characteristics.additional_types = [];
d.static_abilities = [{ functioning_zone: 'battlefield', effect: { enters_tapped_unless_pay_life: { amount: '2' } } }];
d.mana_abilities = []; d.activated = []; d.triggered = []; d.keywords = []; d.x_mana_cost = false;
d.capabilities = ['normal_layout_v1', 'land_play_v1', 'typed_nonbasic_intrinsic_mana_v1', cap];
const base = { ...clone(frozen), definitions: [d] };
const probes = [];
const positive = (name, value) => probes.push({ name, value: clone(value), expected: true });
const negative = (name, change, input = base) => { const value = clone(input); change(value.definitions[0]); probes.push({ name, value, expected: false }); };
for (const amount of ['0', '1', '2', '3', '999999999999999999999999999999999999999999999999']) {
 const x = clone(base); x.definitions[0].static_abilities[0].effect.enters_tapped_unless_pay_life.amount = amount;
 positive(`exact:${amount}`, x);
}
for (const [name, change] of [
 ['negative', d => d.static_abilities[0].effect.enters_tapped_unless_pay_life.amount = '-1'],
 ['number', d => d.static_abilities[0].effect.enters_tapped_unless_pay_life.amount = 2],
 ['null', d => d.static_abilities[0].effect.enters_tapped_unless_pay_life.amount = null],
 ['leading-zero', d => d.static_abilities[0].effect.enters_tapped_unless_pay_life.amount = '02'],
 ['missing', d => delete d.static_abilities[0].effect.enters_tapped_unless_pay_life.amount],
 ['wrong-payer', d => d.static_abilities[0].effect.enters_tapped_unless_pay_life.payer = 'owner'],
 ['null-effect', d => d.static_abilities[0].effect.enters_tapped_unless_pay_life = null],
 ['extra-effect', d => d.static_abilities[0].effect.extra = true],
 ['double', d => d.static_abilities.push(clone(d.static_abilities[0]))],
 ['forced-composition', d => { d.static_abilities.push({ functioning_zone: 'battlefield', effect: 'enters_tapped' }); d.capabilities.push('enters_tapped_v1'); }],
 ['conditional-composition', d => { d.static_abilities.push({ functioning_zone: 'battlefield', effect: { enters_tapped_unless_controller_basic_lands: { minimum: '2' } } }); d.capabilities.push('conditional_basic_land_entry_v1'); }],
 ['zone', d => d.static_abilities[0].functioning_zone = 'stack'],
 ['kind', d => d.characteristics.kind = 'artifact'], ['basic', d => d.characteristics.basic = true],
 ['empty-types', d => d.characteristics.subtypes = []], ['duplicate-types', d => d.characteristics.subtypes = ['Forest', 'Forest']],
 ['unknown-type', d => d.characteristics.subtypes = ['Desert']], ['additional-type', d => d.characteristics.additional_types = ['artifact']],
 ['power', d => d.characteristics.power = '1'], ['mana-cost', d => d.characteristics.mana_cost.generic = '1'],
 ['explicit-mana', d => d.mana_abilities = [{}]], ['activation', d => d.activated = [{}]], ['trigger', d => d.triggered = [{}]],
 ['keyword', d => d.keywords = ['flash']], ['spell', d => d.spell = clone(effects.definitions.find(d => d.name === 'Giant Growth').spell)],
]) negative(name, change);
for (const c of d.capabilities) negative(`missing-cap:${c}`, d => d.capabilities = d.capabilities.filter(x => x !== c));
for (const c of ['basic_land_mana_v1', 'enters_tapped_v1', 'conditional_basic_land_entry_v1', 'optional_additional_mana_cost_v1', 'life_gain_v1']) negative(`spare-cap:${c}`, d => d.capabilities.push(c));
// The new static cannot carry either a casting declaration or an undeclared linked draw.
const naturalize = effects.definitions.find(d => d.name === 'Giant Growth');
const conditional = { op: 'draw_cards_if_additional_cost', cost_identity: 'undeclared', count: '1', source_block: 0 };
for (const costs of [false, true]) {
 const program = clone(naturalize.spell); program.instructions = [clone(conditional)];
 if (costs) program.optional_additional_costs = [{ identity: 'undeclared', source_block: 0, mana: { generic: '0', colored: [] } }];
 negative(`land:cost-draw:${costs}`, d => d.spell = clone(program));
 for (const host of effects.definitions.filter(d => d.activated?.length || d.triggered?.length)) {
  const input = { ...clone(effects), definitions: [clone(host)] }; delete input.definitions[0].source_labels;
  negative(`ability:${host.name}:cost-draw:${costs}`, d => {
   if (d.activated?.length) d.activated[0].program = clone(program);
   if (d.triggered?.length) d.triggered[0].program = clone(program);
  }, input);
 }
 const input = { ...clone(effects), definitions: [clone(naturalize)] }; delete input.definitions[0].source_labels;
 negative(`mode:cost-draw:${costs}`, d => { d.spell.instructions = []; d.spell.targets = []; d.spell.modes = [{ program: clone(program) }, { program: clone(program) }]; }, input);
}
if (option('--catalog')) {
 const catalog = JSON.parse(readFileSync(option('--catalog')));
 assert.equal(catalog.definitions.length, 5);
 positive('five-complete-source-export', catalog);
}
const report = { status: 'PREPARED_ONLY', probes: probes.length, positive: probes.filter(p => p.expected).length,
 negative: probes.filter(p => !p.expected).length, payload_sha256: createHash('sha256').update(JSON.stringify(probes)).digest('hex') };
if (!process.argv.includes('--prepare-only')) {
 assert(option('--schema') && option('--ajv-root'), '--schema and --ajv-root required; no build/install is implicit');
 const bytes = readFileSync(option('--schema'));
 const require = createRequire(join(option('--ajv-root'), 'ajv/package.json'));
 const Ajv = require('./dist/2020.js').default;
 const validate = new Ajv({ strict: false, formats: { uint32: { type: 'number', validate: n => Number.isInteger(n) && n >= 0 && n <= 0xffffffff } } }).compile(JSON.parse(bytes));
 const failures = [];
 for (const p of probes) if (validate(p.value) !== p.expected) failures.push({ name: p.name, expected: p.expected, errors: clone(validate.errors) });
 report.status = 'SCHEMA_EXECUTED'; report.schema_sha256 = createHash('sha256').update(bytes).digest('hex'); report.failures = failures;
 console.log(JSON.stringify(report, null, 2)); assert.equal(failures.length, 0);
} else console.log(JSON.stringify(report, null, 2));
