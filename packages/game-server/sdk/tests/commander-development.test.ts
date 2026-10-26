import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { commanderDevelopmentPolicy as policy } from '../src/examples/commander-development-policy.ts';
import { runFoundationsConsumer } from '../src/examples/foundations-consumer.ts';
import { command, stream } from '../src/validation.ts';

const catalog=JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json',import.meta.url),'utf8'));
const definition=name=>{const d=catalog.definitions.find(row=>row.name===name);assert(d);return d;};
const commander=definition('Isamaru, Hound of Konda'), plains=definition('Plains');
const deck={commander:commander.oracle_id,cards:['Isamaru, Hound of Konda','Yoked Ox','Samite Healer','Sacred Nectar','Healing Hands','Healing Salve','Plains'].map(name=>({oracle_id:definition(name).oracle_id,count:name==='Plains'?94:1}))};
const manifest=()=>({protocol_version:1,match_id:'abc',engine_build_id:'b'.repeat(64),server_build_id:'c'.repeat(64),catalog_sha256:'fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f',profile:'commander-foundations-v1',decks:Array.from({length:4},()=>structuredClone(deck)),starting_player:0,catalog:structuredClone(catalog)});
const object=(name,handle,zone,extra={})=>({oracle_id:definition(name).oracle_id,handle,owner:0,controller:0,zone,tapped:false,attack_ready:true,marked_damage:'0',characteristics:structuredClone(definition(name).characteristics),...extra});
const land=(handle,zone={zone:'battlefield'},extra={})=>object('Plains',handle,zone,extra);
const dog=(zone={zone:'command',owner:0},extra={})=>object('Isamaru, Hound of Konda','commander-handle',zone,extra);
const observation=(request='priority',extra={})=>({observation_version:16,decision:{request,player:0,token:'viewer-decision-token',...extra},view:{viewer:0,active:0,priority:0,turn_owner:0,turn:{number:2,step:'precombat_main',lands_played:1},live_turn_order:[0,1,2,3],commanders:[{designation:'public-commander',oracle_id:commander.oracle_id,history:{owner:0,color_identity:commander.characteristics.color_identity,combat_damage:{},command_casts:{}}}],objects:[],players:Array.from({length:4},(_,player)=>({player,status:'live',hand_count:0,library_count:90,life:'40',mana:{}})),stack:[],combat:[]}});
function choose(o,m=manifest()){
 const before=JSON.stringify({o,m});
 stream({protocol_version:1,entries:[{cursor:'1',publication:{kind:'view_replaced',observation:o}}],next:'1',has_more:false},'0');
 const response=policy(o,0,m);
 if(response!==null)command({protocol_version:1,command_id:'0'.repeat(31)+'1',expected_revision:'1',command:{kind:'decision',token:o.decision.token,response}});
 assert.equal(JSON.stringify({o,m}),before,'policy is pure and does not reorder/mutate DTOs');
 return response;
}

test('source-backed priority plays only visible own Plains at an own empty-stack main phase, then funds commander',()=>{
 const o=observation();o.view.turn.lands_played=0;o.view.objects=[dog(),land('private-plains',{zone:'hand',owner:0})];
 assert.deepEqual(choose(o),{response:'play_land',object:'private-plains'});
 o.view.turn.lands_played=1;o.view.objects=[dog(),land('untapped-plains')];assert.deepEqual(choose(o),{response:'cast',card:'commander-handle'});
 for(const [key,value]of[['active',1],['priority',1]]){const other=structuredClone(o);other.view[key]=value;assert.deepEqual(choose(other),{response:'pass'});}
 const other=structuredClone(o);other.view.turn.step='upkeep';assert.deepEqual(choose(other),{response:'pass'});
 o.view.objects=[object('Yoked Ox','ignored-optional-spell',{zone:'hand',owner:0})];assert.deepEqual(choose(o),{response:'pass'});
});

test('public per-caster commander history controls exact tax and insufficient funding never announces a cast',()=>{
 const o=observation();o.view.commanders[0].history.command_casts={'0':'1','2':'9007199254740993'};o.view.objects=[dog(),land('p1'),land('p2')];
 assert.deepEqual(choose(o),{response:'pass'});o.view.objects.push(land('p3'));assert.deepEqual(choose(o),{response:'cast',card:'commander-handle'});
 o.view.objects[3].tapped=true;assert.deepEqual(choose(o),{response:'pass'});
 o.view.players[0].mana={W:'1'};assert.deepEqual(choose(o),{response:'cast',card:'commander-handle'});
});

test('mana activates only controlled untapped public Plains and finishes affordable or externally unfunded proposals',()=>{
 const o=observation('mana',{cost:commander.characteristics.mana_cost});
 o.view.objects=[land('foreign',{zone:'battlefield'},{controller:1}),land('tapped',{zone:'battlefield'},{tapped:true}),object('Forest','wrong-color',{zone:'battlefield'}),land('owned-land')];
 assert.deepEqual(choose(o),{response:'activate_mana',object:'owned-land'});
 o.view.players[0].mana.W='1';assert.deepEqual(choose(o),{response:'finish_mana',undo_mana:true});
 o.view.players[0].mana={};o.view.objects=[];assert.deepEqual(choose(o),{response:'finish_mana',undo_mana:true});
});

test('spending and very large public cast histories stay exact decimal BigInt quantities',()=>{
 const o=observation('spend_mana',{cost:{generic:'18014398509481986',colored:['W']},available:{W:'18014398509481987'}});
 assert.deepEqual(choose(o),{response:'spend_mana',spending:{W:'18014398509481987'}});
 o.decision.available.W='18014398509481986';assert.throws(()=>choose(o),/commander_policy_unsupported/);
 const p=observation();p.view.objects=[dog()];p.view.commanders[0].history.command_casts={'0':'9007199254740993'};p.view.players[0].mana.W='18014398509481987';assert.deepEqual(choose(p),{response:'cast',card:'commander-handle'});
 p.view.players[0].mana.W='18014398509481986';assert.deepEqual(choose(p),{response:'pass'});
});

test('offered ready positive-power commander attacks next living opponent; no invented attacker or block',()=>{
 const o=observation('attackers',{candidates:['commander-handle','optional-creature','missing'],defenders:[3,1,2]});o.view.live_turn_order=[0,2,3];o.view.objects=[dog({zone:'battlefield'}),object('Yoked Ox','optional-creature',{zone:'battlefield'})];
 assert.deepEqual(choose(o),{response:'attackers',attacks:[['commander-handle',2]]});
 o.view.objects[0].attack_ready=false;assert.deepEqual(choose(o),{response:'attackers',attacks:[]});
 o.view.objects[0].attack_ready=true;o.view.objects[0].characteristics.power='-1';assert.deepEqual(choose(o),{response:'attackers',attacks:[]});
 assert.deepEqual(choose(observation('blockers',{candidates:['commander-handle'],attackers:['enemy']})),{response:'blockers',blocks:[]});
});

test('mandatory setup, commander return and cleanup use offered values and unsupported mechanics fail closed',()=>{
 assert.deepEqual(choose(observation('mulligan',{mulligans_taken:0,bottom_if_mulligan:0,can_mulligan:true})),{response:'mulligan',take:false});
 assert.deepEqual(choose(observation('bottom_opening',{count:1,candidates:['a','b']})),{response:'bottom_opening',objects:['a']});
 assert.deepEqual(choose(observation('commander_return',{designation:'public',object:null,destination:{zone:'hand',owner:0}})),{response:'commander_return',to_command:true});
 assert.deepEqual(choose(observation('discard',{count:2,candidates:['a','b','c']})),{response:'discard',objects:['a','b']});
 assert.throws(()=>choose(observation('discard',{count:3,candidates:['a']})),/commander_policy_unsupported/);
 assert.throws(()=>choose(observation('mana',{cost:{generic:'0',colored:['U']}})),/commander_policy_unsupported/);
 assert.throws(()=>choose(observation('legend_rule',{candidates:['a','b'],previous_choices:[]})),/commander_policy_unsupported/);
 assert.throws(()=>choose(observation('targets',{candidates:['a'],occurrences:[]})),/commander_policy_unsupported/);
 const bad=observation();bad.view.viewer=1;assert.throws(()=>policy(bad,0,manifest()),/commander_policy_unsupported/);
 const mixed=manifest();mixed.decks[1].commander=definition('Yargle and Multani').oracle_id;assert.throws(()=>policy(observation(),0,mixed),/commander_policy_unsupported/);
 const none=observation();none.decision=null;assert.equal(policy(none,0,manifest()),null);
});

test('callback reuses GameClient transport and passes only pinned public manifest and typed observations through a full command recipe',async()=>{
 const states=[];
 const p=observation();p.view.turn.lands_played=0;p.view.objects=[dog(),land('plains',{zone:'hand',owner:0})];states.push(p);
 const c=observation();c.view.objects=[dog(),land('plains')];states.push(c);
 const m=observation('mana',{cost:commander.characteristics.mana_cost});m.view.objects=[land('plains')];states.push(m);
 const f=structuredClone(m);f.view.players[0].mana.W='1';f.view.objects[0].tapped=true;states.push(f);
 states.push(observation('spend_mana',{cost:commander.characteristics.mana_cost,available:{W:'1'}}));
 const a=observation('attackers',{candidates:['commander-handle'],defenders:[1,2,3]});a.view.objects=[dog({zone:'battlefield'})];states.push(a);
 states.forEach((o,i)=>o.decision.token=`private-recipe-token-${i}`);
 const sent=[];let index=0;const json=value=>new Response(JSON.stringify(value),{headers:{'content-type':'application/json'}});
 const report=await runFoundationsConsumer({baseURL:'http://127.0.0.1:3000',matchId:'abc',viewer:0,capability:'c'.repeat(64)},{policy,fetch:async(address,options)=>{
  if(address.endsWith('/manifest'))return json(manifest());
  if(address.endsWith('/commands')){const body=JSON.parse(options.body);sent.push(body);return json({status:'accepted',command_id:body.command_id});}
  const cursor=String(++index);return json({protocol_version:1,entries:[{cursor,publication:index<=states.length?{kind:'view_replaced',observation:states[index-1]}:{kind:'outcome',result:{outcome:'win',player:0}}}],next:cursor,has_more:false});
 }});
 assert.equal(report.status,'completed');assert.equal(report.counts.accepted_inputs,6);
 assert.deepEqual(sent.map(row=>row.command.response.response),['play_land','cast','activate_mana','finish_mana','spend_mana','attackers']);
 assert(!JSON.stringify(report).includes('private-recipe-token'));
});
