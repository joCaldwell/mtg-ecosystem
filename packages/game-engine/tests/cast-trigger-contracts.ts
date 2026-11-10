import assert from "node:assert/strict";
import Ajv2020 from "ajv/dist/2020.js";

// Shared by the complete contract gate and the focused prebuilt-CLI preflight.
// This helper never builds or runs the demonstration/profile matrix.
export function assertCastTriggerSchema(schema: any, catalog: any, legacy: any[]) {
  const validate = new Ajv2020({strict:false,formats:{uint32:{type:"number",validate:(n:number)=>Number.isInteger(n)&&n>=0&&n<=0xffff_ffff}}}).compile(schema);
  assert.equal(validate(catalog),true,JSON.stringify(validate.errors));
  assert.equal(catalog.schema_version,23);
  assert.equal(catalog.definitions.length,8);
  for (const c of legacy) assert.equal(validate(c),true,JSON.stringify(validate.errors));
  for (const version of [22,32]) assert.equal(validate({...catalog,schema_version:version}),false);
  for (const d of catalog.definitions) {
    assert.ok(d.capabilities.includes("cast_trigger_v1"));
    const old=structuredClone(catalog);old.schema_version=22;
    old.definitions=[structuredClone(d)];old.definitions[0].capabilities=old.definitions[0].capabilities.filter((c:string)=>c!=="cast_trigger_v1");
    assert.equal(validate(old),false,"old version must reject the cast condition independently of its capability label");
  }
  for (const mutate of [
    (a:any)=>{a.condition.spell_cast.caster="active_player";},
    (a:any)=>{a.condition.spell_cast.extra=true;},
    (a:any)=>{a.condition.spell_cast.spell={predicate:"any",unknown:true};},
    (a:any)=>{a.condition.spell_cast.spell={predicate:"all",terms:[]};},
    (a:any)=>{a.condition.spell_cast.spell={predicate:"any_of",terms:Array.from({length:33},()=>({predicate:"any"}))};},
    (a:any)=>{a.condition.spell_cast.spell={predicate:"type",kind:"creature",colors:["U"]};},
    (a:any)=>{a.condition.spell_cast.spell={predicate:"any_of",terms:[{predicate:"type",kind:"unknown"}]};},
  ]) {
    const bad=structuredClone(catalog);mutate(bad.definitions[0].triggered[0]);assert.equal(validate(bad),false);
  }
}
