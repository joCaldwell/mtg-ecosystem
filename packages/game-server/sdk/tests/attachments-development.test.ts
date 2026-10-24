import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { attachmentsDevelopmentPolicy, attachmentsSpend } from '../src/examples/attachments-development-policy.ts';
import { command, stream } from '../src/validation.ts';
const catalog = JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-attachments-ir.json', import.meta.url), 'utf8'));
const def = name => catalog.definitions.find(d => d.name === name) ?? assert.fail(name);
const manifest = () => ({protocol_version:1,match_id:'abc',engine_build_id:'b'.repeat(64),catalog_sha256:'56affddedfe7b2482ad48238b95b3f8c30bfc39c701125cff883fa6ea9c404df',profile:'commander-attachments-v1',decks:['Isamaru, Hound of Konda','Yargle and Multani','Caelorna, Coral Tyrant','Rorix Bladewing'].map(name=>({commander:def(name).oracle_id,cards:[]})),starting_player:0,catalog});
const card = (name, handle=name, extra={}) => ({oracle_id:def(name).oracle_id,handle,owner:0,controller:0,zone:{zone:'battlefield'},tapped:false,attack_ready:true,marked_damage:'0',characteristics:structuredClone(def(name).characteristics),...extra});
const obs = (request, fields={}) => ({observation_version:16,decision:{request,player:0,token:'private-token',...fields},view:{viewer:0,active:0,priority:0,turn_owner:0,turn:{number:2,step:'precombat_main',lands_played:1},live_turn_order:[0,1,2,3],commanders:[{designation:'public',oracle_id:def('Isamaru, Hound of Konda').oracle_id,history:{owner:0,color_identity:['W'],command_casts:{},combat_damage:{}}}],objects:[card('Isamaru, Hound of Konda','own'),card('Yargle and Multani','enemy',{controller:1,owner:1})],players:Array.from({length:4},(_,player)=>({player,status:'live',life:'40',hand_count:0,library_count:90,mana:{}})),stack:[],combat:[]}});
const occurrence = {domain:'battlefield_creature',chooser:'controller',selection_id:'target',slot:0,stage:'announcement'};
function choose(o, policy=attachmentsDevelopmentPolicy(), m=manifest()) {
 const before=JSON.stringify({o,m});
 stream({protocol_version:1,entries:[{cursor:'1',publication:{kind:'view_replaced',observation:o}}],next:'1',has_more:false},'0');
 const response=policy(o,0,m);
 if(response) command({protocol_version:1,command_id:'1'.repeat(32),expected_revision:'1',command:{kind:'decision',token:o.decision.token,response}});
 assert.equal(JSON.stringify({o,m}),before,'no DTO mutation');
 return response;
}
test('attachments policy handles every generated request family with schema-valid responses',()=>{
 const requests = [
 ['mana_color',{colors:['W','C']}], ['search_lands',{maximum:1,candidates:[{handle:'private-search',oracle_id:def('Plains').oracle_id}]}],
 ['optional_recursion',{}], ['mulligan',{mulligans_taken:0,bottom_if_mulligan:0,can_mulligan:true}],
 ['bottom_opening',{count:1,candidates:['own','enemy']}],
 ['damage_allocation',{recipient:{entity:'player',player:0},effect:'shield',remaining:'3',required:'3',candidates:[{damage:'hit',amount:'3',source:{announced:'own',current:'own'},oracle_id:def('Isamaru, Hound of Konda').oracle_id}]}],
 ['modes',{candidates:[0]}], ['damage_effect',{recipient:{entity:'player',player:0},candidates:[{damage:'hit',effect:'shield',kind:'prevent_next_damage',amount_before:'3',amount_after:'0',budget_remaining:'3',damage_source:{announced:'own',current:'own'},damage_oracle_id:def('Isamaru, Hound of Konda').oracle_id,effect_source:{announced:'own',current:'own'},effect_oracle_id:def('Healing Salve').oracle_id,effect_controller:0}]}],
 ['order_triggers',{candidates:[{handle:'trigger',source:{announced:'own',current:'own'},oracle_id:def('Rancor').oracle_id,ability:0}]}],
 ['entity_targets',{occurrences:[{...occurrence,domain:'any_damage_target'}],candidates:[{entity:'player',player:1}]}],
 ['stack_targets',{occurrences:[{...occurrence,domain:'stack_spell'}],candidates:['stack']}],
 ['player_targets',{occurrences:[{...occurrence,domain:'opponent_player'}],candidates:[1]}],
 ['sacrifice',{candidates:['own']}], ['resolve_discard',{count:1,candidates:['own']}],
 ['spend_mana',{cost:{generic:'1',colored:['W']},available:{W:'1',C:'1'},mana_ability_source:'rock'}],
 ['commander_return',{object:null,designation:'public',destination:{zone:'hand',owner:0}}],
 ['attackers',{candidates:['own'],defenders:[1,2,3]}],
 ['blockers',{candidates:['own'],attackers:['enemy'],legal_attackers:[{blocker:'own',attackers:['enemy']}]}],
 ['damage',{source:'own',power:'3',recipients:['enemy']}], ['discard',{count:1,candidates:['own']}],
 ['priority',{}], ['targets',{occurrences:[occurrence],candidates:['enemy']}], ['mana',{cost:{generic:'0',colored:[]},mana_ability_source:'rock'}],
 ];
 const schema=JSON.parse(fs.readFileSync(new URL('../generated/protocol.schema.json',import.meta.url),'utf8'));
 // These families belong to later catalogs and must fail closed for the
 // attachments-pinned policy, while still participating in the exact gate.
 const rejected = [['optional_effect',{}], ['sacrifice_cost',{candidates:['own']}], ['forced_sacrifice',{candidates:['own'],previous_choices:[]}]];
 assert.deepEqual([...requests,...rejected].map(([name])=>name).sort(),schema.stream.definitions.PlayerRequest.oneOf.map(r=>r.properties.request.const).sort());
 for(const [name,fields]of requests) assert(choose(obs(name,fields)),name);
 for(const [name,fields]of rejected) assert.throws(()=>choose(obs(name,fields)),/unsupported/,name);
 assert.deepEqual(choose(obs('damage',{source:'own',power:'9007199254740993',recipients:['enemy'],player_recipient:1,lethal_requirements:[['enemy','3']]})),{response:'trample_damage',assignments:[['enemy','3']],player:1,amount:'9007199254740990'});
});
test('equipment prefers commander, respects shroud, and never cycles zero-cost Greaves',()=>{
 const o=obs('priority');o.view.objects.push(card('Lightning Greaves'));o.view.objects[1].controller=0;
 assert.deepEqual(choose(o),{response:'activate_ability',object:'Lightning Greaves',ability:0});
 o.view.objects[2].attached_to='own';o.view.objects[0].keywords=['shroud'];
 assert.deepEqual(choose(o),{response:'pass'});
 o.view.objects[2].attached_to=undefined;o.view.objects[0].keywords=[];
 const t=obs('targets',{occurrences:[occurrence],candidates:['own','enemy']});
 t.view.objects.push(card('Lightning Greaves'));t.view.objects[1].controller=0;
 t.view.stack=[{spell:null,controller:0,cast_complete:false,targets:[],ability:{oracle_id:def('Lightning Greaves').oracle_id,ability:0,handle:'equip',source:{announced:'Lightning Greaves',current:'Lightning Greaves'}}}];
 assert.deepEqual(choose(t),{response:'targets',objects:['own']});
});
test('Pacifism and beneficial Auras choose the proper highest-power offered side',()=>{
 for(const [name,target]of [['Pacifism','enemy'],['Holy Strength','own']]) {
 const o=obs('targets',{occurrences:[occurrence],candidates:['own','enemy']});
 o.view.objects.push(card(name,'aura',{zone:{zone:'stack'}}));
 o.view.stack=[{spell:'aura',controller:0,cast_complete:false,targets:[]}];
 assert.deepEqual(choose(o),{response:'targets',objects:[target]});
 }
});
test('block legality hints, menace pairs and public granted keywords determine blocks',()=>{
 const o=obs('blockers',{candidates:['own','second'],attackers:['enemy'],menace_attackers:['enemy'],legal_attackers:[{blocker:'own',attackers:['enemy']},{blocker:'second',attackers:['enemy']}]});
 o.view.objects[0].keywords=['deathtouch'];o.view.objects.push(card('Yoked Ox','second'));
 assert.deepEqual(choose(o),{response:'blockers',blocks:[['own','enemy'],['second','enemy']]});
 o.decision.legal_attackers.pop();assert.deepEqual(choose(o),{response:'blockers',blocks:[]});
 o.decision.menace_attackers=[];o.decision.legal_attackers=[];assert.deepEqual(choose(o),{response:'blockers',blocks:[]});
});
test('nested Signet mana and exact multicolor spending use only own public sources',()=>{
 const p=attachmentsDevelopmentPolicy(), m=manifest();m.decks[0].commander=def('Yargle and Multani').oracle_id;
 const o=obs('mana',{cost:{generic:'0',colored:['B','G']}});
 o.view.objects=[card('Golgari Signet','signet'),card('Plains','land')];
 assert.deepEqual(choose(o,p,m),{response:'activate_mana',object:'land'});
 o.view.objects[1].tapped=true;o.view.players[0].mana.W='1';
 assert.deepEqual(choose(o,p,m),{response:'activate_mana',object:'signet'});
 o.decision={request:'mana',player:0,token:'inner',cost:{generic:'1',colored:[]},mana_ability_source:'signet'};
 assert.deepEqual(choose(o,p,m),{response:'finish_mana',undo_mana:true});
 o.decision={request:'spend_mana',player:0,token:'inner-spend',cost:{generic:'1',colored:[]},available:{W:'1'},mana_ability_source:'signet'};
 assert.deepEqual(choose(o,p,m),{response:'spend_mana',spending:{W:'1'}});
 assert.deepEqual(attachmentsSpend({generic:'9007199254740993',colored:['B','G']},{B:'1',G:'1',C:'9007199254740993'}),{B:'1',G:'1',C:'9007199254740993'});
});
test('any-color mana, haste readiness, token rows, commander tax and seat isolation',()=>{
 const p=attachmentsDevelopmentPolicy(), o=obs('mana',{cost:{generic:'0',colored:['U']}});
 o.view.objects.push(card('Birds of Paradise','bird',{attack_ready:false,keywords:['haste']}));
 assert.deepEqual(choose(o,p),{response:'activate_mana',object:'bird'});
 o.decision={request:'mana_color',player:0,token:'color',colors:['W','U','B','R','G'],mana_ability_source:'bird'};
 assert.deepEqual(choose(o,p),{response:'mana_color',color:'U'});
 const taxed=obs('priority');taxed.view.objects=[card('Isamaru, Hound of Konda','commander',{zone:{zone:'command',owner:0}})];taxed.view.commanders[0].history.command_casts={'0':'9007199254740993'};taxed.view.players[0].mana.W='18014398509481987';
 assert.deepEqual(choose(taxed),{response:'cast',card:'commander'});
 const token=card('Yoked Ox','token',{token:{name:'Saproling',keywords:[]},oracle_id:''});o.view.objects=[token];o.decision={request:'priority',player:0,token:'priority'};assert.deepEqual(choose(o),{response:'pass'});
 o.view.viewer=1;assert.throws(()=>attachmentsDevelopmentPolicy()(o,0,manifest()),/unsupported/);
});
