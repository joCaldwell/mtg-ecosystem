import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import { GameClient, HttpError, ProtocolError, parseClientConfig, readClientConfig } from '../src/client.ts';
import { presentation } from '../../tools/client-artifacts.ts';
import { PRESENTATION_SCHEMA_SHA256, PRESENTATION_PRODUCER_SHA256 } from '../generated/presentation-identity.ts';
import { ROUTES } from '../generated/routes.ts';
const sha=(bytes:string)=>createHash('sha256').update(bytes).digest('hex');
const artifact=presentation(JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json',import.meta.url),'utf8')));
function fixture(value=artifact) {
 const bytes=JSON.stringify(value)+'\n'; const digest=sha(bytes);
 return {bytes,descriptor:{presentation_version:1,catalog_sha256:artifact.catalog_sha256,presentation_sha256:digest,presentation_schema_sha256:PRESENTATION_SCHEMA_SHA256,producer_sha256:PRESENTATION_PRODUCER_SHA256,path:`/presentation/${digest}.json`}};
}
const json=(body:string,status=200)=>new Response(body,{status,headers:{'Content-Type':'application/json'}});
test('presentation fetch uses authenticated Rust route then credential-free same-origin pinned artifact',async()=>{
 const f=fixture(); const calls:any[]=[];
 const client=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'test-private',fetch:async(url,init)=>{
  calls.push({url,init});return calls.length===1?json(JSON.stringify(f.descriptor)):json(f.bytes);
 }});
 const result=await client.presentation();assert.deepEqual(result.artifact,artifact);assert.deepEqual(result.descriptor,f.descriptor);
 assert.equal(calls[0].url,`http://127.0.0.1:18741${ROUTES.get_presentation.path.replace('{match_id}','a1')}`);
 assert.equal(calls[0].init.headers.Authorization,'Bearer test-private');assert.equal(calls[1].init.headers.Authorization,undefined);
 assert.equal(calls[1].url,`http://127.0.0.1:18741${f.descriptor.path}`);
 for(const {init} of calls){assert.equal(init.credentials,'omit');assert.equal(init.redirect,'error');assert.equal(init.cache,'no-store');}
});
test('presentation 404 stays explicit and descriptor mismatch never requests an artifact',async()=>{
 const f=fixture();
 for(const patch of [{path:'https://evil.invalid/private'},{presentation_schema_sha256:'0'.repeat(64)},{producer_sha256:'0'.repeat(64)},{catalog_sha256:'a'.repeat(63)+'\n'},{extra:'private'}]){
  let calls=0;const client=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'test',fetch:async()=>{calls++;return json(JSON.stringify({...f.descriptor,...patch}));}});
  await assert.rejects(client.presentation(),ProtocolError);assert.equal(calls,1);
 }
 const client=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'test',fetch:async()=>json('{}',404)});
 await assert.rejects(client.presentation(),e=>e instanceof HttpError&&e.status===404);
});
test('artifact digest, strict schema, catalog, duplicate identities and face indices fail closed',async()=>{
 const bad:unknown[]=[];
 const extra=structuredClone(artifact);(extra as any).private_state={};bad.push(extra);
 const catalog=structuredClone(artifact);catalog.catalog_sha256='0'.repeat(64);bad.push(catalog);
 const duplicate=structuredClone(artifact);duplicate.cards.push(duplicate.cards[0]);bad.push(duplicate);
 const face=structuredClone(artifact);face.cards[0].faces[0].face_index=1;bad.push(face);
 const image=structuredClone(artifact);image.cards[0].faces[0].image_url='https://private@evil.invalid/x';bad.push(image);
 for(const value of bad){
  const f=fixture(value as any);let calls=0;
  const client=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'test',fetch:async()=>++calls===1?json(JSON.stringify(f.descriptor)):json(f.bytes)});
  await assert.rejects(client.presentation(),ProtocolError);assert.equal(calls,2);
 }
 const f=fixture();let calls=0;const client=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'test',fetch:async()=>++calls===1?json(JSON.stringify(f.descriptor)):json(f.bytes+' ')});
 await assert.rejects(client.presentation(),ProtocolError);
});
test('presentation body cap cancels streams and never retries',async()=>{
 const f=fixture();let calls=0,cancelled=false;
 const client=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'test',fetch:async()=>++calls===1?json(JSON.stringify(f.descriptor)):new Response(new ReadableStream({start(c){c.enqueue(new Uint8Array(4*1024*1024+1));},cancel(){cancelled=true;}}),{headers:{'Content-Type':'application/json'}})});
 await assert.rejects(client.presentation(),ProtocolError);assert.equal(calls,2);assert.equal(cancelled,true);
 const abort=new AbortController();abort.abort();await assert.rejects(client.presentation({signal:abort.signal}),{name:'AbortError'});assert.equal(calls,2);
});
test('client config is Rust-shaped bounded and accepts only canonical numeric upstream origins',async()=>{
 for(const origin of ['http://127.0.0.1:80','http://[::1]:18741'])assert.equal(parseClientConfig(JSON.stringify({client_config_version:1,server_origin:origin})).server_origin,origin);
 for(const origin of ['http://localhost:18741','http://127.0.0.1:0','http://127.0.0.1:65536','http://127.0.0.1:80/','http://127.0.0.1:80\n','https://127.0.0.1:80','http://user@127.0.0.1:80'])assert.throws(()=>parseClientConfig(JSON.stringify({client_config_version:1,server_origin:origin})),ProtocolError);
 assert.throws(()=>parseClientConfig(' '.repeat(4097)),ProtocolError);
 assert.throws(()=>parseClientConfig('{"client_config_version":1,"client_config_version":1,"server_origin":"http://127.0.0.1:80"}'),ProtocolError);
 const config=await readClientConfig('http://127.0.0.1:18741',{fetch:async(url,init)=>{assert.equal(url,`http://127.0.0.1:18741${ROUTES.get_client_config.path}`);assert.equal((init!.headers as any).Authorization,undefined);return json('{"client_config_version":1,"server_origin":"http://127.0.0.1:18742"}');}});
 assert.equal(config.server_origin,'http://127.0.0.1:18742'); // Session owns expected mapping equality.
});
