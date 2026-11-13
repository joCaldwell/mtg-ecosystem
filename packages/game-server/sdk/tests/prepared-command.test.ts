import test from 'node:test';
import assert from 'node:assert/strict';
import { GameClient, prepareCommand, parsePreparedCommand, ProtocolError, TransportError } from '../src/client.ts';
const envelope=()=>({protocol_version:1,command_id:'a'.repeat(32),expected_revision:'18446744073709551615',command:{kind:'decision' as const,token:'t',response:{response:'pass' as const}}});
test('prepared commands detach and deeply freeze; strict recovery preserves exact whitespace and order',()=>{
 const input=envelope(), prepared=prepareCommand(input);input.command.token='changed';
 assert.equal(prepared.envelope.command.kind==='decision' && prepared.envelope.command.token,'t');
 assert(Object.isFrozen(prepared));assert(Object.isFrozen(prepared.envelope.command));
 const exact=` \n${JSON.stringify(envelope(),null,2)}\n`;
 assert.equal(parsePreparedCommand(exact).exactBody,exact);
 for(const bad of [exact.replace('"protocol_version": 1','"protocol_version": 1,"protocol_version": 1'),exact.replace('"pass"','"pass","work":{}'),' '.repeat(65_537)]) assert.throws(()=>parsePreparedCommand(bad),ProtocolError);
});
test('submitExact sends recovered string once and checks identifier; transport loss never retries',async()=>{
 const exact=`\n${JSON.stringify(envelope(),null,1)}\n`;let calls=0;
 const client=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'private',fetch:async(_url,init)=>{calls++;assert.equal(init?.body,exact);return Response.json({status:'accepted',command_id:'a'.repeat(32)});}});
 assert.equal((await client.submitExact(exact)).status,'accepted');assert.equal(calls,1);
 await assert.rejects(()=>client.submitExact(exact.replace('"pass"','"unknown"')),ProtocolError);assert.equal(calls,1);
 const lost=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'private',fetch:async()=>{calls++;throw new Error('lost');}});
 await assert.rejects(()=>lost.submitExact(exact),TransportError);assert.equal(calls,2);
 const wrong=new GameClient({baseURL:'http://127.0.0.1:18741',matchId:'a1',capability:'private',fetch:async()=>Response.json({status:'accepted',command_id:'b'.repeat(32)})});
 await assert.rejects(()=>wrong.submitExact(exact),ProtocolError);
});
