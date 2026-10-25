// The agent token (spec §10.1). Binding to 127.0.0.1 is not an authentication
// boundary: any local process, and through the browser any web page, can
// reach a loopback port. The MCP endpoint therefore requires this bearer
// token. It is generated on first run and kept beside the database.
//
// Source for the threat: MCP "Local Server Security",
// https://modelcontextprotocol.io/docs/2026-07-28/tutorials/security/local-server-security

import { randomBytes, timingSafeEqual } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { AppError } from "./errors.ts";
import { DATA_DIR } from "./db.ts";

export const TOKEN_FILE = join(DATA_DIR, "agent-token");

export function getAgentToken(file: string = TOKEN_FILE): string | null {
  if (process.env.DECKBUILDER_TOKEN) return process.env.DECKBUILDER_TOKEN;
  if (existsSync(file)) {
    const token = readFileSync(file, "utf8").trim();
    if (token) return token;
  }
  return null;
}

/** Only server startup creates a missing token; reads (including stdio) never do. */
export function ensureAgentToken(file: string = TOKEN_FILE): string {
  return getAgentToken(file) ?? rotateAgentToken(file);
}

/** Replace the token. Every client configured with the old one stops working. */
export function rotateAgentToken(file: string = TOKEN_FILE): string {
  if (process.env.DECKBUILDER_TOKEN)
    throw new AppError("Token rotation is disabled while DECKBUILDER_TOKEN is set; change or unset that environment variable instead");
  const token = randomBytes(32).toString("base64url");
  mkdirSync(join(file, ".."), { recursive: true });
  writeFileSync(file, `${token}\n`, { mode: 0o600 });
  chmodSync(file, 0o600);
  return token;
}

export function tokenMatches(presented: string, expected: string): boolean {
  const a = Buffer.from(presented);
  const b = Buffer.from(expected);
  return a.length === b.length && timingSafeEqual(a, b);
}
