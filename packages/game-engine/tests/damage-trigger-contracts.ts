import assert from "node:assert/strict";
import Ajv2020 from "ajv/dist/2020.js";

// Schema-only assertions; caller supplies current executable schema and a
// compiler-produced catalog (or an explicitly synthetic contract-shape witness).
// This helper does not build, execute games, or create expected IR.
export function assertDamageTriggerSchema(schema: any, catalog: any, legacy: any[]) {
  const validate = new Ajv2020({strict:false,formats:{uint32:{type:"number",validate:(n:number)=>Number.isInteger(n)&&n>=0&&n<=0xffff_ffff}}}).compile(schema);
  assert.equal(validate(catalog),true,JSON.stringify(validate.errors));
  assert.equal(catalog.schema_version,25);
  for (const c of legacy) assert.equal(validate(c),true,JSON.stringify(validate.errors));
  for (const version of [24,34]) assert.equal(validate({...catalog,schema_version:version}),false);
  for (const d of catalog.definitions) {
    assert.ok(d.capabilities.includes("damage_trigger_v1"));
    const old=structuredClone(catalog);old.schema_version=24;old.definitions=[structuredClone(d)];
    old.definitions[0].capabilities=old.definitions[0].capabilities.filter((c:string)=>c!=="damage_trigger_v1");
    assert.equal(validate(old),false,"IR24 rejects the condition independently of the capability label");
  }
  for (const mutate of [
    (c:any)=>{c.extra=true;},
    (c:any)=>{c.source.extra=true;},
    (c:any)=>{c.source={subject:"filter",controller:"you"};},
    (c:any)=>{c.kind="noncombat";},
    (c:any)=>{c.kind=true;},
    (c:any)=>{delete c.kind;},
    (c:any)=>{c.recipient="opponent";},
    (c:any)=>{c.recipient={player:"any"};},
    (c:any)=>{delete c.source;},
  ]) {
    const bad=structuredClone(catalog);bad.definitions=[bad.definitions[0]];
    mutate(bad.definitions[0].triggered[0].condition.damage_dealt);
    assert.equal(validate(bad),false,JSON.stringify(bad));
  }
}
