// The agent endpoint (spec §10.1): MCP over Streamable HTTP, served by the
// same process that owns the database and serves the UI.
//
// Built on the official TypeScript SDK's v2 line (@modelcontextprotocol/server),
// which implements the 2026-07-28 revision of the protocol and, from the same
// factory, serves clients still speaking the 2025 revisions statelessly. That
// revision removed sessions and the initialize handshake
// (https://modelcontextprotocol.io/specification/2026-07-28/changelog), which
// is why nothing here keeps per-connection state and every tool names its
// deck.
//
// The SDK's handler validates neither Host nor Origin and never reads a
// token; all three checks are done here, in front of it, as its serving guide
// directs (https://ts.sdk.modelcontextprotocol.io/v2/serving/http).

import type { IncomingMessage, ServerResponse } from "node:http";
import type { Database } from "../sqlite.ts";
import {
  McpServer,
  createMcpHandler,
} from "@modelcontextprotocol/server";
import { SAMPLE_AGENT_LABEL, type Actor } from "../actor.ts";
import { readRequestBody } from "../http-body.ts";
import { createRateLimiter, RateLimitError } from "../ratelimit.ts";
import { clientIp } from "../mode.ts";
import { allowedRequest, getMode, type Mode, publicOrigin } from "../mode.ts";
import { AppError } from "../errors.ts";
import { verifyToken } from "../tokens.ts";
import { INSTRUCTIONS } from "./playbook.ts";
import { TOOLS, inputSchema, runTool } from "./tools.ts";
import { logging } from "../logging.ts";

export const MCP_PATH = "/mcp";
const MAX_BODY_BYTES = 1024 * 1024;

// The 2026-07-28 revision carries the client's identity in every request's
// _meta; the 2025 revisions sent it once, in initialize.
const CLIENT_INFO_KEY = "io.modelcontextprotocol/clientInfo";

export interface AgentEndpointOptions {
  mode?: Mode;
  version: string;
  verifyOAuth?: (token: string) => Promise<Actor | null>;
  rateLimit?: ReturnType<typeof createRateLimiter>;
  withDatabase?: <T>(work:()=>T|Promise<T>)=>Promise<T>;
  /** Called after any tool wrote something, so the UI can refresh. */
  onChange?: (userId: number, change: { kind: string; deck_id?: number }) => void;
}

// Who is calling, for attribution only (spec §1, rule 5) — it grants nothing.
// A name in the URL (/mcp/claude-code) is the owner's own label for a client
// and wins; otherwise the client's self-reported name; otherwise its
// User-Agent product; otherwise "agent".
export function agentName(pathname: string, body: unknown, userAgent: string | undefined): string {
  let fromPath = "";
  try { fromPath = decodeURIComponent(pathname.slice(MCP_PATH.length).replace(/^\/+|\/+$/g, "")); } catch { /* malformed hint carries no authority */ }
  if (fromPath) return clean(fromPath);
  const first = Array.isArray(body) ? body[0] : body;
  const params = (first as any)?.params;
  const reported = params?._meta?.[CLIENT_INFO_KEY]?.name ?? params?.clientInfo?.name;
  if (typeof reported === "string" && reported.trim()) return clean(reported);
  const product = userAgent?.split(/[\s/]/)[0];
  if (product && !/^(node|undici|mozilla|curl)$/i.test(product)) return clean(product);
  return "agent";
}

const clean = (name: string) => name.replace(/[^\w .@-]/g, "").trim().slice(0, 40) || "agent";

function respond(res: ServerResponse, status: number, message: string, headers: Record<string, string> = {}) {
  res.writeHead(status, { "Content-Type": "application/json", ...headers });
  res.end(JSON.stringify({ jsonrpc: "2.0", error: { code: -32000, message }, id: null }));
}

export function createAgentEndpoint(db: Database, opts: AgentEndpointOptions) {
  // Which protocol era each client speaks, logged once per client: the
  // question spec §15 leaves to be answered by what actually connects.
  const mode = opts.mode ?? getMode();
  const seen = new Set<string>();
  const handler = createMcpHandler(
    (ctx) => {
      const actor = ctx.authInfo!.extra!.actor as Actor;
      const who = `${ctx.authInfo?.clientId ?? "agent"}/${ctx.era}`;
      if (!seen.has(who)) {
        if (seen.size >= 256) seen.clear();
        seen.add(who);
        logging.event("mcp_connected", { user_id: actor.user_id, via: actor.via, label: actor.label, era: ctx.era });
      }
      const server = new McpServer(
        { name: "deck-builder", version: opts.version },
        { instructions: INSTRUCTIONS },
      );
      // Registered in the order of TOOLS, which never varies, so a client can
      // cache the list (the spec asks for a deterministic order).
      for (const tool of TOOLS) {
        const run = async (args: unknown) => {
          const start = performance.now();
          try {
            const result = runTool({ db, actor }, tool, args ?? {});
            if (result.changed) opts.onChange?.(actor.user_id, { kind: tool.name });
            logging.tool(actor, tool.name, start, 200);
            return { content: [{ type: "text" as const, text: result.text }],
              ...(result.structuredContent ? { structuredContent: result.structuredContent } : {}) };
          } catch (e) {
            logging.tool(actor, tool.name, start, e instanceof AppError ? e.status : 500, e);
            // A failure the agent can act on is a tool result, not a protocol
            // error: the message says how to fix the call.
            if (e instanceof AppError)
              return { content: [{ type: "text" as const, text: e.message }], isError: true };
            return {
              content: [{ type: "text" as const, text: "The app hit an internal error handling that call. Nothing was changed." }],
              isError: true,
            };
          }
        };
        const config = {
          title: tool.title,
          description: tool.description,
          annotations: {
            readOnlyHint: tool.readOnly,
            destructiveHint: tool.destructive ?? false,
            idempotentHint: tool.readOnly,
            openWorldHint: false,
          },
        };
        server.registerTool(tool.name, { ...config, inputSchema: inputSchema(tool) }, run);
      }
      return server;
    },
    { onerror: (e) => logging.error(e) },
  );

  async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    if (!allowedRequest(req, mode)) return respond(res, 403, "Host or Origin is not allowed");
    const url = new URL(req.url ?? "/", mode === "hosted" ? publicOrigin() : "http://127.0.0.1");
    const headers = new Headers();
    for (const [k, v] of Object.entries(req.headers))
      if (v !== undefined) headers.set(k, Array.isArray(v) ? v.join(", ") : v);

    let body: Buffer;
    try {
      body = req.method === "GET" || req.method === "HEAD" ? Buffer.alloc(0) : await readRequestBody(req);
    } catch (e: any) {
      return respond(res, e.status ?? 400, e.message);
    }
    const request = new Request(url, {
      method: req.method,
      headers,
      body: body.length ? new Uint8Array(body) : undefined,
    });

    // DNS rebinding and cross-site requests: a page in the owner's browser
    // can reach a loopback port, so the Host must be loopback and an Origin,
    // if present, must be too.
    if (!allowedRequest(req, mode)) return respond(res, 403, "Host or Origin is not allowed");

    const dispatch = async () => {
    const auth = req.headers.authorization ?? "";
    const presented = auth.startsWith("Bearer ") ? auth.slice(7).trim() : "";
    const authenticated = verifyToken(db, presented) ?? (mode === "hosted" && presented ? await opts.verifyOAuth?.(presented) : null);
    if (!authenticated) {
      if (mode === "hosted") try { opts.rateLimit?.("unauthenticated", clientIp(req)); }
      catch(e) { if(e instanceof RateLimitError) return respond(res,429,e.message,{"Retry-After":String(e.retryAfter)});throw e; }
      return respond(
        res,
        401,
        "Missing or wrong bearer token. Copy the agent token from the app's Settings page into this client's configuration.",
        { "WWW-Authenticate": `Bearer resource_metadata="${mode === "hosted" ? publicOrigin() : `http://${req.headers.host}`}/.well-known/oauth-protected-resource/mcp"${mode === "hosted" ? ", scope=\"deck-builder\"" : ""}${presented ? ", error=\"invalid_token\"" : ""}` },
      );
    }

    let parsed: unknown;
    if (body.length) {
      try {
        parsed = JSON.parse(body.toString("utf8"));
      } catch {
        return respond(res, 400, "Request body is not valid JSON");
      }
    }
    const hint = agentName(url.pathname, parsed, req.headers["user-agent"]);
    const name = mode === "hosted" || authenticated.oauth_client_id || hint === "agent" ? authenticated.label : hint;
    if (mode === "hosted") try {
      opts.rateLimit?.("mcp", authenticated.oauth_client_id ? `${authenticated.user_id}:${authenticated.oauth_client_id}` : `personal:${authenticated.token_id}`);
      const tool = (parsed as any)?.params?.name;
      if (tool === "search_cards" || tool === "search_memories") opts.rateLimit?.("search", `mcp:${authenticated.user_id}`);
      if (tool === "compare_decks" || tool === "compare_deck_sources") opts.rateLimit?.("compare", `mcp:${authenticated.user_id}`);
    } catch (e) { if (e instanceof RateLimitError) return respond(res,429,e.message,{"Retry-After":String(e.retryAfter)}); throw e; }
    // The sample's label marks demonstration data (§11.3); a client that
    // claims it, by URL, token label or OAuth name, is attributed apart.
    const actor: Actor = { ...authenticated, label: name === SAMPLE_AGENT_LABEL ? `${name} (connected)` : name };
    logging.actor(actor);
    const response = await handler.fetch(request, {
      authInfo: { token: presented, clientId: name, scopes: [], extra: { actor } },
      parsedBody: parsed,
    });
    return response;
    };
    const response = await (opts.withDatabase ? opts.withDatabase(dispatch) : dispatch());
    if (response) return send(res, response);
  }

  return { handle, close: () => handler.close?.() };
}

async function send(res: ServerResponse, response: Response): Promise<void> {
  const headers: Record<string, string> = {};
  response.headers.forEach((v, k) => (headers[k] = v));
  res.writeHead(response.status, headers);
  if (!response.body) return void res.end();
  const reader = response.body.getReader();
  res.on("close", () => void reader.cancel().catch(() => {}));
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    res.write(value);
  }
  res.end();
}
