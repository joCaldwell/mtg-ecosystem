// Immutable exported data only: no native execution, installation or root import.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const option=k=>{const i=process.argv.indexOf(k);assert(i>=0);return resolve(process.argv[i+1]);};
const require=createRequire(join(option('--ajv-root'),'ajv/package.json'));
const schema=readFileSync(option('--schema')), bytes=readFileSync(option('--catalog'));
const catalog=JSON.parse(bytes);
assert.equal(catalog.definitions.length,3);
const Ajv=require('./dist/2020.js').default;
const validate=new Ajv({strict:false,formats:{uint32:{type:'number',validate:n=>Number.isInteger(n)&&n>=0&&n<=0xffffffff}}}).compile(JSON.parse(schema));
const report={schema_sha256:createHash('sha256').update(schema).digest('hex'),catalog_sha256:createHash('sha256').update(bytes).digest('hex'),ajv:require('./package.json').version,positive:0,negative:0,failures:[]};
function check(name,x,expected){if(validate(x)!==expected)report.failures.push({name,expected,errors:structuredClone(validate.errors)});else report[expected?'positive':'negative']++;}
check('three frozen complete sources',catalog,true);
const types=['plains','island','swamp','mountain','forest'];
for(const d of catalog.definitions){
 const base={...structuredClone(catalog),definitions:[structuredClone(d)]};
 for(const q of ['1','2','3','101','999999999999999999999999999999999999'])for(let mask=1;mask<32;mask++)for(const tapped of [false,true]){
  const x=structuredClone(base),s=x.definitions[0].spell.instructions[0].search;
  s.maximum=q;s.any_basic_land_types=types.filter((_,i)=>mask&(1<<i));s.force_tapped=tapped;
  check(`${d.name}:q${q}:types${mask}:tapped${tapped}`,x,true);
 }
 for(const [name,change] of [
  ['zero',d=>d.spell.instructions[0].search.maximum='0'],['negative',d=>d.spell.instructions[0].search.maximum='-1'],
  ['number',d=>d.spell.instructions[0].search.maximum=3],['null quantity',d=>d.spell.instructions[0].search.maximum=null],
  ['leading zero',d=>d.spell.instructions[0].search.maximum='01'],['missing quantity',d=>delete d.spell.instructions[0].search.maximum],
  ['empty selector',d=>d.spell.instructions[0].search.any_basic_land_types=[]],['duplicate',d=>d.spell.instructions[0].search.any_basic_land_types=['forest','forest']],
  ['noncanonical',d=>d.spell.instructions[0].search.any_basic_land_types=['forest','island']],['unknown type',d=>d.spell.instructions[0].search.any_basic_land_types=['desert']],
  ['extra filter',d=>d.spell.instructions[0].search.basic=true],['missing policy',d=>delete d.spell.instructions[0].search.force_tapped],
  ['null policy',d=>d.spell.instructions[0].search.force_tapped=null],['extra opcode field',d=>d.spell.instructions[0].maximum=1],
  ['wrong block',d=>d.spell.instructions[0].source_block=1],['extra instruction',d=>d.spell.instructions.push({op:'draw_cards',count:'1',source_block:0})],
  ['extra search',d=>d.spell.instructions.push(structuredClone(d.spell.instructions[0]))],['permanent',d=>d.characteristics.kind='creature'],
  ['wrong zone',d=>d.spell.functioning_zone='battlefield'],['wrong resolution',d=>d.spell.resolution='permanent'],
  ['optional guard',d=>{d.spell.optional_additional_costs=[{identity:`card:${d.oracle_id}/faces/0/abilities/0/keywords/0#keyword`,source_block:0,mana:{generic:'1',colored:[]}}];d.capabilities.push('optional_additional_mana_cost_v1');}],
  ['conditional guard',d=>d.spell.instructions.push({op:'draw_cards_if_additional_cost',cost_identity:'foreign',count:'1',source_block:0})],
  ['zero life contextual fence',d=>d.spell.instructions.push({op:'gain_life',recipient:'controller',amount:'0',source_block:0})],
  ['activation',d=>{d.activated=[{cost:'tap_source',program:structuredClone(d.spell)}];}],
  ['mode',d=>{const child=structuredClone(d.spell);d.spell.instructions=[];d.spell.modes=[{program:child},{program:structuredClone(child)}];}],
  ['orphan capability',d=>d.spell.instructions=[]],
 ]){const x=structuredClone(base);change(x.definitions[0]);check(`${d.name}:${name}`,x,false);}
 for(const cap of d.capabilities){const x=structuredClone(base);x.definitions[0].capabilities=d.capabilities.filter(c=>c!==cap);check(`${d.name}:missing ${cap}`,x,false);}
 for(const cap of ['basic_land_search_v1','basic_land_split_search_v1']){const x=structuredClone(base);x.definitions[0].capabilities.push(cap);check(`${d.name}:spare ${cap}`,x,false);}
}
console.log(JSON.stringify(report,null,2));assert.equal(report.failures.length,0);
