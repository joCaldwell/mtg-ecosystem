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
function control(count='3',sorcery=false){const d=clone(frozen.definitions.find(d=>d.name==='Giant Growth'));d.characteristics.kind=sorcery?'sorcery':'instant';d.spell.timing=sorcery?'main_phase':'instant';d.spell.targets=[];d.spell.instructions=[{op:'draw_controller_cards',count,source_block:0}];d.capabilities=['normal_layout_v1',sorcery?'cast_sorcery_v1':'cast_instant_v1','controller_counted_draw_v1'];return {...clone(frozen),definitions:[d]};}
for(const count of ['0','1','2','3','999999999999999999999999999999'])for(const sorcery of [false,true]){const base=control(count,sorcery),label=`draw:${count}:${sorcery}`;positive(label,base);for(const cap of base.definitions[0].capabilities)negative(`${label}:missing:${cap}`,base,d=>d.capabilities=d.capabilities.filter(c=>c!==cap));for(const cap of ['controller_draw_v1','controller_draw_discard_v1','controller_hand_return_v1'])negative(`${label}:spare:${cap}`,base,d=>d.capabilities.push(cap));}
const base=control();for(const [name,change] of [
['negative',d=>d.spell.instructions[0].count='-1'],['noncanonical',d=>d.spell.instructions[0].count='03'],['number',d=>d.spell.instructions[0].count=3],['missing-count',d=>delete d.spell.instructions[0].count],['null-count',d=>d.spell.instructions[0].count=null],['recipient',d=>d.spell.instructions[0].recipient='event_player'],['block',d=>d.spell.instructions[0].source_block=1],['missing-op',d=>d.spell.instructions=[]],['extra',d=>d.spell.instructions.push(clone(d.spell.instructions[0]))],['discard-tail',d=>d.spell.instructions.push({op:'discard_cards',count:'2',source_block:0})],['cost',d=>d.spell.additional_sacrifice={types:['creature'],subtypes:[]}],['mode',d=>{const p=clone(d.spell);d.spell.instructions=[];d.spell.modes=[{program:p},{program:clone(p)}]}],['keyword',d=>d.keywords=['flash']],['kind',d=>d.characteristics.kind='creature'],['basic',d=>d.characteristics.basic=true],['power',d=>d.characteristics.power='1'],['legendary',d=>d.characteristics.legendary=true],['additional-type',d=>d.characteristics.additional_types=['artifact']],['x',d=>d.x_mana_cost=true],['zone',d=>d.spell.functioning_zone='battlefield'],['resolution',d=>d.spell.resolution='permanent'],['timing',d=>d.spell.timing='main_phase'],['target',d=>{d.spell.targets=[{slot:0,selection_id:'extra',chooser:'controller',stage:'announcement',domain:'any_player'}];d.capabilities.push('any_player_target_v1')}]
])negative(`strict:${name}`,base,change);
for(const host of ['Samite Healer','Blood Artist'])negative(`ability:${host}`,base,d=>{const p=clone(d.spell),s=clone(frozen.definitions.find(x=>x.name===host));if(s.activated?.length)s.activated[0].program=p;else s.triggered[0].program=p;Object.assign(d,s);d.capabilities.push('controller_counted_draw_v1')});
// Retain the entire previous pure-family inventory as an exact prefix.
assert.equal(createHash('sha256').update(JSON.stringify(positives)).digest('hex'),'6e24fca97cef504250856c381d61c9805136f2d1b193ce4f1f7cc3a25736b38f');
assert.equal(createHash('sha256').update(JSON.stringify(probes)).digest('hex'),'88707d18123c993303a3d99751d54fba58ad7914a3117cf9f17bf7ddc4331ef9');
function withLoss(count='3',amount='3',sorcery=false){const c=control(count,sorcery),d=c.definitions[0];d.spell.instructions.push({op:'lose_life',recipient:'controller',amount,source_block:0});d.capabilities.push('life_loss_v1');return c;}
for(const [draw,loss] of [['0','1'],['1','3'],['3','3'],['999999999999999999999999','999999999999999999999999']])for(const sorcery of [false,true]){const c=withLoss(draw,loss,sorcery),label=`life:${draw}:${loss}:${sorcery}`;positive(label,c);for(const cap of c.definitions[0].capabilities)negative(`${label}:missing:${cap}`,c,d=>d.capabilities=d.capabilities.filter(x=>x!==cap));negative(`${label}:spare-gain`,c,d=>d.capabilities.push('life_gain_v1'));}
const life=withLoss();for(const [name,change] of [
['zero',d=>d.spell.instructions[1].amount='0'],['negative',d=>d.spell.instructions[1].amount='-1'],['number',d=>d.spell.instructions[1].amount=3],['null',d=>d.spell.instructions[1].amount=null],['block',d=>d.spell.instructions[1].source_block=1],['missing-recipient',d=>delete d.spell.instructions[1].recipient],['null-recipient',d=>d.spell.instructions[1].recipient=null],['event-recipient',d=>d.spell.instructions[1].recipient='event_player'],['opponents',d=>d.spell.instructions[1].recipient='each_opponent'],['target',d=>d.spell.instructions[1].recipient={target:{target_slot:0}}],['gain',d=>{d.spell.instructions[1].op='gain_life';d.capabilities=d.capabilities.filter(c=>c!=='life_loss_v1');d.capabilities.push('life_gain_v1')}],['extra',d=>d.spell.instructions.push(clone(d.spell.instructions[1]))],['reorder',d=>d.spell.instructions.reverse()],['removed-tail-spare-cap',d=>d.spell.instructions.pop()],['removed-draw',d=>d.spell.instructions.shift()]
])negative(`life-strict:${name}`,life,change);
negative('pure:spare-loss',base,d=>d.capabilities.push('life_loss_v1'));
for(const [op,cap] of [['put_hand_on_library','controller_hand_return_v1'],['discard_cards','controller_draw_discard_v1']])negative(`paired:spare-loss:${op}`,life,d=>{d.spell.instructions[1]={op,count:'2',source_block:0};d.capabilities=d.capabilities.filter(c=>c!=='controller_counted_draw_v1');d.capabilities.push(cap)});
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
