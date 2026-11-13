/** Explicit real-host read/security probe. No provisioning, Cargo, retries,
 * commands, raw-view exports or capability logging. Output is create-new. */
import { readFileSync, writeFileSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { request } from 'node:http';
import assert from 'node:assert/strict';
import { GameClient, parseSeatBootstrap, readClientConfig } from '../sdk/dist/browser.js';
const [origin, infoPath, binaryPath, sourceCommit, output, ...seatPaths] = process.argv.slice(2);
assert(origin && infoPath && binaryPath && output && /^[0-9a-f]{40}$/.test(sourceCommit ?? '') && seatPaths.length===4,'usage: check-local-serving.ts ORIGIN NATIVE_INFO BINARY SOURCE_SHA NEW_REPORT SEAT0 SEAT1 SEAT2 SEAT3');
const url=new URL(origin);assert.equal(url.href,`${url.origin}/`);assert(['127.0.0.1','[::1]'].includes(url.hostname));assert.equal(url.protocol,'http:');
const nativeInfo=JSON.parse(readFileSync(infoPath,'utf8'));const sha=(bytes:Uint8Array)=>createHash('sha256').update(bytes).digest('hex');
const config=await readClientConfig(origin);assert.equal(new URL(config.server_origin).origin,url.origin);
const seats=seatPaths.map(file=>{const stat=statSync(file);assert.equal(stat.mode&0o077,0);assert(stat.size<=4096);return parseSeatBootstrap(readFileSync(file,'utf8'));});
const raw=(path:string,headers:Record<string,string>={},method='GET',body=''):Promise<{status:number;headers:Record<string,any>;body:string}>=>new Promise((resolve,reject)=>{
 const req=request({hostname:url.hostname.replace(/^\[|\]$/g,''),port:url.port||80,path,method,headers:{Host:url.host,...headers}},res=>{
  let bytes=0;const chunks:Buffer[]=[];res.on('data',chunk=>{bytes+=chunk.length;if(bytes>8*1024*1024){req.destroy(new Error('Probe response bound'));return;}chunks.push(chunk);});
  res.on('end',()=>resolve({status:res.statusCode!,headers:res.headers,body:Buffer.concat(chunks).toString('utf8')}));
 });req.setTimeout(30_000,()=>req.destroy(new Error('Probe timeout')));req.on('error',reject);req.end(body);
});
const rows=[];
for(const [path,headers,expected,method] of [
 ['/',{},200,'GET'],['/join',{},200,'GET'],['/table',{},200,'GET'],['/about',{},200,'GET'],['/fixtures/opening',{},200,'GET'],['/fixtures/missing-metadata',{},200,'GET'],
 ['/client-config.json',{},200,'GET'],['/',{Host:'evil.example'},403,'GET'],['/',{Origin:'null'},403,'GET'],['/',{Origin:'http://evil.example'},403,'GET'],['/',{'Sec-Fetch-Site':'cross-site'},403,'GET'],
 ['/%2e%2e/operator.json',{},400,'GET'],['/../operator.json',{},400,'GET'],['/assets/%2fsecret.js',{},400,'GET'],['/index.html?source=1',{},400,'GET'],
 ['/issued.json',{},404,'GET'],['/client-assets.json',{},404,'GET'],['/presentation/artifacts.json',{},404,'GET'],['/native-info.json',{},404,'GET'],['/private/registry',{},404,'GET'],['/assets/missing.js',{},404,'GET'],
 ['/v1/matches/a1/resume',{},401,'GET'],['/v1/matches/a1/presentation',{},401,'GET'],['/v1/matches/a1/resume',{},405,'HEAD'],['/client-config.json',{},400,'OPTIONS'],
] as [string,Record<string,string>,number,string][]){
 const r=await raw(path,headers,method);assert.equal(r.status,expected,`status for ${path}`);assert.equal(r.headers['cache-control'],'no-store');assert.equal(r.headers['x-content-type-options'],'nosniff');assert.equal(r.headers['access-control-allow-origin'],undefined);assert(!r.headers['content-security-policy'].includes('unsafe-'));assert(r.headers['content-security-policy'].includes("connect-src 'self'"));
 if(expected>=400 && method!=='HEAD')assert.equal(r.body,'{"error":"request_failed"}');rows.push({path,status:r.status,method});
}
const views=[];
for(const seat of seats){
 assert.equal(new URL(seat.server_origin).origin,url.origin);assert.equal(seat.engine_build_id,nativeInfo.engine_build_id);assert.equal(seat.server_build_id,nativeInfo.server_build_id);
 const client=new GameClient({baseURL:origin,matchId:seat.match_id,capability:seat.capability});
 const manifest=await client.manifest();const resumed=await client.resume();const presentation=await client.presentation();const page=await client.poll({after:resumed.next,waitMs:0});
 for(const data of [manifest,resumed]){assert.equal(data.engine_build_id,seat.engine_build_id);assert.equal(data.server_build_id,seat.server_build_id);assert.equal(data.catalog_sha256,seat.catalog_sha256);assert.equal(data.match_id,seat.match_id);}
 assert.equal(resumed.viewer,seat.viewer);assert.equal(presentation.artifact.catalog_sha256,seat.catalog_sha256);assert.equal(page.next,resumed.next);assert.equal(page.entries.length,0);assert.equal(resumed.state.status,'available');
 if(resumed.state.status==='available')for(const object of resumed.state.observation.view.objects)if(object.zone.zone==='hand')assert.equal(object.owner,seat.viewer);
 const descriptor=await raw(`/v1/matches/${seat.match_id}/presentation`,{Authorization:`Bearer ${seat.capability}`});assert.equal(descriptor.headers['cache-control'],'no-store');
 const artifact=await raw(presentation.descriptor.path);assert.equal(artifact.status,200);assert.equal(artifact.headers['cache-control'],'public, max-age=31536000, immutable');assert.equal(artifact.headers['content-type'],'application/json');
 for(const token of seats.map(s=>s.capability))assert(!artifact.body.includes(token)&&!descriptor.body.includes(token));
 views.push({viewer:seat.viewer,match_id:seat.match_id,catalog_sha256:seat.catalog_sha256,cursor:resumed.next,presentation_sha256:presentation.descriptor.presentation_sha256});
}
const report={format_version:1,source_commit:sourceCommit,binary_sha256:sha(readFileSync(binaryPath)),engine_build_id:nativeInfo.engine_build_id,server_build_id:nativeInfo.server_build_id,origin,security_cases:rows,seat_reads:views,scope:'real HTTP reads/security only; no commands, browsers or natural game',node:process.version};
writeFileSync(output,JSON.stringify(report,null,2)+'\n',{flag:'wx',mode:0o600});console.log(JSON.stringify({status:'passed',security_cases:rows.length,seats:views.length,report:output}));
