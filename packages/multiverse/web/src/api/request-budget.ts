export interface UiRequestBudget {
  count: number;
  window_ms: number;
}

interface RequestBudgetEffects {
  fetch: typeof fetch;
  now(): number;
  origin(): string | undefined;
  wait(milliseconds: number, signal: AbortSignal): Promise<void>;
}

interface ActiveBudget {
  readonly cancel: AbortController;
  readonly spacing: number;
  readonly windowMs: number;
  nextRequestAt: number;
  cooldownUntil: number;
}

/** Share dispatch capacity across product fetches during hosted play. The server
 * still accounts for all tabs and streams. A slot is charged before sending, and
 * a long poll does not hold up later requests. */
export function createGameRequestBudget(effects: RequestBudgetEffects) {
  let active: ActiveBudget | null = null;

  function begin(policy: UiRequestBudget | undefined): () => void {
    if (!policy || !Number.isSafeInteger(policy.count) || policy.count < 2
      || !Number.isSafeInteger(policy.window_ms) || policy.window_ms < 1) return () => {};
    // Leave room for traffic outside fetch, including EventSource reconnects.
    const pacedCount = Math.max(1, Math.floor(policy.count * 0.9));
    const budget: ActiveBudget = { cancel: new AbortController(), spacing: Math.ceil(policy.window_ms / pacedCount),
      windowMs: policy.window_ms, nextRequestAt: 0, cooldownUntil: 0 };
    active?.cancel.abort();
    active = budget;
    return () => {
      if (active === budget) active = null;
      budget.cancel.abort();
    };
  }

  const send: typeof fetch = async (input, init) => {
    const budget = active;
    const origin = effects.origin();
    if (!budget || !origin) return effects.fetch(input, init);
    const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url, origin);
    if (url.origin !== origin || !url.pathname.startsWith("/api/")) return effects.fetch(input, init);

    const requestSignal = init?.signal ?? (input instanceof Request ? input.signal : null);
    const signal = requestSignal ? AbortSignal.any([requestSignal, budget.cancel.signal]) : budget.cancel.signal;
    for (;;) {
      signal.throwIfAborted();
      const now = effects.now();
      if (now >= budget.nextRequestAt) {
        budget.nextRequestAt = now + budget.spacing;
        break;
      }
      await effects.wait(budget.nextRequestAt - now, signal);
    }
    const response = await effects.fetch(input, init);
    if (active === budget && response.status === 429 && response.headers.get("X-Multiverse-UI-Limit") === "1") {
      // The Actor limiter emits delta-seconds. Scope the hint to this page's
      // authenticated budget and leave the original response to its caller.
      const seconds = response.headers.get("Retry-After");
      if (seconds && /^\d+$/.test(seconds)) {
        const delay = Number(seconds) * 1_000;
        if (delay > 0 && delay <= budget.windowMs)
          budget.cooldownUntil = Math.max(budget.cooldownUntil, effects.now() + delay);
      }
    }
    return response;
  };

  async function waitForCooldown(signal: AbortSignal): Promise<void> {
    const budget = active;
    if (!budget) return;
    const waiting = AbortSignal.any([signal, budget.cancel.signal]);
    for (;;) {
      waiting.throwIfAborted();
      const remaining = budget.cooldownUntil - effects.now();
      if (remaining <= 0) return;
      await effects.wait(remaining, waiting);
    }
  }

  return { begin, send, waitForCooldown };
}

const requests = createGameRequestBudget({
  fetch: (input, init) => globalThis.fetch(input, init),
  now: () => performance.now(),
  origin: () => globalThis.location?.origin,
  wait(milliseconds, signal) {
    return new Promise<void>((resolve, reject) => {
      signal.throwIfAborted();
      const aborted = () => {
        clearTimeout(timer);
        reject(signal.reason);
      };
      const timer = setTimeout(() => {
        signal.removeEventListener("abort", aborted);
        resolve();
      }, milliseconds);
      signal.addEventListener("abort", aborted, { once: true });
    });
  },
});

export const productFetch = requests.send;
export const beginGameRequestBudget = requests.begin;
export const waitForGameRequestCooldown = requests.waitForCooldown;
