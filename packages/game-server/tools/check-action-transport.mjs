/** Explicit real-host probe. All children use supplied retained executables;
 * no Cargo, remote data, SDK retries, credential logs or private-state exports. */
import fs from 'node:fs';
import path from 'node:path';
import {spawn,spawnSync} from 'node:child_process';
import {createServer} from 'node:net';
import {once} from 'node:events';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {GameClient,parseSeatBootstrap,prepareCommand,TransportError,HttpError} from '../sdk/dist/browser.js';
const [binary,provisioner,nativeInfoPath,sourceManifest,output]=process.argv.slice(2);
assert(binary&&provisioner&&nativeInfoPath&&sourceManifest&&output&&!fs.existsSync(output),'usage: check-action-transport.mjs BINARY PROVISIONER NATIVE_INFO SOURCE_MANIFEST NEW_OUTPUT_DIR');
for(const f of [binary,provisioner,nativeInfoPath,sourceManifest])assert(path.isAbsolute(f));
fs.mkdirSync(output,{mode:0o700});
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');const nativeInfo=JSON.parse(fs.readFileSync(nativeInfoPath));
const listener=createServer();listener.listen(0,'127.0.0.1');await once(listener,'listening');const port=listener.address().port;await new Promise(resolve=>listener.close(resolve));
const origin=`http://127.0.0.1:${port}`;const registry=path.join(output,'registry'),privateDir=path.join(output,'private');
const provision=spawnSync(provisioner,[registry,privateDir,origin],{encoding:'utf8'});assert.equal(provision.status,0,'producer provision failed (details private)');
const seats=Array.from({length:4},(_,s)=>parseSeatBootstrap(fs.readFileSync(path.join(privateDir,`seat-${s}.json`),'utf8')));
for(const s of seats){assert.equal(s.engine_build_id,nativeInfo.engine_build_id);assert.equal(s.server_build_id,nativeInfo.server_build_id);}
const clients=seats.map(s=>new GameClient({baseURL:origin,matchId:s.match_id,capability:s.capability}));
let host;const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));const cases=[];const publicQueries=[];const commands=[];
const log=fs.openSync(path.join(output,'host.log'),'wx',0o600);
async function launch(){host=spawn(binary,['serve',registry,`127.0.0.1:${port}`],{stdio:['ignore',log,log]});for(let i=0;i<400;i++){if(host.exitCode!==null)throw new Error('host exited');try{await clients[0].resume();return;}catch(error){if(!(error instanceof TransportError || error instanceof HttpError))throw error;}await delay(50);}throw new Error('host readiness bound');}
async function stop(){if(!host||host.exitCode!==null)return;host.kill('SIGINT');const watchdog=setTimeout(()=>host.kill('SIGKILL'),10000);try{await once(host,'exit');}finally{clearTimeout(watchdog);}}
async function offered(){for(let n=0;n<100;n++){const resumes=await Promise.all(clients.map(c=>c.resume()));for(let s=0;s<4;s++){const r=resumes[s];if(r.state.status==='available'&&r.state.observation.decision)return {seat:s,resume:r,decision:r.state.observation.decision};}await delay(20);}throw new Error('no offered decision within bound');}
const query=(s,body)=>clients[s].queryActions(body);const start=(token,work=1)=>({operation:'start',token,scope:{scope:'window'},page_size:1,work});
async function raw(seat,body,extra='',auth=true){const r=await fetch(`${origin}/v1/matches/a1/actions/query${extra}`,{method:'POST',headers:{'Content-Type':'application/json',...(auth?{Authorization:`Bearer ${seats[seat].capability}`}:{})},body});const text=await r.text();assert.equal(r.headers.get('cache-control'),'no-store');return {status:r.status,text};}
const watchdog=setTimeout(()=>{host?.kill('SIGKILL');process.exitCode=1;},180000);
try{
 await launch();
 // Real accepted command with intentionally dropped completed HTTP response.
 for(let i=0;i<4;i++){
  const {seat,resume,decision}=await offered();assert.equal(decision.request,'mulligan');
  const prepared=prepareCommand({protocol_version:1,command_id:(i+1).toString(16).padStart(32,'0'),expected_revision:resume.next,command:{kind:'decision',token:decision.token,response:{response:'mulligan',take:false}}});
  let calls=0;
  if(i===0){const lost=new GameClient({baseURL:origin,matchId:'a1',capability:seats[seat].capability,fetch:async(url,init)=>{calls++;const response=await fetch(url,init);assert.equal(response.status,200);await response.arrayBuffer();throw new Error('injected loss after real commit');}});await assert.rejects(()=>lost.submitExact(prepared.exactBody),TransportError);assert.equal(calls,1);}
  else assert.equal((await clients[seat].submitExact(prepared.exactBody)).status,'accepted');
  let execution;for(let n=0;n<100;n++){execution=await clients[seat].execution(prepared.envelope.command_id);if(execution.status.status==='consumed_by_rules')break;await delay(20);}assert.equal(execution.status.status,'consumed_by_rules');
  const repeated=await clients[seat].submitExact(prepared.exactBody);assert.equal(repeated.status,'accepted');assert.equal((await clients[seat].execution(prepared.envelope.command_id)).status.status,'consumed_by_rules');
  commands.push({viewer:seat,command_id:prepared.envelope.command_id,body_sha256:sha(prepared.exactBody),injected_response_loss:i===0,exact_retry_status:repeated.status,execution:execution.status.status});
 }
 cases.push('real submissions and durable execution reconciliation after injected response loss; exact duplicate remains accepted');
 const {seat,resume,decision}=await offered();assert.equal(decision.request,'priority');const other=(seat+1)%4;
 const first=await query(seat,start(decision.token));assert.equal(first.viewer,seat);assert.equal(first.outcome.status,'incomplete');publicQueries.push(first);
 const cursor=first.outcome.cursor;
 assert.equal((await query(other,{operation:'continue',cursor,work:1})).outcome.status,'stale');
 assert.equal((await query(other,{operation:'cancel',cursor})).outcome.status,'stale');
 const next=await query(seat,{operation:'continue',cursor,work:100});assert.equal(next.outcome.status,'incomplete');assert.notEqual(next.outcome.cursor,cursor);publicQueries.push(next);
 assert.equal((await query(seat,{operation:'continue',cursor,work:1})).outcome.status,'stale');
 assert.equal((await query(seat,{operation:'cancel',cursor:next.outcome.cursor})).outcome.status,'cancelled');
 assert.equal((await query(seat,{operation:'cancel',cursor:next.outcome.cursor})).outcome.status,'stale');
 cases.push('authenticated continuation rotation; foreign seat/reused cursor stale; cancel drops scratch');
 const before=await clients[seat].resume();const complete=await query(seat,{...start(decision.token,1000),page_size:32});assert.equal(complete.outcome.status,'complete');publicQueries.push(complete);
 assert.deepEqual(await clients[seat].resume(),before);cases.push('complete query preserves committed cursor/observation/lifecycle');
 const collected=[];let paged=await query(seat,{...start(decision.token,100),page_size:1});let pages=0;
 for(;pages<16;pages++){
  assert(['complete','incomplete'].includes(paged.outcome.status));collected.push(...paged.outcome.page.beginnings);publicQueries.push(paged);
  if(paged.outcome.status==='complete')break;
  paged=await query(seat,{operation:'continue',cursor:paged.outcome.cursor,work:100});
 }
 assert.equal(paged.outcome.status,'complete');assert.deepEqual(collected,complete.outcome.page.beginnings);assert(pages>=1);
 cases.push('single-use continuation chain exhausts to complete with exactly the single-page beginnings');
 assert.equal((await query(other,start(decision.token))).outcome.status,'stale');
 assert.equal((await query(seat,start('v999-d999'))).outcome.status,'stale');cases.push('foreign and stale decision tokens reveal no reason');
 const foreignView=await clients[other].resume();assert.equal(foreignView.state.status,'available');
 const foreignHand=foreignView.state.observation.view.objects.find(o=>o.zone.zone==='hand'&&o.owner===other);assert(foreignHand);
 const foreignScope=await query(seat,{...start(decision.token,1000),scope:{scope:'object',object:foreignHand.handle}});
 assert.deepEqual(foreignScope.outcome,{status:'unavailable',reason:{kind:'core',reason:'scope'}});
 cases.push('foreign scoped object handle rejected by core without foreign lookup disclosure');
 for(const [body,suffix,auth,status] of [[JSON.stringify({...start(decision.token),viewer:other}),'',true,400],[JSON.stringify(start(decision.token)).replace('"work":1','"work":1,"work":1'),'',true,400],[JSON.stringify({...start(decision.token),work:1001}),'',true,400],[' '.repeat(2049),'',true,413],[JSON.stringify(start(decision.token)),'?viewer=0',true,400],[JSON.stringify(start(decision.token)),'',false,401]]){assert.equal((await raw(seat,body,suffix,auth)).status,status);}
 cases.push('auth, unknown viewer, nested duplicate, work/body/query-string bounds');
 // Fixed-window quota witness on a different seat, no query rules mutation.
 let quota=false;for(let n=0;n<20;n++){const result=await query(other,start('v999-d999'));if(result.outcome.status==='unavailable'&&result.outcome.reason.kind==='quota'){quota=true;break;}}assert(quota);cases.push('per-seat quota is query status; no fabricated rules outcome');
 const concurrent=await Promise.all(Array.from({length:12},()=>raw(other,JSON.stringify(start('v999-d999')))));
 assert(concurrent.some(r=>r.status===429));assert(concurrent.every(r=>r.status===200||r.status===429));
 cases.push('one HTTP query in flight per authenticated seat; concurrent admission returns429');
 await delay(1100);
 const pending=await query(seat,start(decision.token));assert.equal(pending.outcome.status,'incomplete');
 await stop();await launch();
 const recovered=await query(seat,{operation:'continue',cursor:pending.outcome.cursor,work:1});assert.equal(recovered.outcome.status,'stale');
 assert.deepEqual((await clients[seat].resume()).state,resume.state);cases.push('real host restart/recovery clears private continuations and preserves permitted state');
 const abort=new AbortController();abort.abort();await assert.rejects(()=>clients[seat].queryActions(start(decision.token),{signal:abort.signal}),{name:'AbortError'});cases.push('abort before send');
 const fixture={fixture_version:1,scope:'real authenticated queries; token/handle/cursor strings are scoped to this ended capture and confer no authorization; no hidden state',queries:publicQueries};
 const fixtureBytes=JSON.stringify(fixture,null,2)+'\n';fs.writeFileSync(path.join(output,'public-queries.json'),fixtureBytes,{flag:'wx',mode:0o644});
 const report={format_version:1,status:'passed',scope:'bounded real host query/continuation and command reconciliation probe; injected response loss explicitly distinguished; no natural game/browser play claim',node:process.version,probe_sha256:sha(fs.readFileSync(new URL(import.meta.url))),sdk_build_sha256:sha(fs.readFileSync(new URL('../sdk/dist/build.json',import.meta.url))),source_manifest_sha256:sha(fs.readFileSync(sourceManifest)),binary_sha256:sha(fs.readFileSync(binary)),provisioner_sha256:sha(fs.readFileSync(provisioner)),engine_build_id:nativeInfo.engine_build_id,server_build_id:nativeInfo.server_build_id,schema_sha256:JSON.parse(fs.readFileSync(new URL('../sdk/dist/build.json',import.meta.url))).schema_sha256,cases,commands,public_fixture_sha256:sha(fixtureBytes)};
 fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx',mode:0o644});console.log(JSON.stringify({status:'passed',cases:cases.length,commands:commands.length,report:path.join(output,'report.json')}));
}finally{clearTimeout(watchdog);await stop();fs.closeSync(log);}
