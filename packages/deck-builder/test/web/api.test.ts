import { test } from "node:test";
import assert from "node:assert/strict";
import { api, ApiError } from "../../web/src/api/client.ts";

test("ApiError retains HTTP status and server error text, including non-JSON failures", async t => {
  t.mock.method(globalThis, "fetch", async () => new Response(JSON.stringify({ error: "You do not have access" }), { status: 403, headers: { "Content-Type": "application/json" } }));
  await assert.rejects(api.getDeck(1), (error: unknown) => error instanceof ApiError && error.status === 403 && error.error === "You do not have access" && error.message === error.error);
  t.mock.method(globalThis, "fetch", async () => new Response("Bad gateway", { status: 502, headers: { "Content-Type": "text/html" } }));
  await assert.rejects(api.getDeck(1), (error: unknown) => error instanceof ApiError && error.status === 502);
  t.mock.method(globalThis, "fetch", async () => new Response("<html>not the API</html>", { headers: { "Content-Type": "text/html" } }));
  await assert.rejects(api.getDeck(1), /unexpected response/);
});

// The deck page marks rows with its own and the global memories (spec §7.2);
// the list endpoint caps a response at 100, so both scopes are paged through.
test("deckMemories asks for this deck's and the global memories and follows every page", async (t) => {
  const seen: string[] = [];
  const memory = (id: number, scope: string) => ({ id, scope, status: "active", deck_id: scope === "deck" ? 7 : null, cards: [], revised_at: `2026-10-${String(28 - Math.floor(id / 100)).padStart(2, "0")} 00:00:00` });
  t.mock.method(globalThis, "fetch", async (url: string) => {
    seen.push(url);
    const q = new URL(url, "http://x").searchParams;
    const scope = q.get("scope") ?? "deck";
    // 100 per page, as the server caps it; global has two pages, continued
    // from the first page's last row (keyset: before_id + before_revised_at).
    const page = scope === "deck" ? { memories: [memory(1, "deck")], has_more: false }
      : !q.has("before_id") ? { memories: Array.from({ length: 100 }, (_, i) => memory(199 - i, "global")), has_more: true }
      : { memories: [memory(99, "global"), memory(98, "global")], has_more: false };
    return new Response(JSON.stringify({ ...page, scopes: [], budgets: {}, global_budget: {} }), { headers: { "Content-Type": "application/json" } });
  });
  const result = await api.deckMemories(7);
  assert.equal(seen.length, 3);
  assert.equal(seen[0], "/api/memories?deck=7");
  assert.equal(seen[1], "/api/memories?scope=global");
  const next = new URL(seen[2], "http://x").searchParams;
  assert.equal(next.get("scope"), "global");
  assert.equal(next.get("before_id"), "100", "continues from the previous page's last row");
  assert.equal(next.get("before_revised_at"), "2026-10-27 00:00:00");
  assert.equal(result.length, 103, "every row once");
  assert.ok(result.some(m => m.id === 98), "the page past 100 is included");
});
