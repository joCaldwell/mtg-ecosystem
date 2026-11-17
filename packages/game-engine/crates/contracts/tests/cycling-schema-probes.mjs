// Bounded synthetic IR controls, never card/catalog/profile admission.
// --prepare-only constructs/hashes probes; schema execution requires explicit
// --schema exported.json --ajv-root existing/node_modules, no builds/installs.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
const base = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-ir.json', import.meta.url)));
const land = structuredClone(base.definitions.find(d => d.characteristics.basic));
delete land.source_labels;
land.characteristics.basic = false;
land.characteristics.subtypes = ['Forest', 'Island', 'Mountain'];
land.static_abilities = [{ functioning_zone: 'battlefield', effect: 'enters_tapped' }];
land.activated = [{ functioning_zone: 'hand', cost: { discard_source: { mana: { generic: '3', colored: [] } } }, program: {
  timing: 'instant', resolution: 'instructions', functioning_zone: 'stack', targets: [], modes: [],
  instructions: [{ op: 'draw_cards', count: '1', source_block: 1 }],
} }];
land.capabilities = ['normal_layout_v1','land_play_v1','typed_nonbasic_intrinsic_mana_v1','enters_tapped_v1','activated_ability_v1','hand_activation_v1','discard_source_cost_v1','mana_activation_cost_v1','controller_draw_v1'].sort();
const baseline = { ...base, definitions: [land] };
const probes = [];
function probe(name, expected, change = () => {}) { const c = structuredClone(baseline); change(c.definitions[0]); probes.push({ name, expected, catalog: c }); }
probe('synthetic whole composition', true);
for (const n of ['0','17','999999999999999999999999999999999999999999']) probe(`generic ${n}`, true, d => d.activated[0].cost.discard_source.mana.generic = n);
probe('colored and colorless fixed mana', true, d => d.activated[0].cost.discard_source.mana.colored = ['U','C']);
probe('one subtype, no triple ceiling', true, d => d.characteristics.subtypes = ['Forest']);
for (const [name, change] of [
  ['BF activation',d=>d.activated[0].functioning_zone='battlefield'],
  ['stack activation',d=>d.activated[0].functioning_zone='stack'],
  ['wrong cost',d=>d.activated[0].cost={mana:{cost:{generic:'3',colored:[]}}}],
  ['missing mana',d=>delete d.activated[0].cost.discard_source.mana],
  ['null cost',d=>d.activated[0].cost.discard_source=null],
  ['null mana',d=>d.activated[0].cost.discard_source.mana=null],
  ['unknown cost field',d=>d.activated[0].cost.discard_source.other=true],
  ['negative mana',d=>d.activated[0].cost.discard_source.mana.generic='-1'],
  ['numeric mana',d=>d.activated[0].cost.discard_source.mana.generic=3],
  ['null mana quantity',d=>d.activated[0].cost.discard_source.mana.generic=null],
  ['noncanonical quantity',d=>d.activated[0].cost.discard_source.mana.generic='03'],
  ['two draws',d=>d.activated[0].program.instructions[0].count='2'],
  ['opponent draw',d=>d.activated[0].program.instructions[0].recipient='each_opponent'],
  ['explicit controller draw',d=>d.activated[0].program.instructions[0].recipient='controller'],
  ['optional draw',d=>d.activated[0].program.instructions=[{op:'optional_draw_cards',source_block:1}]],
  ['effect discard tail',d=>d.activated[0].program.instructions.push({op:'discard_cards',count:'1',source_block:1})],
  ['partial no draw',d=>d.activated[0].program.instructions=[]],
  ['hand program',d=>d.activated[0].program.functioning_zone='hand'],
  ['hand static',d=>d.static_abilities[0].functioning_zone='hand'],
  ['hand intrinsic',d=>d.intrinsic_zone='hand'],
  ['optional entry composition',d=>d.static_abilities[0].effect={enters_tapped_unless_pay_life:{amount:'2'}}],
  ['missing tapped',d=>d.static_abilities=[]],
  ['duplicate activation',d=>d.activated.push(structuredClone(d.activated[0]))],
  ['wrong kind',d=>d.characteristics.kind='artifact'],
  ['untouched subtype duplicate fence',d=>d.characteristics.subtypes=['Forest','Forest']],
  ['empty subtype',d=>d.characteristics.subtypes=[]],
  ['main timing',d=>d.activated[0].program.timing='main_phase'],
  ['extra unknown activation',d=>d.activated[0].from='graveyard'],
  ['orphan hand capability',d=>d.activated=[]],
]) probe(name, false, change);
for (const cap of land.capabilities) probe(`missing ${cap}`, false, d => d.capabilities = d.capabilities.filter(c=>c!==cap));
for (const cap of ['pay_life_cost_v1','optional_life_land_entry_v1','composite_activation_cost_v1','controller_discard_v1']) probe(`spare ${cap}`, false, d => d.capabilities.push(cap));
const report = { kind: 'synthetic-cycling-schema-probes', probes: probes.length, positive: probes.filter(p=>p.expected).length, negative: probes.filter(p=>!p.expected).length, payload_sha256: createHash('sha256').update(JSON.stringify(probes)).digest('hex'), schema_execution: 'UNRUN' };
assert(report.positive > 0 && report.negative > 0);
if (!process.argv.includes('--prepare-only')) {
  const option = k => { const i=process.argv.indexOf(k); assert(i>=0,`missing ${k}`); return resolve(process.argv[i+1]); };
  const require = createRequire(join(option('--ajv-root'),'ajv/package.json'));
  const schema = readFileSync(option('--schema'));
  const Ajv = require('./dist/2020.js').default;
  const validate = new Ajv({strict:false,formats:{uint32:{type:'number',validate:n=>Number.isInteger(n)&&n>=0&&n<=0xffffffff}}}).compile(JSON.parse(schema));
  const failures = [];
  for (const {name,expected,catalog} of probes) if (validate(catalog)!==expected) failures.push({name,expected,errors:structuredClone(validate.errors)});
  report.schema_execution='executed'; report.schema_sha256=createHash('sha256').update(schema).digest('hex'); report.failures=failures;
  console.log(JSON.stringify(report,null,2)); assert.equal(failures.length,0);
} else console.log(JSON.stringify(report,null,2));
