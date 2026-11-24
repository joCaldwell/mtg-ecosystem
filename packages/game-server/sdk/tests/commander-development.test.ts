import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { commanderDevelopmentPolicy as policy } from '../src/examples/commander-development-policy.ts';
import { runFoundationsConsumer } from '../src/examples/foundations-consumer.ts';
import { command, stream } from '../src/validation.ts';

const catalog=JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json',import.meta.url),'utf8'));
const definition=name=>{const d=catalog.definitions.find(row=>row.name===name);assert(d);return d;};
const commander=definition('Isamaru, Hound of Konda'), plains=definition('Plains');
const deck = JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/profiles/commander-foundations-v1.json', import.meta.url), 'utf8')).decks[0];
const manifest=()=>({protocol_version:2,match_id:'abc',engine_build_id:'b'.repeat(64),server_build_id:'c'.repeat(64),catalog_sha256:fs.readFileSync(new URL('../../../game-engine/tests/fixtures/catalogs/match.sha256', import.meta.url), 'utf8').trim(),profile:'commander-foundations-v1',decks:Array.from({length:4},()=>structuredClone(deck)),starting_player:0,catalog:structuredClone(catalog)});
const object=(name,handle,zone,extra={})=>({oracle_id:definition(name).oracle_id,handle,owner:0,controller:0,zone,tapped:false,attack_ready:true,marked_damage:'0',characteristics:structuredClone(definition(name).characteristics),...extra});
const land=(handle,zone={zone:'battlefield'},extra={})=>object('Plains',handle,zone,extra);
const dog=(zone={zone:'command',owner:0},extra={})=>object('Isamaru, Hound of Konda','commander-handle',zone,extra);
const slot=(allowed,min=1,max=min,subject=null,role='choice')=>({role:{role},subject,allowed,min,max,repeat:false});
const decision=(reason,candidates,shape)=>({player:0,token:'viewer-decision-token',reason,candidates,shape});
const pick=(reason,candidates,slots)=>decision(reason,candidates,{shape:'pick',slots,rules:[]});
const priority=(...actions)=>pick({reason:'priority'},[{action:'pass'},...actions].map(action=>({candidate:'action',action})),[slot(Array.from({length:actions.length+1},(_,i)=>i))]);
const pay=(step,cost,available={},sources=[])=>decision({reason:step==='produce'?'mana':'spend_mana',source:null},sources.map(object=>({candidate:'action',action:{action:'activate_mana',object}})),{shape:'pay',cost,pool:{unrestricted:available},step:step==='produce'?{step,sources:sources.map((_,i)=>i)}:{step}});
const expectedPick=(index)=>({answer:'pick',picks:[[index]]});
const observation=(decision=priority())=>({observation_version:57,decision,view:{viewer:0,active:0,priority:0,turn_owner:0,turn:{number:2,step:'precombat_main',lands_played:1},live_turn_order:[0,1,2,3],commanders:[{designation:'public-commander',oracle_id:commander.oracle_id,history:{owner:0,color_identity:commander.characteristics.color_identity,combat_damage:{},command_casts:{}}}],objects:[],players:Array.from({length:4},(_,player)=>({player,status:'live',hand_count:0,library_count:90,life:'40',mana:{}})),stack:[],combat:[]}});
function choose(o,m=manifest()){
 const before=JSON.stringify({o,m});
 stream({protocol_version:2,entries:[{cursor:'1',publication:{kind:'view_replaced',observation:o}}],next:'1',has_more:false},'0');
 const response=policy(o,0,m);
 if(response!==null)command({protocol_version:2,command_id:'0'.repeat(31)+'1',expected_revision:'1',command:{kind:'decision',token:o.decision.token,answer:response}});
 assert.equal(JSON.stringify({o,m}),before,'policy is pure and does not reorder/mutate DTOs');
 return response;
}

test('priority chooses an offered own Plains, then an affordable commander, respecting tax, timing and exact quantities',()=>{
 const o=observation(priority({action:'play_land',object:'land'},{action:'cast',card:'commander-handle'}));
 o.view.turn.lands_played=0;o.view.objects=[dog(),land('land',{zone:'hand',owner:0})];
 assert.deepEqual(choose(o),expectedPick(1));
 o.view.turn.lands_played=1;o.view.objects=[dog(),land('land')];
 assert.deepEqual(choose(o),expectedPick(2));
 o.view.turn.step='upkeep';assert.deepEqual(choose(o),expectedPick(0));o.view.turn.step='precombat_main';
 o.view.commanders[0].history.command_casts={'0':'9007199254740993'};
 assert.deepEqual(choose(o),expectedPick(0));
 o.view.players[0].mana.W='18014398509481987';assert.deepEqual(choose(o),expectedPick(2));
});
test('payment uses offered sources, supports reversal, and preserves exact white spending',()=>{
 const cost=commander.characteristics.mana_cost;
 const o=observation(pay('produce',cost,{},['foreign','land']));
 o.view.objects=[land('foreign',{zone:'battlefield'},{controller:1}),land('land')];
 assert.deepEqual(choose(o),{answer:'pay',pay:{step:'activate',source:1}});
 o.view.players[0].mana.W='1';assert.deepEqual(choose(o),{answer:'pay',pay:{step:'finish',undo_on_failure:true}});
 o.view.players[0].mana={};o.view.objects=[];
 assert.deepEqual(choose(o),{answer:'pay',pay:{step:'finish',undo_on_failure:true}});
 const huge='9007199254740995';
 assert.deepEqual(choose(observation(pay('spend',{generic:huge,colored:[]},{W:huge}))),{answer:'pay',pay:{step:'spend',unrestricted:{W:huge}}});
});
const attacks=()=>pick({reason:'attackers'},[{candidate:'object',object:'commander-handle'},...([3,2].map(player=>({candidate:'player',player})))],[slot([1,2],0,1,0,'attack_with')]);
test('uses supplied combat slots and ordered opening cards',()=>{
 const o=observation(attacks());o.view.objects=[dog({zone:'battlefield'})];
 assert.deepEqual(choose(o),expectedPick(2));
 o.view.objects[0].attack_ready=false;assert.deepEqual(choose(o),{answer:'pick',picks:[[]]});
 assert.deepEqual(choose(observation(decision({reason:'bottom_opening'},[{candidate:'object',object:'a'}],{shape:'order',items:[0],min:1,max:1}))),{answer:'order',order:[0]});
 assert.deepEqual(choose(observation(pick({reason:'mulligan',mulligans_taken:0,bottom_if_mulligan:0},[{candidate:'flag',flag:'keep'}],[slot([0])]))),expectedPick(0));
 const bad=manifest();bad.decks[2].commander='different';assert.throws(()=>choose(observation(),bad),/commander_policy_unsupported/);
});
test('callback retains private bindings and sends the six indexed decisions once through GameClient',async()=>{
 const states=[];
 const p=observation(priority({action:'play_land',object:'plains'}));p.view.turn.lands_played=0;p.view.objects=[dog(),land('plains',{zone:'hand',owner:0})];states.push(p);
 const c=observation(priority({action:'cast',card:'commander-handle'}));c.view.objects=[dog(),land('plains')];states.push(c);
 const m=observation(pay('produce',commander.characteristics.mana_cost,{},['plains']));m.view.objects=[land('plains')];states.push(m);
 const f=structuredClone(m);f.view.players[0].mana.W='1';f.view.objects[0].tapped=true;states.push(f);
 states.push(observation(pay('spend',commander.characteristics.mana_cost,{W:'1'})));
 const a=observation(attacks());a.view.objects=[dog({zone:'battlefield'})];states.push(a);
 states.forEach((o,i)=>o.decision.token=`private-recipe-token-${i}`);
 const sent=[];let index=0;const json=value=>new Response(JSON.stringify(value),{headers:{'content-type':'application/json'}});
 const report=await runFoundationsConsumer({baseURL:'http://127.0.0.1:3000',matchId:'abc',viewer:0,capability:'c'.repeat(64)},{policy,fetch:async(address,options)=>{
  if(address.endsWith('/manifest'))return json(manifest());
  if(address.endsWith('/commands')){const body=JSON.parse(options.body);sent.push(body);return json({status:'accepted',command_id:body.command_id});}
  const cursor=String(++index);return json({protocol_version:2,entries:[{cursor,publication:index<=states.length?{kind:'view_replaced',observation:states[index-1]}:{kind:'outcome',result:{outcome:'win',player:0}}}],next:cursor,has_more:false});
 }});
 assert.equal(report.status,'completed');assert.equal(report.counts.accepted_inputs,6);
 assert.deepEqual(sent.map(row=>row.command.answer),[expectedPick(1),expectedPick(1),{answer:'pay',pay:{step:'activate',source:0}},{answer:'pay',pay:{step:'finish',undo_on_failure:true}},{answer:'pay',pay:{step:'spend',unrestricted:{W:'1'}}},expectedPick(2)]);
 assert(!JSON.stringify(report).includes('private-recipe-token'));
});
