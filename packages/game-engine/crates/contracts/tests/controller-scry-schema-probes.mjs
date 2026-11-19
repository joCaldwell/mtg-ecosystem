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
function control(amount='2',order=0,filtered=false,sorcery=false) {
 const d=clone(frozen.definitions.find(d=>d.name==='Giant Growth'));
 d.characteristics.kind=sorcery?'sorcery':'instant';d.spell.timing=sorcery?'main_phase':'instant';
 d.spell.targets=[];d.spell.instructions=[{op:'scry',amount,source_block:0}];
 d.capabilities=['normal_layout_v1',sorcery?'cast_sorcery_v1':'cast_instant_v1','controller_scry_v1'];
 const draw={op:'draw_cards',count:'1',source_block:1};
 if(order) {if(order===1)d.spell.instructions.push(draw);else d.spell.instructions.unshift(draw);d.capabilities.push('controller_draw_v1');}
 if(filtered) {
  d.spell.targets=[{slot:0,selection_id:'line:0:0',chooser:'controller',stage:'announcement',domain:'battlefield_permanent',filter:{types:['creature','artifact'],non_types:[],non_colors:[],opponent:false}}];
  d.spell.instructions.unshift({op:'grant_keywords',target_slot:0,keywords:['hexproof','indestructible'],duration:'until_end_of_turn',source_block:0});
  d.capabilities.push('temporary_permanent_keyword_program_v1','permanent_removal_v1','attachments_v1','indestructible_v1');
 }
 return {...clone(frozen),definitions:[d]};
}
for(const amount of ['0','1','2','999999999999999999999999999'])for(const order of [0,1,2])for(const sorcery of [false,true]) {
 const base=control(amount,order,false,sorcery),label=`ordinary:${amount}:${order}:${sorcery}`;positive(label,base);
 for(const cap of base.definitions[0].capabilities)negative(`${label}:missing:${cap}`,base,d=>{d.capabilities=d.capabilities.filter(c=>c!==cap)});
 for(const cap of ['attachments_v1','life_gain_v1','creature_target_v1','temporary_power_toughness_v1'])negative(`${label}:spare:${cap}`,base,d=>{d.capabilities.push(cap)});
 for(const invalid of ['-1','01',2,null,{},''])negative(`${label}:amount:${JSON.stringify(invalid)}`,base,d=>{d.spell.instructions.find(i=>i.op==='scry').amount=invalid});
}
for(const amount of ['0','1','17'])for(const sorcery of [false,true])positive(`filtered:${amount}:${sorcery}`,control(amount,0,true,sorcery));
// A synthetic color-cast trigger exercises schema capability closure independently
// of the ordinary-spell controls. Source and AST hashes describe this exact block.
const triggerCatalog = clone(frozen);
const observer = clone(frozen.definitions.find(d => d.name === 'Grizzly Bears'));
triggerCatalog.definitions = [observer];
observer.name = 'Synthetic Cast Scry Observer';
observer.oracle_id = 'synthetic-cast-scry-schema';
observer.provenance.printing_id = 'synthetic-cast-scry-schema-print';
const text = 'Whenever you cast a red spell, scry 5.';
const source = { ...JSON.parse(observer.provenance.source_json), name:observer.name, id:observer.provenance.printing_id,
  oracle_id:observer.oracle_id, oracle_text:text };
const ast = [{ kind:'triggered', trigger:{ trigger:'cast', who:{player:'you'}, what:{colors:['red'],cls:'spell'} },
  effects:[{steps:[{effect:'scry',amount:{amount:'fixed',value:5}}]}] }];
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
observer.provenance.source_json = JSON.stringify(source);
observer.provenance.source_sha256 = sha256(observer.provenance.source_json);
observer.provenance.abilities_json = JSON.stringify(ast);
observer.provenance.ast_sha256 = sha256(JSON.stringify([ast]));
delete observer.source_labels;
const requiredTriggerCaps = ['cast_trigger_v1','triggered_ability_v1','cast_color_predicate_v1','controller_scry_v1'];
observer.capabilities.push(...requiredTriggerCaps);
observer.triggered = [{ functioning_zone:'battlefield', condition:{spell_cast:{caster:'controller',spell:{predicate:'spell_color',color:'R'}}},
  program:{functioning_zone:'stack',timing:'instant',resolution:'instructions',targets:[],
    instructions:[{op:'scry',amount:'5',source_block:0}]} }];
positive('cast-scry:complete',triggerCatalog);
for (const cap of requiredTriggerCaps) {
  negative('cast-scry:missing:'+cap, triggerCatalog,d=>{d.capabilities=d.capabilities.filter(c=>c!==cap)});
}
negative('cast-scry:erased',triggerCatalog,d=>{delete d.triggered;d.capabilities=d.capabilities.filter(c=>!requiredTriggerCaps.includes(c))});
const damageCatalog = clone(triggerCatalog);
const damageObserver = damageCatalog.definitions[0];
damageObserver.name = 'Violet Observatory';
const damageLine = 'Whenever you cast a red spell, ~ deals 5 damage to target opponent.';
const damageSource = {...source,name:damageObserver.name,oracle_text:damageLine.replace('~',damageObserver.name)};
const damageAst = clone(ast);
damageAst[0].effects[0].steps[0] = {effect:'damage',source:{ref:'self'},amount:{amount:'fixed',value:5},
  to:{to:'player',ref:{player:'target-opponent',selectionId:'line:0:12'}}};
damageObserver.provenance.source_json=JSON.stringify(damageSource);
damageObserver.provenance.source_sha256=sha256(damageObserver.provenance.source_json);
damageObserver.provenance.abilities_json=JSON.stringify(damageAst);
damageObserver.provenance.ast_sha256=sha256(JSON.stringify([damageAst]));
damageObserver.capabilities=damageObserver.capabilities.filter(c=>c!=='controller_scry_v1');
damageObserver.capabilities.push('opponent_target_v1','noncombat_damage_v1');
const damageProgram=damageObserver.triggered[0].program;
damageProgram.instructions=[{op:'deal_damage',target_slot:0,amount:'5',source_block:0}];
damageProgram.targets=[{slot:0,selection_id:'line:0:12',domain:'opponent_player',chooser:'controller',stage:'announcement'}];
positive('cast-damage:complete',damageCatalog);
for(const cap of ['cast_trigger_v1','cast_color_predicate_v1','triggered_ability_v1','opponent_target_v1','noncombat_damage_v1']) {
  negative('cast-damage:missing:'+cap,damageCatalog,d=>{d.capabilities=d.capabilities.filter(c=>c!==cap)});
}
negative('cast-damage:wrong-selection',damageCatalog,d=>{d.triggered[0].program.targets[0].selection_id='line:0:11'});
negative('cast-damage:second-instruction',damageCatalog,d=>{d.triggered[0].program.instructions.push(clone(d.triggered[0].program.instructions[0]))});
const ordinary=control(),filtered=control('1',0,true);
for(const [name,change] of [
 ['double',d=>d.spell.instructions.push(clone(d.spell.instructions[0]))],
 ['life',d=>{d.spell.instructions.push({op:'gain_life',recipient:'controller',amount:'2',source_block:0});d.capabilities.push('life_gain_v1')}],
 ['sacrifice',d=>d.spell.additional_sacrifice={types:['creature'],subtypes:[]}],
 ['zone',d=>d.spell.functioning_zone='battlefield'],['resolution',d=>d.spell.resolution='permanent'],
 ['targets',d=>d.spell.targets=clone(filtered.definitions[0].spell.targets)],
 ['amount-missing',d=>delete d.spell.instructions[0].amount],['extra-actor',d=>d.spell.instructions[0].who=null],
 ['extra-recipient',d=>d.spell.instructions[0].recipient=null],['block-missing',d=>delete d.spell.instructions[0].source_block],
 ['block-negative',d=>d.spell.instructions[0].source_block=-1],
 ['mode',d=>{const p=clone(d.spell);d.spell.instructions=[];d.spell.modes=[{program:p},{program:clone(p)}]}],
 ['x',d=>d.x_mana_cost=true],['static',d=>d.static_abilities=[{functioning_zone:'battlefield',effect:'cant_be_countered'}]],
 ['kind',d=>d.characteristics.kind='creature'],['power',d=>d.characteristics.power='1'],
 ['flash',d=>d.keywords=['flash']],['additional-type',d=>d.characteristics.additional_types=['artifact']],
 ['basic',d=>d.characteristics.basic=true],['legendary',d=>d.characteristics.legendary=true],
 ['timing',d=>d.spell.timing='main_phase'],
]) negative(`ordinary:${name}`,ordinary,change);
for(const [name,change] of [
 ['reverse-types',d=>d.spell.targets[0].filter.types.reverse()],['one-type',d=>d.spell.targets[0].filter.types=['artifact']],
 ['land',d=>d.spell.targets[0].filter.types=['land']],['null-filter',d=>d.spell.targets[0].filter=null],
 ['missing-filter',d=>delete d.spell.targets[0].filter],['controlled-domain',d=>d.spell.targets[0].domain='controlled_battlefield_permanent'],
 ['non-types',d=>d.spell.targets[0].filter.non_types=['land']],['opponent',d=>d.spell.targets[0].filter.opponent=true],
 ['tapped',d=>d.spell.targets[0].filter.tapped=true],['grant-block',d=>d.spell.instructions[0].source_block=1],
 ['scry-block',d=>d.spell.instructions[1].source_block=1],['modifier',d=>d.spell.instructions[0].modifier={power:'1',toughness:'1'}],
 ['life-tail',d=>{d.spell.instructions[1]={op:'gain_life',recipient:'controller',amount:'2',source_block:0};d.capabilities=d.capabilities.filter(c=>c!=='controller_scry_v1');d.capabilities.push('life_gain_v1')}],
 ['wrong-slot',d=>d.spell.instructions[0].target_slot=1],['third-op',d=>{d.spell.instructions.push({op:'draw_cards',count:'1',source_block:0});d.capabilities.push('controller_draw_v1')}]
])negative(`filtered:${name}`,filtered,change);
const pure=clone(filtered);pure.definitions[0].spell.instructions.pop();pure.definitions[0].capabilities=pure.definitions[0].capabilities.filter(c=>c!=='controller_scry_v1');positive('filtered:pure-grant',pure);
const explicitNone=clone(filtered);explicitNone.definitions[0].spell.instructions[0].modifier=null;positive('filtered:explicit-none',explicitNone);
for(const host of ['Samite Healer','Blood Artist'])negative(`ability:${host}`,ordinary,d=>{const p=clone(d.spell),source=clone(frozen.definitions.find(x=>x.name===host));if(source.activated?.length)source.activated[0].program=p;else source.triggered[0].program=p;Object.assign(d,source);d.capabilities.push('controller_scry_v1')});
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
