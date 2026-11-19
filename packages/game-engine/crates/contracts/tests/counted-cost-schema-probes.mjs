// Immutable exported data only; no Cargo, installs, or source imports.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const option=k=>{let i=process.argv.indexOf(k);assert(i>=0);return resolve(process.argv[i+1]);};
const require=createRequire(join(option('--ajv-root'),'ajv/package.json'));
const schema=readFileSync(option('--schema')), bytes=readFileSync(option('--catalog'));
const catalog=JSON.parse(bytes), d=catalog.definitions[0];
assert.equal(d.oracle_id,'71366469-04ea-4526-bf24-7abc02fdc5ab');
const Ajv=require('./dist/2020.js').default;
const validate=new Ajv({strict:false,formats:{uint32:{type:'number',validate:n=>Number.isInteger(n)&&n>=0&&n<=0xffffffff}}}).compile(JSON.parse(schema));
const report={schema_sha256:createHash('sha256').update(schema).digest('hex'),catalog_sha256:createHash('sha256').update(bytes).digest('hex'),ajv:require('./package.json').version,positive:0,negative:0,failures:[]};
function check(name,x,expected){if(validate(x)!==expected)report.failures.push({name,expected,errors:structuredClone(validate.errors)});else report[expected?'positive':'negative']++;}
check('frozen complete',catalog,true);
for(const q of ['1','2','999999999999999999999999999999999999'])for(const n of [1,2,17]){
 const x=structuredClone(catalog);x.definitions[0].static_abilities=Array.from({length:n},()=>structuredClone(d.static_abilities[0]));
 for(const a of x.definitions[0].static_abilities)a.effect.reduce_self_generic_cost.per_object=q;
 check(`positive exact ${q} slots ${n}`,x,true);
}
for(const [name,change] of [
 ['zone',a=>a.functioning_zone='battlefield'],
 ['zero',a=>a.effect.reduce_self_generic_cost.per_object='0'],
 ['negative',a=>a.effect.reduce_self_generic_cost.per_object='-1'],
 ['number',a=>a.effect.reduce_self_generic_cost.per_object=1],
 ['null',a=>a.effect.reduce_self_generic_cost.per_object=null],
 ['missing quantity',a=>delete a.effect.reduce_self_generic_cost.per_object],
 ['extra',a=>a.effect.reduce_self_generic_cost.extra=1],
 ['creature',a=>a.effect.reduce_self_generic_cost.count.kind='creature'],
 ['all controllers',a=>a.effect.reduce_self_generic_cost.count.controller='any'],
 ['owner',a=>a.effect.reduce_self_generic_cost.count.controller='owner'],
 ['extra selector',a=>a.effect.reduce_self_generic_cost.count.owner='you'],
 ['null selector',a=>a.effect.reduce_self_generic_cost.count=null],
 ['missing controller',a=>delete a.effect.reduce_self_generic_cost.count.controller],
 ]){const x=structuredClone(catalog);change(x.definitions[0].static_abilities[0]);check(name,x,false);}
for(const cap of ['flying_v1','counted_self_generic_cost_reduction_v1','cascade_v1','resolution_cast_v1','random_bottom_v1']){const x=structuredClone(catalog);x.definitions[0].capabilities=d.capabilities.filter(c=>c!==cap);check(`missing ${cap}`,x,false);}
const orphan=structuredClone(catalog);orphan.definitions[0].static_abilities=[];check('orphan capability',orphan,false);
// Existing optional/conditional guards apply even with the new descriptor.
for(const field of ['spell','triggered']){
 const x=structuredClone(catalog), p=field==='spell'?x.definitions[0].spell:x.definitions[0].triggered[0].program;
 p.optional_additional_costs=[{identity:`card:${d.oracle_id}/faces/0/abilities/0/keywords/0#keyword`,source_block:0,mana:{generic:'1',colored:[]}}];
 x.definitions[0].capabilities.push('optional_additional_mana_cost_v1');check(`optional guard ${field}`,x,false);
}
console.log(JSON.stringify(report,null,2));assert.equal(report.failures.length,0);
