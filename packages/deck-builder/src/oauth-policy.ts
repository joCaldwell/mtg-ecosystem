import { verifyOAuthQueryParams } from "@better-auth/oauth-provider";
import type { Database } from "./sqlite.ts";
import { ServiceError } from "./errors.ts";

export function nativeRedirect(uri: string): boolean {
  try {
    const u = new URL(uri);
    if (u.username || u.password || u.hash) return false;
    if (u.protocol === "http:") return ["localhost", "127.0.0.1", "[::1]"].includes(u.hostname);
    return !["https:", "http:", "javascript:", "data:", "file:", "vbscript:", "ftp:", "ws:", "wss:"].includes(u.protocol) && !!u.pathname;
  } catch { return false; }
}
export interface ConsentIdentity {
  name: string; identityHost: string | null; redirectHost: string; redirectUri: string; loopback: boolean;
}
export async function consentIdentity(db: Database, query: string, secret: string): Promise<ConsentIdentity> {
  // Verify the SAME signed parameters the provider will consume. A valid
  // signature alone is not consent: bind to the stored client/registered URI.
  if (!await verifyOAuthQueryParams(query, secret)) throw new ServiceError("Invalid or expired authorization request. Start the connection again.", 400);
  const params = new URLSearchParams(query);
  if (params.getAll("client_id").length !== 1 || params.getAll("redirect_uri").length !== 1) throw new ServiceError("Ambiguous authorization request", 400);
  const id = params.get("client_id")!, uri = params.get("redirect_uri")!;
  const client = db.prepare('SELECT name,redirectUris,clientDiscoveryId FROM oauthClient WHERE clientId=? AND (disabled IS NULL OR disabled=0)').get(id) as { name: string | null; redirectUris: string; clientDiscoveryId: string | null } | undefined;
  if (!client) throw new ServiceError("Unknown OAuth client; start the connection again", 400);
  const redirect = new URL(uri);
  const registered = JSON.parse(client.redirectUris) as string[];
  // CIMD native loopback registrations intentionally omit the ephemeral port
  // (RFC 8252). All other registered components must still match exactly.
  const loopback = redirect.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(redirect.hostname);
  if (!registered.some(value => {
    if (value === uri) return true;
    if (!loopback) return false;
    const allowed = new URL(value); allowed.port = redirect.port;
    return allowed.toString() === redirect.toString();
  })) throw new ServiceError("Redirect URI is not registered for this client", 400);
  const identityHost = client.clientDiscoveryId === "cimd" && id.startsWith("https://") ? new URL(id).hostname : null;
  return { name: client.name ?? "OAuth client", identityHost, redirectHost: redirect.host || redirect.protocol, redirectUri: uri, loopback };
}
