import { createHash } from "node:crypto";
import { AppError } from "./errors.ts";
// Single service instance by design (v3 D12). These sliding windows are in
// memory and reset at restart; multiple replicas require a shared limiter.
export const LIMITS = {
  unauthenticated: { count: 120, window: 60_000 }, // shared discovery/static/health
  auth: { count: 20, window: 60_000 }, // sign-in, registration and token endpoints
  ui: { count: 240, window: 60_000 }, // one human's UI/API
  mcp: { count: 240, window: 60_000 }, // one verified personal/OAuth credential
  search: { count: 60, window: 60_000 }, // expensive search
  compare: { count: 30, window: 60_000 }, // expensive deck comparisons
} as const;
export class RateLimitError extends AppError {
  retryAfter: number;
  constructor(retryAfter: number) { super(`Slow down; retry in ${retryAfter} seconds`, 429); this.retryAfter = retryAfter; }
}
export function createRateLimiter(now = Date.now) {
  const buckets = new Map<string, number[]>();
  return (kind: keyof typeof LIMITS, key: string) => {
    const time = now(), rule = LIMITS[kind];
    // Hash credential keys: never retain raw bearer tokens in limiter state.
    const id = kind + ":" + createHash("sha256").update(key).digest("hex");
    if (buckets.size >= 10_000) for (const [k,v] of buckets) if (v.at(-1)! <= time - 60_000) buckets.delete(k);
    // Bound memory for unique-IP floods as well as expired buckets.
    if (!buckets.has(id) && buckets.size >= 10_000) throw new RateLimitError(60);
    const hits = (buckets.get(id) ?? []).filter(t => t > time - rule.window);
    if (hits.length >= rule.count) throw new RateLimitError(Math.max(1, Math.ceil((hits[0] + rule.window - time) / 1000)));
    hits.push(time); buckets.set(id, hits);
  };
}
