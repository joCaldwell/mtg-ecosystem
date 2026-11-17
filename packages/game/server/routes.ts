import { ServiceError, type ApplicationRequest } from "@mtg-ecosystem/deck-builder/application";
import type { createGameCoordinator } from "./games.ts";
function num(value: unknown, name = "number", min = 0): number {
  if ((typeof value !== "number" && typeof value !== "string") || String(value).trim() === "") throw new ServiceError(`${name} must be an integer`);
  const n = Number(value);
  if (!Number.isSafeInteger(n) || n < min) throw new ServiceError(`${name} must be an integer >= ${min}`);
  return n;
}
export async function handleGameRequest(games: ReturnType<typeof createGameCoordinator>, { req, res, url, actor, json, readJson, readRequestBody }: ApplicationRequest): Promise<void> {
  const nativePath = url.pathname.match(/^\/api\/games\/([a-f0-9]{32})\/native\/(client-config\.json|presentation\/[a-f0-9]{64}\.json|v1\/.*)$/);
  if (nativePath) {
    if (req.method !== "GET" && req.method !== "POST") return json(405, { error: "Native operation unavailable" });
    // Preserve exact validated UTF-8 bytes. Never JSON.parse/re-serialize a
    // journal body or hold the shared database gate over upstream I/O.
    if (req.method === "GET" && (req.headers["transfer-encoding"] || req.headers["content-length"] && req.headers["content-length"] !== "0")) return json(400, { error: "Unexpected native body" });
    if (req.method === "POST" && req.headers["content-type"] && req.headers["content-type"].split(";")[0].trim().toLowerCase() !== "application/json") return json(415, { error: "Content-Type must be application/json" });
    const raw = await readRequestBody(req);
    if (raw.length > 65_536) return json(413, { error: "Native body exceeds limit" });
    let body: string;
    try { body = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(raw); }
    catch { return json(400, { error: "Invalid native body encoding" }); }
    if (req.method === "GET" && body) return json(400, { error: "Unexpected native body" });
    const authorization = req.headers.authorization;
    const publicGet = req.method === "GET" && (nativePath[2] === "client-config.json" || /^presentation\/[a-f0-9]{64}\.json$/.test(nativePath[2]));
    if (!publicGet && (typeof authorization !== "string" || !/^Bearer [a-f0-9]{64}$/.test(authorization))) return json(401, { error: "Human seat authorization is required" });
    // Reject URL normalization/encoded aliases before passing the reviewed
    // path. The bootstrap cannot choose this app-owned endpoint.
    const path = `/${nativePath[2]}${url.search}`;
    const originalPath = (req.url ?? "").split("?")[0];
    if (!originalPath.startsWith("/") || originalPath.startsWith("//") || originalPath.includes("%") || originalPath.includes("\\") || /(^|\/)\.{1,2}(\/|$)/.test(originalPath)) return json(400, { error: "Invalid native path" });
    const reply = await games.nativeProxy(actor, nativePath[1], { method: req.method, path, body, authorization: authorization ?? "" });
    res.writeHead(reply.status, { "Content-Type": "application/json", "Cache-Control": "no-store" });
    res.end(reply.body); return;
  }
  const body = req.method === "POST" ? await readJson(req) : {};
  const match = url.pathname.match(/^\/api\/games\/([a-f0-9]{32})(\/(resume|seat))?$/);
  if (req.method === "POST" && url.pathname === "/api/games/admission") return json(200, { admission: await games.admission(actor, num(body.deck_id, "deck_id", 1), num(body.expected_revision, "expected_revision")) });
  if (req.method === "POST" && url.pathname === "/api/games") return json(200, await games.launch(actor, num(body.deck_id, "deck_id", 1), num(body.expected_revision, "expected_revision"), body.request_id));
  if (req.method === "GET" && url.pathname === "/api/games") return json(200, { games: await games.list(actor) });
  if (match && req.method === "GET" && !match[2]) return json(200, { game: await games.get(actor, match[1]) });
  if (match && req.method === "POST" && match[3] === "resume") return json(200, await games.resume(actor, match[1]));
  if (match && req.method === "POST" && match[3] === "seat") return json(200, await games.humanSeat(actor, match[1]));
  return json(404, { error: "Game operation not found" });
}
