import type { IncomingMessage } from "node:http";
import type { Actor } from "./actor.ts";

export type Mode = "local" | "hosted";
export function getMode(env = process.env): Mode {
  const value = env.DECKBUILDER_MODE ?? "local";
  if (value !== "local" && value !== "hosted") throw new Error("DECKBUILDER_MODE must be local or hosted");
  if (value === "hosted") publicOrigin(env);
  return value;
}
export function publicOrigin(env = process.env): string {
  const value = env.DECKBUILDER_PUBLIC_ORIGIN;
  if (!value) throw new Error("Hosted mode requires DECKBUILDER_PUBLIC_ORIGIN");
  const url = new URL(value);
  if (!["http:", "https:"].includes(url.protocol) || url.origin !== value)
    throw new Error("DECKBUILDER_PUBLIC_ORIGIN must be an HTTP(S) origin without a trailing slash");
  return url.origin;
}
export function localActor(): Actor { return { user_id: 1, via: "ui", label: "ui" }; }

/** Both front doors use the same mode-aware Host/Origin boundary. */
export function allowedRequest(req: IncomingMessage, mode: Mode): boolean {
  const host = req.headers.host ?? "";
  const origin = req.headers.origin;
  if (mode === "hosted") {
    const expected = new URL(publicOrigin());
    const proxy = process.env.DECKBUILDER_TRUST_PROXY === "1";
    const forwardedHost = req.headers["x-forwarded-host"];
    const forwardedProto = req.headers["x-forwarded-proto"];
    if (proxy && (Array.isArray(forwardedHost) || Array.isArray(forwardedProto))) return false;
    const actualHost = proxy && forwardedHost ? forwardedHost : host;
    if (actualHost !== expected.host || (proxy && forwardedProto && forwardedProto !== expected.protocol.slice(0,-1))) return false;
    return !origin || origin === expected.origin;
  }
  const loopback = new Set(["127.0.0.1", "localhost", "[::1]", "::1"]);
  if (!loopback.has(host.replace(/:\d+$/, ""))) return false;
  if (!origin) return true;
  try { return loopback.has(new URL(origin).hostname); } catch { return false; }
}

/** Only enable behind one trusted ingress which overwrites these headers. */
export function clientIp(req: IncomingMessage, env = process.env): string {
  const forwarded = req.headers["x-forwarded-for"];
  return env.DECKBUILDER_TRUST_PROXY === "1" && typeof forwarded === "string"
    ? forwarded.split(",")[0].trim() : req.socket.remoteAddress ?? "unknown";
}
export function securityHeaders(res: import("node:http").ServerResponse, mode: Mode) {
  if (mode !== "hosted") return;
  res.setHeader("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https://cards.scryfall.io; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'self'");
  res.setHeader("X-Content-Type-Options", "nosniff");
  res.setHeader("Referrer-Policy", "strict-origin-when-cross-origin");
  if (publicOrigin().startsWith("https:")) res.setHeader("Strict-Transport-Security", "max-age=31536000");
  res.setHeader("Cache-Control", "no-store");
}
