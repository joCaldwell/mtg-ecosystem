import { AsyncLocalStorage } from "node:async_hooks";
import { randomUUID } from "node:crypto";
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Actor } from "./actor.ts";

type Context = { req_id: string; method: string; path: string; actor?: Actor; error?: ReturnType<typeof errorFrames> };
const requests = new AsyncLocalStorage<Context>();

// Log stack frames, never the exception message: provider errors can contain
// OAuth URLs/tokens, and validation errors can quote submitted values.
function errorFrames(error: unknown) {
  const value = error as { name?: unknown; code?: unknown; stack?: unknown } | null;
  const symbol = (v: unknown) => typeof v === "string" && /^[A-Z][A-Za-z0-9_.$-]{0,63}$/.test(v) ? v : undefined;
  let stack = typeof value?.stack === "string" ? value.stack : "";
  if (error instanceof Error) {
    const message = `${error.name}: ${error.message}`;
    if (stack.startsWith(message)) stack = stack.slice(message.length);
  }
  return { name: symbol(value?.name) ?? "Error", ...(symbol(value?.code) ? { code: symbol(value?.code) } : {}),
    stack: stack.split("\n").filter(line => /^\s+at /.test(line)).slice(0, 20).join("\n") };
}
function write(fields: Record<string, unknown>) {
  process.stdout.write(JSON.stringify({ ts: new Date().toISOString(), ...fields }) + "\n");
}
function pathForLog(req: IncomingMessage) {
  const path = new URL(req.url ?? "/", "http://localhost").pathname;
  // URL labels and unknown paths may contain secrets. Never log queries,
  // fragments, callback codes, headers, bodies, tool arguments or results.
  if (path === "/mcp" || path.startsWith("/mcp/")) return "/mcp";
  if (/^\/(api|\.well-known)\//.test(path)) {
    const routes = new Set(("api .well-known health session events account export handle settings admin invites " +
      "auth signup sign-in social callback discord google error oauth2 authorize token register consent " +
      "oauth-authorization-server oauth-protected-resource mcp openid-configuration jwks decks cards search " +
      "profile memories activity focus stats jobs refresh backup tokens clients disconnect proposals items " +
      "rule brief engines slots tags power compare fork import preview log review undo category sourcing " +
      "ownership maybe shortlist quantity playtest table observations ui budgets read retire revise " +
      // Sharing (§18). A slug or handle is never a route word, so it logs as :value:
      // an unlisted deck's slug is a capability URL and stays out of the logs.
      "public users sharing").split(" "));
    return path.split("/").map(segment => !segment || routes.has(segment) ? segment : /^\d+$/.test(segment) ? ":id" : ":value").join("/");
  }
  // Shared pages name the page kind, never the slug or handle (§18).
  const shared = path.match(/^\/(d|u)\/[^/]+\/?$/);
  if (shared) return `/${shared[1]}/:value`;
  return ["/", "/signin", "/privacy", "/terms", "/consent", "/consent.js"].includes(path) ? path : "/:other";
}

/** Transport-only logging; it observes resolved Actors and grants no rights. */
export const logging = {
  request<T>(req: IncomingMessage, res: ServerResponse, handle: () => T): T {
    const context: Context = { req_id: randomUUID(), method: req.method ?? "UNKNOWN", path: pathForLog(req) };
    const start = performance.now();
    res.setHeader("X-Request-ID", context.req_id);
    let completed = false;
    const finish = () => {
      if (completed) return;
      completed = true;
      const status = res.writableFinished ? res.statusCode : 499;
      if (context.path === "/api/health" && status < 500) return;
      const error = context.error && (status >= 500 ? context.error : { name: context.error.name, ...(context.error.code ? { code: context.error.code } : {}) });
      write({ level: status >= 500 ? "error" : status >= 400 ? "warn" : "info", event: "request",
        req_id: context.req_id, user_id: context.actor?.user_id ?? null,
        // No resolved principal (a shared page, card search, sign-in): anonymous, never an address.
        via: context.actor?.via ?? null, label: context.actor?.label ?? "anonymous",
        method: context.method, path: context.path, status,
        ms: Math.round((performance.now() - start) * 100) / 100, ...(error ? { error } : {}) });
    };
    res.once("finish", finish);
    res.once("close", finish);
    return requests.run(context, handle);
  },
  actor(actor: Actor | null) { const context = requests.getStore(); if (context && actor) context.actor = actor; },
  error(error: unknown) {
    const context = requests.getStore(); if (context) context.error = errorFrames(error);
    if (process.env.DECKBUILDER_MODE !== "hosted" && (!(error as any)?.status || (error as any).status >= 500))
      console.error(error instanceof Error ? error.message : "Unknown error");
  },
  tool(actor: Actor, tool: string, start: number, status: number, error?: unknown) {
    const context = requests.getStore();
    const detail = error ? errorFrames(error) : undefined;
    write({ level: status >= 500 ? "error" : status >= 400 ? "warn" : "info", event: "tool", req_id: context?.req_id ?? null,
      user_id: actor.user_id, via: actor.via, label: actor.label, method: context?.method ?? "POST",
      tool, status, ms: Math.round((performance.now() - start) * 100) / 100,
      ...(detail ? { error: status >= 500 ? detail : { name: detail.name, ...(detail.code ? { code: detail.code } : {}) } } : {}) });
  },
  describeError: errorFrames,
  failure(event: string, error: unknown) {
    write({ level: "error", event, error: errorFrames(error) });
    if (process.env.DECKBUILDER_MODE !== "hosted") console.error(error instanceof Error ? error.message : "Unknown error");
  },
  event(event: string, fields: Record<string, unknown> = {}) { write({ level: "info", event, ...fields }); },
};
