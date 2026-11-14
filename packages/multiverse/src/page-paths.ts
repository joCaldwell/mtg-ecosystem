/** Product-owned production SPA surface. API/MCP/auth transports remain backend
 * routes; unknown paths receive 404 rather than an arbitrary HTML fallback. */
export function isApplicationPath(pathname: string): boolean {
  return /^\/(?:play(?:\/preview)?|decks|inbox|memories|profile|activity|settings|design|signin|consent|privacy|terms|deck\/[^/]+(?:\/(?:review|playtest))?|[du]\/[^/]+)?$/.test(pathname);
}
