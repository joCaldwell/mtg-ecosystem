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
