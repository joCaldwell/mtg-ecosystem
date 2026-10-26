import { createHash } from "node:crypto";
import { ipBucket } from "./mode.ts";
import { AppError } from "./errors.ts";
// Single service instance by design (v3 D12). These sliding windows are in
// memory and reset at restart; multiple replicas require a shared limiter.
export const LIMITS = {
  unauthenticated: { count: 120, window: 60_000 }, // shared discovery/static/health
  auth: { count: 20, window: 60_000 }, // sign-in, registration and token endpoints
  claim: { count: 5, window: 60_000 }, // unknown invite/owner-code attempts
  token: { count: 240, window: 60_000 }, // per OAuth client, shared connector egress
  ui: { count: 240, window: 60_000 }, // one human's UI/API
  mcp: { count: 240, window: 60_000 }, // one verified personal/OAuth credential
  search: { count: 60, window: 60_000 }, // expensive search
  anonymous_search: { count: 30, window: 60_000 }, // the same, per IP (/64), without an account (§18.6)
  compare: { count: 30, window: 60_000 }, // expensive deck comparisons
  import: { count: 20, window: 60_000 }, // pasted-list parses and diffs, preview and apply (§12)
} as const;
export class RateLimitError extends AppError {
  retryAfter: number;
  constructor(retryAfter: number) { super(`Slow down; retry in ${retryAfter} seconds`, 429); this.retryAfter = retryAfter; }
}
export function createRateLimiter(now = Date.now) {
  type Kind=keyof typeof LIMITS;
  const buckets = new Map<Kind,Map<string,number[]>>();
  return (kind: Kind, key: string) => {
    const time=now(),rule=LIMITS[kind];
    const group=buckets.get(kind)??new Map<string,number[]>();buckets.set(kind,group);
    const normalized=["unauthenticated","auth","claim","anonymous_search"].includes(kind)?ipBucket(key):key;
    const id=createHash("sha256").update(normalized).digest("hex");
    const hits=(group.get(id)??[]).filter(t=>t>time-rule.window);
    // Independent bounded LRU stores. Anonymous saturation never locks out a
    // new UI user or verified MCP credential. Evict old anonymous keys, don't
    // impose a global 60-second outage on every new principal.
    group.delete(id);
    if(group.size>=4096)group.delete(group.keys().next().value!);
    group.set(id,hits);
    if(hits.length>=rule.count)throw new RateLimitError(Math.max(1,Math.ceil((hits[0]+rule.window-time)/1000)));
    hits.push(time);
  };
}
