import assert from "node:assert/strict";
import test from "node:test";
import { createGameRequestBudget } from "../web/src/api/request-budget.ts";

const policy = { count: 240, window_ms: 60_000 };
const origin = "http://127.0.0.1:8080";

function fakeTime(send: typeof fetch) {
  let now = 0;
  const timers: { at: number; done(): void }[] = [];
  const budget = createGameRequestBudget({
    fetch: send,
    origin: () => origin,
    now: () => now,
    wait(milliseconds, signal) {
      return new Promise<void>((resolve, reject) => {
        const aborted = () => reject(signal.reason);
        signal.addEventListener("abort", aborted, { once: true });
        timers.push({ at: now + milliseconds, done() {
          signal.removeEventListener("abort", aborted);
          resolve();
        } });
      });
    },
  });
  async function advance(milliseconds: number) {
    now += milliseconds;
    for (const timer of timers.filter(timer => timer.at <= now)) {
      timers.splice(timers.indexOf(timer), 1);
      timer.done();
    }
    await Promise.resolve();
  }
  return { budget, advance };
}

test("game and account requests share dispatch slots while an earlier long poll stays open", async () => {
  const calls: unknown[] = [];
  let finishPoll!: (response: Response) => void;
  const poll = new Promise<Response>(resolve => { finishPoll = resolve; });
  const { budget, advance } = fakeTime(async (input, init) => {
    calls.push([input, init]);
    return calls.length === 1 ? poll : new Response();
  });
  const release = budget.begin(policy);
  const events = budget.send(`${origin}/api/games/01/native/v1/matches/01/events`);
  const controller = new AbortController();
  const cancelled = budget.send("/api/focus", { signal: controller.signal });
  const body = '{"frozen":"command bytes"}';
  const pending = budget.send("/api/games/01/native/v1/matches/01/commands", { method: "POST", body });
  const assertCancelled = assert.rejects(cancelled, { name: "AbortError" });
  controller.abort();
  await assertCancelled;
  await advance(277);
  assert.equal(calls.length, 1);
  await advance(1);
  await pending;
  assert.deepEqual(calls[1], ["/api/games/01/native/v1/matches/01/commands", { method: "POST", body }]);

  const inbox = budget.send("/api/inbox");
  const cancel = budget.send("/api/games/01/native/v1/matches/01/actions/query", { method: "POST", body: '{"operation":"cancel","cursor":"original"}' });
  const owner = new AbortController();
  const stale = budget.send("/api/games/01", { signal: owner.signal });
  const assertStale = assert.rejects(stale, { name: "AbortError" });
  release();
  owner.abort();
  await Promise.all([inbox, cancel]);
  await assertStale;
  finishPoll(new Response());
  await events;
  assert.deepEqual(calls.slice(2), [
    ["/api/inbox", undefined],
    ["/api/games/01/native/v1/matches/01/actions/query", { method: "POST", body: '{"operation":"cancel","cursor":"original"}' }],
  ]);
});

test("only an Actor-budget 429 defers background reads within its active game", async () => {
  let requests = 0;
  const { budget, advance } = fakeTime(async () => {
    return new Response(null, { status: 429, headers: { "Retry-After": "3",
      ...(++requests === 2 ? { "X-Multiverse-UI-Limit": "1" } : {}) } });
  });
  const release = budget.begin(policy);
  const signal = new AbortController().signal;
  await budget.send("/api/search");
  await budget.waitForCooldown(signal); // A search-specific limit has no Actor hint.

  const response = budget.send("/api/games/01/native/v1/matches/01/manifest");
  await advance(278);
  assert.equal((await response).status, 429);
  let cooled = false;
  const waiting = budget.waitForCooldown(signal).then(() => { cooled = true; });
  await advance(2_999);
  assert.equal(cooled, false);
  await advance(1);
  await waiting;
  release();
  const releaseReplacement = budget.begin(policy);
  await budget.waitForCooldown(signal);
  releaseReplacement();
});
