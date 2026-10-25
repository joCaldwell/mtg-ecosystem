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
import * as z from "zod/v4";
import {
  McpServer,
  createMcpHandler,
  hostHeaderValidationResponse,
  localhostAllowedHostnames,
  localhostAllowedOrigins,
  originValidationResponse,
} from "@modelcontextprotocol/server";
import { agent } from "../actor.ts";
import { AppError } from "../errors.ts";
import { tokenMatches } from "../token.ts";
import { INSTRUCTIONS } from "./playbook.ts";
import { TOOLS } from "./tools.ts";

export const MCP_PATH = "/mcp";
const MAX_BODY_BYTES = 1024 * 1024;

// The 2026-07-28 revision carries the client's identity in every request's
// _meta; the 2025 revisions sent it once, in initialize.
const CLIENT_INFO_KEY = "io.modelcontextprotocol/clientInfo";

export interface AgentEndpointOptions {
  token: string;
  version: string;
  /** Called after any tool wrote something, so the UI can refresh. */
  onChange?: () => void;
}

// Who is calling, for attribution only (spec §1, rule 5) — it grants nothing.
// A name in the URL (/mcp/claude-code) is the owner's own label for a client
// and wins; otherwise the client's self-reported name; otherwise its
// User-Agent product; otherwise "agent".
export function agentName(pathname: string, body: unknown, userAgent: string | undefined): string {
  const fromPath = decodeURIComponent(pathname.slice(MCP_PATH.length).replace(/^\/+|\/+$/g, ""));
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

function readBody(req: IncomingMessage): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0, oversized = false;
    req.on("data", (chunk: Buffer) => {
      if (oversized) return;
      size += chunk.length;
      if (size > MAX_BODY_BYTES) { oversized = true; chunks.length = 0; reject(new AppError("Request body too large", 413)); }
      else chunks.push(chunk);
    });
    req.on("end", () => { if (!oversized) resolve(Buffer.concat(chunks)); });
    req.on("error", reject);
  });
}

export function createAgentEndpoint(db: Database, opts: AgentEndpointOptions) {
  // Which protocol era each client speaks, logged once per client: the
  // question spec §15 leaves to be answered by what actually connects.
  const seen = new Set<string>();
  const handler = createMcpHandler(
    (ctx) => {
      const actor = agent(ctx.authInfo?.clientId ?? "agent");
      const who = `${ctx.authInfo?.clientId ?? "agent"}/${ctx.era}`;
      if (!seen.has(who)) {
        seen.add(who);
        console.log(
          `[mcp] "${ctx.authInfo?.clientId ?? "agent"}" connected, speaking the ` +
            (ctx.era === "modern" ? "2026-07-28 protocol" : "2025 protocol (served statelessly)"),
        );
      }
      const server = new McpServer(
        { name: "deck-builder", version: opts.version },
        { instructions: INSTRUCTIONS },
      );
      // Registered in the order of TOOLS, which never varies, so a client can
      // cache the list (the spec asks for a deterministic order).
      for (const tool of TOOLS) {
        const run = async (args: unknown) => {
          try {
            const out = tool.run({ db, actor }, args ?? {});
            const result = typeof out === "string" ? { text: out } : out;
            if (result.changed) opts.onChange?.();
            return { content: [{ type: "text" as const, text: result.text }],
              ...(result.structuredContent ? { structuredContent: result.structuredContent } : {}) };
          } catch (e) {
            // A failure the agent can act on is a tool result, not a protocol
            // error: the message says how to fix the call.
            if (e instanceof AppError)
              return { content: [{ type: "text" as const, text: e.message }], isError: true };
            console.error(`[mcp] ${tool.name} failed`, e);
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
        if (Object.keys(tool.input).length)
          server.registerTool(tool.name, { ...config, inputSchema: ["create_deck", "edit_cards"].includes(tool.name) ? z.strictObject(tool.input) : z.object(tool.input) }, run);
        else server.registerTool(tool.name, config, () => run({}));
      }
      return server;
    },
    { onerror: (e) => console.error("[mcp]", e.message) },
  );

  async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "127.0.0.1"}`);
    const headers = new Headers();
    for (const [k, v] of Object.entries(req.headers))
      if (v !== undefined) headers.set(k, Array.isArray(v) ? v.join(", ") : v);

    let body: Buffer;
    try {
      body = req.method === "GET" || req.method === "HEAD" ? Buffer.alloc(0) : await readBody(req);
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
    const rejected =
      hostHeaderValidationResponse(request, localhostAllowedHostnames()) ??
      originValidationResponse(request, localhostAllowedOrigins());
    if (rejected) return send(res, rejected);

    const auth = req.headers.authorization ?? "";
    const presented = auth.startsWith("Bearer ") ? auth.slice(7).trim() : "";
    if (!presented || !tokenMatches(presented, opts.token))
      return respond(
        res,
        401,
        "Missing or wrong bearer token. Copy the agent token from the app's Settings page into this client's configuration.",
        { "WWW-Authenticate": "Bearer" },
      );

    let parsed: unknown;
    if (body.length) {
      try {
        parsed = JSON.parse(body.toString("utf8"));
      } catch {
        return respond(res, 400, "Request body is not valid JSON");
      }
    }
    const name = agentName(url.pathname, parsed, req.headers["user-agent"]);
    const response = await handler.fetch(request, {
      authInfo: { token: presented, clientId: name, scopes: [] },
      parsedBody: parsed,
    });
    return send(res, response);
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
