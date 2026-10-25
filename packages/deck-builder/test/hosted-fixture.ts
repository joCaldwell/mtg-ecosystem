import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { createHmac, randomBytes, createHash } from "node:crypto";
import { createServer } from "node:net";
import type { AddressInfo } from "node:net";
import { generateKeyPair, exportJWK, SignJWT } from "jose";
import { createHostedAuth, beginSignup, provisionIdentity } from "../src/auth.ts";
import { createApp } from "../src/app.ts";
import { accountInfo, accountScopes, exportAccount, deleteAccount, createInvite, chooseHandle, adminInfo } from "../src/accounts.ts";
import { createRateLimiter, LIMITS, RateLimitError } from "../src/ratelimit.ts";
import { createDeck } from "../src/deck/service.ts";
import { remember } from "../src/memory.ts";
import { createToken } from "../src/tokens.ts";
import { createProposal } from "../src/deck/proposals.ts";
import { OWNER, agent } from "./actors.ts";
import { testDb } from "./helpers.ts";
export const secret = "test-only-auth-secret-0123456789-abcdef";
export const env = { BETTER_AUTH_SECRET: secret, DISCORD_CLIENT_ID:"fixture", DISCORD_CLIENT_SECRET:"fixture", DECKBUILDER_PUBLIC_ORIGIN:"http://localhost:8787", DECKBUILDER_CLAIM_CODE:"test-only-owner-code" };
export function enroll(db: ReturnType<typeof testDb>, id:string, code:string, name=id) {
  return provisionIdentity(db,{id,name},beginSignup(db,code,env),env);
}
export function authRow(db: ReturnType<typeof testDb>, id:string) {
  db.prepare('INSERT INTO user(id,name,email,emailVerified,createdAt,updatedAt) VALUES(?,?,?,1,?,?)').run(id,id,`${id}@test.invalid`,Date.now(),Date.now());
}
export async function fixture(metadataFetch?: Parameters<typeof createHostedAuth>[2]) {
  const socket=createServer();await new Promise<void>(r=>socket.listen(0,"127.0.0.1",r));
  const port=(socket.address() as AddressInfo).port;await new Promise<void>(r=>socket.close(()=>r()));
  const base=`http://127.0.0.1:${port}`;
  const saved=process.env.DECKBUILDER_PUBLIC_ORIGIN;process.env.DECKBUILDER_PUBLIC_ORIGIN=base;
  const db=testDb(),auth=createHostedAuth(db,{...env,DECKBUILDER_PUBLIC_ORIGIN:base},metadataFetch);await auth.ready;
  const app=createApp(db,{mode:"hosted",auth});await new Promise<void>(r=>app.server.listen(port,"127.0.0.1",r));
  async function call(path:string,opts:RequestInit={}) {return fetch(base+path,{...opts,headers:{Origin:base,...opts.headers}});}
  async function session(id:string) {
    authRow(db,id);
    const ctx=await auth.ready, session=await ctx.internalAdapter.createSession(id);
    assert.ok(session);
    const signature=createHmac("sha256",secret).update(session.token).digest("base64");
    return `better-auth.session_token=${encodeURIComponent(session.token+"."+signature)}`;
  }
  return {db,auth,app,base,call,session,async close(){await app.close();db.close();if(saved===undefined)delete process.env.DECKBUILDER_PUBLIC_ORIGIN;else process.env.DECKBUILDER_PUBLIC_ORIGIN=saved;}};
}
