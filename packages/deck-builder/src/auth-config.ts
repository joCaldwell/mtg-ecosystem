// Pinned docs: https://github.com/better-auth/better-auth/blob/v1.7.13/docs/content/docs/plugins/mcp.mdx
// and adapters/sqlite.mdx. The native driver shares our connection; schema is
// generated offline and applied ONLY by db.ts, never by Better Auth at runtime.
import { betterAuth, type BetterAuthOptions } from "better-auth";
import { jwt } from "better-auth/plugins";
import { mcp } from "@better-auth/mcp";
import { cimd } from "@better-auth/cimd";
import { fetchClientMetadata } from "./cimd-transport.ts";
import type { Database } from "./sqlite.ts";

export function authConfiguration(db: Database, origin: string, env = process.env): BetterAuthOptions {
  return {
    database: db,
    baseURL: `${origin}/api/auth`,
    secret: env.BETTER_AUTH_SECRET,
    trustedOrigins: [origin],
    socialProviders: {
      ...(env.DISCORD_CLIENT_ID && env.DISCORD_CLIENT_SECRET ? {
        discord: { clientId: env.DISCORD_CLIENT_ID, clientSecret: env.DISCORD_CLIENT_SECRET, mapProfileToUser: p => ({ signupHandle: p.username }) },
      } : {}),
      ...(env.GOOGLE_CLIENT_ID && env.GOOGLE_CLIENT_SECRET ? {
        google: { clientId: env.GOOGLE_CLIENT_ID, clientSecret: env.GOOGLE_CLIENT_SECRET },
      } : {}),
    },
    user: { additionalFields: { signupHandle: { type: "string", required: false, input: false } } },
    account: { accountLinking: { enabled: false }, encryptOAuthTokens: true },
    session: { cookieCache: { enabled: false } },
    // Only our consent-bound OAuth access tokens may authorize MCP. Disable
    // the JWT plugin's generic session-token endpoint and BA's own deletion.
    disabledPaths: ["/token", "/delete-user", "/update-user"],
    advanced: {
      useSecureCookies: origin.startsWith("https:"),
      defaultCookieAttributes: { httpOnly: true, sameSite: "lax", secure: origin.startsWith("https:") },
      // The app checks the trusted ingress itself. Never let a forwarded header
      // select Better Auth's base URL or bypass our IP policy.
      ipAddress: { ipAddressHeaders: [] },
    },
    logger: { disabled: true }, // never log provider responses or credentials
    rateLimit: { enabled: false }, // app-owned, single-instance limits in ratelimit.ts
    plugins: [jwt(), mcp({
      resource: `${origin}/mcp`, loginPage: "/signin", consentPage: "/consent",
      scopes: ["deck-builder", "offline_access"],
      grantTypes: ["authorization_code", "refresh_token"],
      allowDynamicClientRegistration: true, allowUnauthenticatedClientRegistration: true,
      clientRegistrationDefaultApplicationType: "infer",
      clientRegistrationRequirePKCE: true,
      refreshTokenReuseInterval: 0,
      clientPrivileges: async () => false,
    }), cimd({ fetchClientMetadataResource: fetchClientMetadata, metadataProfile: "mcp-2026-07-28" })],
  };
}
// Retain inference for the application, while the schema CLI reads the same configuration.
export function configureAuth(options: BetterAuthOptions) { return betterAuth(options); }
