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
  if (mode === "hosted") return host === new URL(publicOrigin()).host && (!origin || origin === publicOrigin());
  const loopback = new Set(["127.0.0.1", "localhost", "[::1]", "::1"]);
  if (!loopback.has(host.replace(/:\d+$/, ""))) return false;
  if (!origin) return true;
  try { return loopback.has(new URL(origin).hostname); } catch { return false; }
}
