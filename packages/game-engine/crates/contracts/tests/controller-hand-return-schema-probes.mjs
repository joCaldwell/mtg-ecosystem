// Separate synthetic IR inventory. Native/schema execution is UNRUN by A.
// --prepare-only constructs payloads without Ajv, schema export or native work.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';
const frozen = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-effects-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);


const probes=[],positives=[];
function positive(name,value) {const copy=clone(value);positives.push({name,value:copy,exact:JSON.stringify(copy)});probes.push({name,value:copy,expected:true});}
function negative(name,base,change) {const value=clone(base);change(value.definitions[0]);probes.push({name,value,expected:false});}
function control(draw='3',count='2',sorcery=false){const d=clone(frozen.definitions.find(d=>d.name==='Giant Growth'));d.characteristics.kind=sorcery?'sorcery':'instant';d.spell.timing=sorcery?'main_phase':'instant';d.spell.targets=[];d.spell.instructions=[{op:'draw_controller_cards',count:draw,source_block:0},{op:'put_hand_on_library',count,source_block:0}];d.capabilities=['normal_layout_v1',sorcery?'cast_sorcery_v1':'cast_instant_v1','controller_hand_return_v1'];return {...clone(frozen),definitions:[d]};}
for(const [draw,count] of [['0','0'],['3','2'],['999999999999999999999999','999999999999999999999999']])for(const sorcery of [false,true]){const base=control(draw,count,sorcery),label=`hand:${draw}:${count}:${sorcery}`;positive(label,base);for(const cap of base.definitions[0].capabilities)negative(`${label}:missing:${cap}`,base,d=>d.capabilities=d.capabilities.filter(c=>c!==cap));for(const cap of ['controller_draw_v1','controller_library_order_v1','controller_library_selection_v1'])negative(`${label}:spare:${cap}`,base,d=>d.capabilities.push(cap));}
const base=control();for(const [name,change] of [
['negative-draw',d=>d.spell.instructions[0].count='-1'],['negative-return',d=>d.spell.instructions[1].count='-1'],['noncanonical',d=>d.spell.instructions[0].count='03'],['number',d=>d.spell.instructions[1].count=2],['missing-count',d=>delete d.spell.instructions[1].count],['extra-field',d=>d.spell.instructions[0].recipient=null],['draw-block',d=>d.spell.instructions[0].source_block=1],['return-block',d=>d.spell.instructions[1].source_block=1],['missing-return',d=>d.spell.instructions.pop()],['missing-draw',d=>d.spell.instructions.shift()],['extra',d=>d.spell.instructions.push(clone(d.spell.instructions[1]))],['reorder',d=>d.spell.instructions.reverse()],['cost',d=>d.spell.additional_sacrifice={types:['creature'],subtypes:[]}],['mode',d=>{const p=clone(d.spell);d.spell.instructions=[];d.spell.modes=[{program:p},{program:clone(p)}]}],['keyword',d=>d.keywords=['flash']],['kind',d=>d.characteristics.kind='creature'],['basic',d=>d.characteristics.basic=true],['power',d=>d.characteristics.power='1'],['legendary',d=>d.characteristics.legendary=true],['additional-type',d=>d.characteristics.additional_types=['artifact']],['x',d=>d.x_mana_cost=true],['zone',d=>d.spell.functioning_zone='battlefield'],['resolution',d=>d.spell.resolution='permanent'],['timing',d=>d.spell.timing='main_phase'],['target',d=>{d.spell.targets=[{slot:0,selection_id:'extra',chooser:'controller',stage:'announcement',domain:'any_player'}];d.capabilities.push('any_player_target_v1')}]
])negative(`strict:${name}`,base,change);
for(const host of ['Samite Healer','Blood Artist'])negative(`ability:${host}`,base,d=>{const p=clone(d.spell),s=clone(frozen.definitions.find(x=>x.name===host));if(s.activated?.length)s.activated[0].program=p;else s.triggered[0].program=p;Object.assign(d,s);d.capabilities.push('controller_hand_return_v1')});
for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact);
const hash = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const summary = { format_version: 1, status: 'prepared_only', probes: probes.length,
  positive: probes.filter(p => p.expected).length, negative: probes.filter(p => !p.expected).length,
  frozen_controls_sha256: hash(positives), probe_payloads_sha256: hash(probes) };
if (process.argv.includes('--prepare-only')) console.log(JSON.stringify(summary));
else {
  const option = name => { const i = process.argv.indexOf(name); return i < 0 ? undefined : process.argv[i + 1]; };
  const path = option('--schema'); assert(path, '--schema PATH required (or --prepare-only)');
  const root = option('--ajv-root');
  const require = createRequire(root ? join(resolve(root), '__schema_probe__.cjs') : import.meta.url);
  const Ajv2020 = require('ajv/dist/2020.js').default;
  const bytes = readFileSync(path), ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addFormat('uint32', { type: 'number', validate: n => Number.isInteger(n) && n >= 0 && n <= 4294967295 });
  const validate = ajv.compile(JSON.parse(bytes)), failures = [];
  for (const probe of probes) {
    const actual = validate(clone(probe.value));
    if (actual !== probe.expected) failures.push({ name: probe.name, expected: probe.expected, actual, errors: clone(validate.errors) });
  }
  for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact);
  console.log(JSON.stringify({ ...summary, status: failures.length ? 'failed' : 'passed', schema_sha256: createHash('sha256').update(bytes).digest('hex'), failures }, null, 2));
  if (failures.length) process.exitCode = 1;
}
