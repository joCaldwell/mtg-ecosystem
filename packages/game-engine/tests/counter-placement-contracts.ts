// Builtin-only assertions: caller supplies Ajv, own exported schema and complete-source catalog.
import assert from 'node:assert/strict';
export function assertCounterPlacementSchema(validate: any, catalog: any): {positives: number, negatives: number} {
  let positives=0, negatives=0;
  const good = (c: any) => {assert.equal(validate(c),true,JSON.stringify(validate.errors)); positives++;};
  const bad = (c: any) => {assert.equal(validate(c),false,'invalid counter replacement IR admitted'); negatives++;};
  good(catalog);
  const capability='controller_creature_plus_one_counter_replacement_v1';
  const find=(c: any,name: string)=>c.definitions.find((d: any)=>d.name===name);
  const replacement=(d: any)=>d.static_abilities[0].effect.controller_creature_plus_one_counter_replacement;
  const scales=find(catalog,'Hardened Scales'),jack=find(catalog,'Corpsejack Menace'),ramos=find(catalog,'Ramos, Dragon Engine');
  assert.ok(scales&&jack&&ramos);
  assert.deepEqual(replacement(scales),{modification:{operation:'add',amount:'1'}});
  assert.deepEqual(replacement(jack),{modification:{operation:'multiply',factor:'2'}});
  assert.deepEqual(ramos.triggered[0].condition,{spell_cast:{caster:'controller',spell:{predicate:'any'}}});
  assert.equal(jack.characteristics.power,'4');assert.equal(jack.characteristics.toughness,'4');
  for (const name of ['Hardened Scales','Corpsejack Menace']) {
    for (const amount of ['1','3','7','10000000000000000000000000000000000000']) {
      const c=structuredClone(catalog),d=find(c,name),m=replacement(d).modification;
      if(m.operation==='add')m.amount=amount;else m.factor=amount;
      good(c);
    }
    for (const mutation of [
      (d: any)=>{d.capabilities=d.capabilities.filter((c: string)=>c!==capability);},
      (d: any)=>{d.static_abilities[0].functioning_zone='stack';},
      (d: any)=>{d.characteristics.kind='land';d.characteristics.power=null;d.characteristics.toughness=null;},
      (d: any)=>{replacement(d).modification.operation='subtract';},
      (d: any)=>{const m=replacement(d).modification;m[m.operation==='add'?'amount':'factor']='0';},
      (d: any)=>{const m=replacement(d).modification;m[m.operation==='add'?'amount':'factor']='-1';},
      (d: any)=>{replacement(d).modification.extra=true;},
      (d: any)=>{replacement(d).modification.amount='1';replacement(d).modification.factor='2';},
      (d: any)=>{d.static_abilities[0].effect.controller_creature_plus_one_counter_replacement.extra=true;},
    ]) {const c=structuredClone(catalog);mutation(find(c,name));bad(c);}
  }
  // Ordinary source is unchanged and cannot claim the new family capability.
  const c=structuredClone(catalog);find(c,'Ramos, Dragon Engine').capabilities.push(capability);bad(c);
  const ordinary=structuredClone(catalog);ordinary.definitions=ordinary.definitions.filter((d: any)=>!['Hardened Scales','Corpsejack Menace'].includes(d.name));good(ordinary);
  // Preserve M1a's actual enum and null-Option serialization regression controls.
  assert.equal(ramos.mana_abilities[0].costs.mana,undefined);
  assert.equal(ramos.mana_abilities[0].restriction,'once_each_turn');
  const wrong=structuredClone(catalog);find(wrong,'Ramos, Dragon Engine').triggered[0].condition.spell_cast.spell={any:{}};bad(wrong);
  return {positives,negatives};
}
