import { createHash, randomBytes, randomUUID, timingSafeEqual } from "node:crypto";
import type { IncomingMessage, ServerResponse } from "node:http";
import { fromNodeHeaders, toNodeHandler } from "better-auth/node";
import { APIError } from "better-auth/api";
import { createLocalJWKSet, jwtVerify, type JSONWebKeySet } from "jose";
import type { Database } from "./sqlite.ts";
import type { Actor } from "./actor.ts";
import { authConfiguration, configureAuth } from "./auth-config.ts";
import { withTransaction } from "./db.ts";
import { ServiceError } from "./errors.ts";
import { readRequestBody } from "./http-body.ts";
import { publicOrigin } from "./mode.ts";

const digest = (s: string) => createHash("sha256").update(s).digest();
const equalCode = (a: string, b: string) => timingSafeEqual(digest(a), digest(b));
const cookieName = "deckbuilder_signup";
const intentFrom = (cookie = "") => cookie.split(";").map(x => x.trim()).find(x => x.startsWith(`${cookieName}=`))?.slice(cookieName.length + 1) ?? "";

/** Short-lived browser ticket; only hashes survive the provider redirect. */
export function beginSignup(db: Database, code: string, env = process.env, now = Date.now()): string {
  if (code.length > 256) throw new ServiceError("Invalid invite or claim code", 403);
  let kind = "open";
  if (env.DECKBUILDER_CLAIM_CODE && equalCode(code, env.DECKBUILDER_CLAIM_CODE) && db.prepare("SELECT 1 FROM users WHERE id=1 AND auth_id IS NULL").get()) kind = "claim";
  else if (db.prepare("SELECT 1 FROM invites WHERE code=? AND used_by IS NULL").get(code)) kind = "invite";
  else if (env.DECKBUILDER_INVITES_REQUIRED !== "0") throw new ServiceError("A valid unused invite or owner claim code is required for a new account", 403);
  const ticket = randomBytes(32).toString("base64url");
  withTransaction(db, () => {
    db.prepare("DELETE FROM signup_intents WHERE expires_at<=?").run(now);
    db.prepare("INSERT INTO signup_intents VALUES(?,?,?,?)").run(digest(ticket).toString("hex"), kind, digest(code).toString("hex"), now + 600_000);
  });
  return ticket;
}

export function provisionIdentity(db: Database, identity: { id: string; name: string; signupHandle?: string }, ticket: string, env = process.env, now = Date.now()): { user_id: number; claimed: boolean } {
  return withTransaction(db, () => {
    const existing = db.prepare("SELECT id FROM users WHERE auth_id=? AND deleted_at IS NULL").get(identity.id) as { id: number } | undefined;
    if (existing) return { user_id: existing.id, claimed: false };
    const intent = db.prepare("DELETE FROM signup_intents WHERE id=? AND expires_at>? RETURNING kind,code_hash").get(digest(ticket).toString("hex"), now) as { kind: string; code_hash: string } | undefined;
    if (!intent) throw new ServiceError("Sign in again with a valid invite or owner claim code", 403);
    const claimed = intent.kind === "claim";
    if (claimed && (!env.DECKBUILDER_CLAIM_CODE || !equalCode(intent.code_hash, digest(env.DECKBUILDER_CLAIM_CODE).toString("hex")) || !db.prepare("SELECT 1 FROM users WHERE id=1 AND auth_id IS NULL").get())) throw new ServiceError("The owner claim code is no longer valid", 403);
    const invite = intent.kind === "invite" ? (db.prepare("SELECT code FROM invites WHERE used_by IS NULL").all() as { code: string }[]).find(x => equalCode(digest(x.code).toString("hex"), intent.code_hash)) : undefined;
    if (intent.kind === "invite" && !invite) throw new ServiceError("The invite has already been used", 403);
    if (intent.kind === "open" && env.DECKBUILDER_INVITES_REQUIRED !== "0") throw new ServiceError("An invite is required", 403);
    let base = (identity.signupHandle ?? identity.name).normalize("NFKD").toLowerCase().replace(/[^a-z0-9_-]/g, "").slice(0, 24);
    if (base.length < 3) base = (base + "user").slice(0, 24);
    let handle = base, suffix = 1;
    while (db.prepare("SELECT 1 FROM users WHERE handle=? AND id!=?").get(handle, claimed ? 1 : -1)) {
      const tail = `_${suffix++}`; handle = base.slice(0, 24 - tail.length) + tail;
    }
    const collision = handle !== base ? 1 : 0, name = identity.name.slice(0, 200) || handle;
    const id = claimed ? 1 : Number(db.prepare("INSERT INTO users(handle,display_name,auth_id,needs_handle) VALUES(?,?,?,?)").run(handle, name, identity.id, collision).lastInsertRowid);
    if (claimed) db.prepare("UPDATE users SET handle=?,display_name=?,auth_id=?,needs_handle=? WHERE id=1 AND auth_id IS NULL").run(handle, name, identity.id, collision);
    if (invite) db.prepare("UPDATE invites SET used_by=?,used_at=datetime('now') WHERE code=? AND used_by IS NULL").run(id, invite.code);
    return { user_id: id, claimed };
  });
}

export function createHostedAuth(db: Database, env = process.env) {
  const origin = publicOrigin(env);
  if (!env.BETTER_AUTH_SECRET || env.BETTER_AUTH_SECRET.length < 32) throw new Error("Hosted mode requires BETTER_AUTH_SECRET (at least 32 characters)");
  const options = authConfiguration(db, origin, env);
  if (!Object.keys(options.socialProviders ?? {}).length) throw new Error("Hosted mode requires a configured Discord or Google provider");
  // v1.7.13 concepts/database.mdx: social creation runs in the native
  // adapter's transaction. Principal/invite writes BEFORE auth insertion
  // are synchronous and roll back with a failed provider account creation.
  const claims = new Set<string>();
  options.databaseHooks = { user: { create: {
    before: async (user, ctx) => {
      const id = randomUUID();
      try {
        if (provisionIdentity(db, { id, name: user.name, signupHandle: user.signupHandle as string | undefined }, intentFrom(ctx?.headers?.get("cookie") ?? ""), env).claimed) claims.add(id);
      } catch (e) { throw new APIError("FORBIDDEN", { message: e instanceof ServiceError ? e.message : "Account registration failed" }); }
      return { data: { ...user, id } };
    },
    after: async user => { if (claims.delete(user.id)) console.log("[auth] Owner account claimed; you may now remove DECKBUILDER_CLAIM_CODE."); },
  } }, session: { create: { before: async session => {
    if (!db.prepare("SELECT 1 FROM users WHERE auth_id=? AND deleted_at IS NULL").get(session.userId)) return false;
  } } } };
  const auth = configureAuth(options), ready = auth.$context;
  const nodeHandler = toNodeHandler({ handler: async (request:Request) => {
    const response=await auth.handler(request);
    // A registration race/expired intent is recoverable on our sign-in page;
    // do not strand the browser on a JSON callback error from a DB hook.
    if(new URL(request.url).pathname.startsWith("/api/auth/callback/") && response.status>=400 && response.status<500)
      return Response.redirect(`${origin}/signin?error=registration_failed`,302);
    return response;
  } });
  return {
    auth, origin, ready,
    async resolveUiActor(req: IncomingMessage): Promise<Actor | null> {
      const session = await auth.api.getSession({ headers: fromNodeHeaders(req.headers) });
      if (!session) return null;
      const user = db.prepare("SELECT id FROM users WHERE auth_id=? AND deleted_at IS NULL").get(session.user.id) as { id: number } | undefined;
      return user ? { user_id: user.id, via: "ui", label: "ui" } : null;
    },
    signup(_req: IncomingMessage, res: ServerResponse, code: string) {
      const ticket = beginSignup(db, code, env);
      res.setHeader("Set-Cookie", `${cookieName}=${ticket}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600${origin.startsWith("https:") ? "; Secure" : ""}`);
    },
    async handle(req: IncomingMessage, res: ServerResponse) {
      await ready;
      // better-call@1.4.4's Node bridge explicitly supports a consumed .body
      // string. Keep OAuth form bytes intact; no duplicate-key reserialization.
      // https://github.com/better-auth/better-call/blob/v1.4.4/src/adapters/node/request.ts
      if(!["GET","HEAD"].includes(req.method ?? "GET")) (req as IncomingMessage & {body:string}).body=(await readRequestBody(req)).toString("utf8");
      // The Node bridge itself reads x-forwarded-proto. Give it only our fixed,
      // validated issuer; user headers never influence URL construction here.
      req.headers.host=new URL(origin).host;
      req.headers["x-forwarded-proto"]=new URL(origin).protocol.slice(0,-1);
      return nodeHandler(req, res);
    },
    async verifyAccessToken(token: string): Promise<Actor | null> {
      try {
        // https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization
        // Validate public JWKS, issuer, expiry and exact RFC 8707 audience.
        // Read our own JWKS through the handler; no proxy-derived network URL.
        const jwks = await auth.handler(new Request(`${origin}/api/auth/jwks`));
        const { payload } = await jwtVerify(token, createLocalJWKSet(await jwks.json() as JSONWebKeySet), {
          issuer: `${origin}/api/auth`, audience: `${origin}/mcp`, requiredClaims: ["exp", "sub", "iat"], typ: "at+jwt",
        });
        if (payload.aud !== `${origin}/mcp`) return null;
        if (payload.cnf !== undefined) return null; // Bearer door never downgrades a DPoP-bound token.
        if (typeof payload.client_id !== "string" || typeof payload.scope !== "string" || !payload.scope.split(" ").includes("deck-builder")) return null;
        const user = db.prepare("SELECT id FROM users WHERE auth_id=? AND deleted_at IS NULL").get(payload.sub!) as { id: number } | undefined;
        const client = db.prepare('SELECT name FROM oauthClient WHERE clientId=? AND (disabled IS NULL OR disabled=0)').get(payload.client_id) as { name: string | null } | undefined;
        const consent = db.prepare('SELECT scopes FROM oauthConsent WHERE clientId=? AND userId=?').get(payload.client_id, payload.sub!) as { scopes: string } | undefined;
        if (!user || !client || !consent || !(JSON.parse(consent.scopes) as string[]).includes("deck-builder")) return null;
        const label = (client.name ?? "OAuth client").replace(/[^\w .@-]/g, "").slice(0, 80) || "OAuth client";
        db.prepare("INSERT INTO oauth_usage(user_id,client_id,label) VALUES(?,?,?) ON CONFLICT(user_id,client_id) DO UPDATE SET label=excluded.label,last_used_at=datetime('now')").run(user.id, payload.client_id, label);
        return { user_id: user.id, via: "agent", label, oauth_client_id: payload.client_id };
      } catch { return null; }
    },
  };
}
export type HostedAuth = ReturnType<typeof createHostedAuth>;
