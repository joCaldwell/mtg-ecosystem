import assert from "node:assert/strict";
import Ajv2020 from "ajv/dist/2020.js";

// Shared focused/full contract assertions. Callers supply a prebuilt schema;
// this helper never builds or runs a demonstration or profile workload.
export function assertAttackTriggerSchema(schema: any, catalog: any, legacy: any[]) {
  const validate = new Ajv2020({strict:false,formats:{uint32:{type:"number",validate:(n:number)=>Number.isInteger(n)&&n>=0&&n<=0xffff_ffff}}}).compile(schema);
  assert.equal(validate(catalog),true,JSON.stringify(validate.errors));
  assert.equal(catalog.schema_version,24);
  assert.equal(catalog.definitions.length,17);
  for (const c of legacy) assert.equal(validate(c),true,JSON.stringify(validate.errors));
  for (const version of [23,26]) assert.equal(validate({...catalog,schema_version:version}),false);
  for (const d of catalog.definitions) {
    assert.ok(d.capabilities.includes("attack_trigger_v1"));
    const old=structuredClone(catalog);old.schema_version=23;
    old.definitions=[structuredClone(d)];old.definitions[0].capabilities=old.definitions[0].capabilities.filter((c:string)=>c!=="attack_trigger_v1");
    assert.equal(validate(old),false,"IR23 must reject the condition independently of its capability label");
  }
  const source=catalog.definitions.find((d:any)=>d.triggered[0].condition.creature_attacked.attacker.subject==="source");
  const observer=catalog.definitions.find((d:any)=>d.triggered[0].condition.creature_attacked.attacker.subject==="filter");
  for (const [definition,mutate] of [
    [source,(a:any)=>{a.condition.creature_attacked.extra=true;}],
    [source,(a:any)=>{a.condition.creature_attacked.attacker.extra=true;}],
    [source,(a:any)=>{a.condition.creature_attacked.attacker={subject:"attacking_player"};}],
    [observer,(a:any)=>{a.condition.creature_attacked.attacker.controller="owner";}],
    [observer,(a:any)=>{a.condition.creature_attacked.attacker.types={predicate:"any",unknown:true};}],
    [observer,(a:any)=>{a.condition.creature_attacked.attacker.types={predicate:"all",terms:[]};}],
    [observer,(a:any)=>{a.condition.creature_attacked.attacker.types={predicate:"any_of",terms:Array.from({length:33},()=>({predicate:"any"}))};}],
    [observer,(a:any)=>{a.condition.creature_attacked.attacker.types={predicate:"type",kind:"unknown"};}],
    [observer,(a:any)=>{a.condition.creature_attacked.attacker.status="attacking";}],
  ] as [any,(a:any)=>void][]) {
    const bad=structuredClone(catalog);bad.definitions=[structuredClone(definition)];mutate(bad.definitions[0].triggered[0]);
    assert.equal(validate(bad),false,JSON.stringify(bad));
  }
}
