// Frozen exported source catalog/schema only. No build, install or cache access.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {join,resolve} from 'node:path';
import {createHash} from 'node:crypto';
const option=k=>{const i=process.argv.indexOf(k);assert(i>=0);return resolve(process.argv[i+1]);};
const require=createRequire(join(option('--ajv-root'),'ajv/package.json'));
const bytes=readFileSync(option('--catalog')),schema=readFileSync(option('--schema')),catalog=JSON.parse(bytes);
assert.equal(catalog.definitions.length,2);
const Ajv=require('./dist/2020.js').default;
const validate=new Ajv({strict:false,formats:{uint32:{type:'number',validate:n=>Number.isInteger(n)&&n>=0&&n<=0xffffffff}}}).compile(JSON.parse(schema));
const report={schema_sha256:createHash('sha256').update(schema).digest('hex'),catalog_sha256:createHash('sha256').update(bytes).digest('hex'),ajv:require('./package.json').version,positive:0,negative:0,failures:[]};
function check(name,x,expected){if(validate(x)!==expected)report.failures.push({name,expected,errors:structuredClone(validate.errors)});else report[expected?'positive':'negative']++;}
check('two complete frozen sources',catalog,true);
const subtypes=['Plains','Island','Swamp','Mountain','Forest'];
for(const d of catalog.definitions){
 const base={...structuredClone(catalog),definitions:[structuredClone(d)]};
 for(let mask=1;mask<32;mask++)for(const n of ['0','1','2','3','999999999999999999999999999999999999999999999999999999']){
  const x=structuredClone(base);x.definitions[0].characteristics.subtypes=subtypes.filter((_,i)=>mask&(1<<i));
  x.definitions[0].static_abilities[0].effect.enters_tapped_unless_controller_basic_lands.minimum=n;
  check(`${d.name}:types${mask}:minimum${n}`,x,true);
 }
 for(const [name,change] of [
  ['negative',d=>d.static_abilities[0].effect.enters_tapped_unless_controller_basic_lands.minimum='-1'],
  ['number',d=>d.static_abilities[0].effect.enters_tapped_unless_controller_basic_lands.minimum=2],
  ['null',d=>d.static_abilities[0].effect.enters_tapped_unless_controller_basic_lands.minimum=null],
  ['missing minimum',d=>delete d.static_abilities[0].effect.enters_tapped_unless_controller_basic_lands.minimum],
  ['leading zero',d=>d.static_abilities[0].effect.enters_tapped_unless_controller_basic_lands.minimum='02'],
  ['extra effect operand',d=>d.static_abilities[0].effect.enters_tapped_unless_controller_basic_lands.owner=true],
  ['empty subtypes',d=>d.characteristics.subtypes=[]],['unknown subtype',d=>d.characteristics.subtypes=['Desert']],
  ['duplicate subtype',d=>d.characteristics.subtypes=['Forest','Forest']],['basic',d=>d.characteristics.basic=true],
  ['additional type',d=>d.characteristics.additional_types=['artifact']],['spell',d=>d.spell={}],
  ['power',d=>d.characteristics.power='1'],['cost',d=>d.characteristics.mana_cost.generic='1'],
  ['wrong zone',d=>d.intrinsic_zone='stack'],['wrong static zone',d=>d.static_abilities[0].functioning_zone='stack'],
  ['extra static',d=>d.static_abilities.push({functioning_zone:'battlefield',effect:'cant_be_countered'})],
  ['explicit mana',d=>d.mana_abilities=[{}]],['activation',d=>d.activated=[{}]],['trigger',d=>d.triggered=[{}]],
  ['keyword',d=>d.keywords=['flash']],['orphan entry capability',d=>d.static_abilities=[]],
 ]){const x=structuredClone(base);change(x.definitions[0]);check(`${d.name}:${name}`,x,false);}
 for(const cap of d.capabilities){const x=structuredClone(base);x.definitions[0].capabilities=d.capabilities.filter(c=>c!==cap);check(`${d.name}:missing ${cap}`,x,false);}
 for(const cap of ['basic_land_mana_v1','enters_tapped_v1','board_wipes_v1']){const x=structuredClone(base);x.definitions[0].capabilities.push(cap);check(`${d.name}:spare ${cap}`,x,false);}
}
console.log(JSON.stringify(report,null,2));assert.equal(report.failures.length,0);
