import test from 'node:test';
import assert from 'node:assert/strict';
import {GameClient, ProtocolError, TransportError} from '../src/client.ts';
const page={token:'v0-d0',scope:{scope:'window' as const},context:{window:'priority' as const},beginnings:[{initiation:{action:'pass'},label:'unavailable'}]};
const client=(fetch:typeof globalThis.fetch)=>new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'secret',fetch});
const request={operation:'start' as const,token:'v0-d0',scope:{scope:'window' as const},page_size:1,work:1};
test('bounded generated action query sends once, returns faithful contexts and explicit cursor statuses',async()=>{
 let calls=0;const cursor='f'.repeat(64);
 const sdk=client(async(url,init)=>{calls++;assert.equal(url,'http://127.0.0.1:18741/v1/matches/a1/actions/query');assert.equal(init?.method,'POST');assert.equal(new Headers(init?.headers).get('authorization'),'Bearer secret');
 const sent=JSON.parse(init?.body as string);return Response.json({protocol_version:1,match_id:'a1',viewer:0,outcome:sent.operation==='cancel'?{status:'cancelled'}:{status:'incomplete',page,required_minimum:1,cursor}});});
 const result=await sdk.queryActions(request);assert.equal(result.outcome.status,'incomplete');
 await sdk.queryActions({operation:'continue',cursor,work:1});await sdk.queryActions({operation:'cancel',cursor});assert.equal(calls,3);
 for(const invalid of [{...request,viewer:0},{...request,work:1001},{...request,page_size:0},{operation:'continue',cursor:'bad',work:1}]) await assert.rejects(()=>sdk.queryActions(invalid as never),ProtocolError);
 assert.equal(calls,3);
});
test('query rejects raw duplicates, oversized pages, wrong bindings and transport failure without retry',async()=>{
 for(const outcome of [{status:'complete',page:{...page,token:'foreign'}},{status:'complete',page:{...page,beginnings:Array(33).fill(page.beginnings[0])}},{status:'complete',page:{...page,scope:{scope:'object',object:'foreign'}}},{status:'stale',work:{}}]){
  await assert.rejects(()=>client(async()=>Response.json({protocol_version:1,match_id:'a1',viewer:0,outcome})).queryActions(request),ProtocolError);
 }
 let calls=0;await assert.rejects(()=>client(async()=>{calls++;throw new Error('private');}).queryActions(request),TransportError);assert.equal(calls,1);
 await assert.rejects(()=>client(async()=>new Response('{"protocol_version":1,"protocol_version":1}',{headers:{'content-type':'application/json'}})).queryActions(request),ProtocolError);
 const abort=new AbortController();abort.abort();await assert.rejects(()=>client(async()=>{throw new Error('must not fetch');}).queryActions(request,{signal:abort.signal}),{name:'AbortError'});
});
