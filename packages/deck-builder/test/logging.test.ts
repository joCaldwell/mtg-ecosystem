import { test, mock } from "node:test";
import assert from "node:assert/strict";
import type { AddressInfo } from "node:net";
import { Client, StreamableHTTPClientTransport } from "@modelcontextprotocol/client";
import { createApp } from "../src/app.ts";
import { createToken } from "../src/tokens.ts";
import { OWNER } from "./actors.ts";
import { testDb } from "./helpers.ts";
import { logging } from "../src/logging.ts";

test("hosted error formatting removes multiline messages that imitate frames", () => {
  const error = Object.assign(new Error("SECRET_MESSAGE\n    at SECRET_FRAME"), { code: "EACCES" });
  const detail = logging.describeError(error);
  assert.equal(detail.code, "EACCES");
  assert.ok(!JSON.stringify(detail).includes("SECRET_"));
  assert.match(detail.stack, /at /);
});

test("request/tool JSON logs correlate resolved principals without credentials, queries, bodies or results", async () => {
  const db = testDb();
  const token = createToken(db, OWNER, "logging-test").token;
  const lines: string[] = [];
  const originalWrite = process.stdout.write.bind(process.stdout);
  const capture = mock.method(process.stdout, "write", (chunk: any, ...args: any[]) => {
    if (typeof chunk !== "string" || !chunk.startsWith('{"ts":')) return (originalWrite as any)(chunk, ...args);
    lines.push(chunk); return true;
  });
  const app = createApp(db, { resolveUiActor: async req => {
    if (req.headers["x-test-user"] === "2") {
      await new Promise(resolve => setTimeout(resolve, 20));
      return { user_id: 2, via: "ui", label: "ui" };
    }
    return OWNER;
  } });
  const clients: Client[] = [];
  try {
    await new Promise<void>(resolve => app.server.listen(0, "127.0.0.1", resolve));
    const base = `http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
    await (await fetch(`${base}/api/health`)).text();
    // These paths do not need real user-2 records; the 404 is logged for the
    // resolved principal while a concurrent user-1 request completes first.
    const responses = await Promise.all([
      fetch(`${base}/api/not-a-route?code=QUERY_SECRET`, { headers: { "x-test-user": "2", Cookie: "COOKIE_SECRET" } }),
      fetch(`${base}/api/not-a-route`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ secret: "BODY_SECRET" }) }),
    ]);
    await Promise.all(responses.map(r => r.text()));
    const client = new Client({ name: "log-client", version: "1" }, { versionNegotiation: { mode: { pin: "2026-07-28" } } });
    clients.push(client);
    await client.connect(new StreamableHTTPClientTransport(new URL(`${base}/mcp/log-client?state=PATH_SECRET`), { requestInit: { headers: { Authorization: `Bearer ${token}` } } }));
    await client.callTool({ name: "start_session", arguments: {} });
    await client.callTool({ name: "get_deck", arguments: { deck: "ARGUMENT_SECRET" } });
    // node --test can also write binary harness messages to stdout.
    const entries = lines.flatMap(line => line.trim().split("\n")).filter(line => line.startsWith('{"ts":')).map(line => JSON.parse(line));
    const requests = entries.filter(e => e.event === "request");
    assert.ok(!requests.some(e => e.path === "/api/health"));
    assert.ok(requests.some(e => e.user_id === 1 && e.method === "POST" && e.status === 404));
    assert.ok(requests.some(e => e.user_id === 2 && e.method === "GET" && e.status === 404));
    assert.equal(new Set(requests.map(e => e.req_id)).size, requests.length);
    for (const entry of entries.filter(e => e.event === "tool")) {
      const request = requests.find(r => r.req_id === entry.req_id);
      assert.ok(request, "tool must correlate with an HTTP request");
      assert.equal(request.user_id, entry.user_id);
      assert.equal(entry.via, "agent");
      assert.ok(entry.ms >= 0);
      assert.ok(Number.isFinite(Date.parse(entry.ts)));
    }
    assert.ok(entries.some(e => e.tool === "start_session" && e.status === 200));
    assert.ok(entries.some(e => e.tool === "get_deck" && e.status === 404 && e.level === "warn" && !e.error.stack));
    for (const secret of [token, "QUERY_SECRET", "COOKIE_SECRET", "BODY_SECRET", "PATH_SECRET", "ARGUMENT_SECRET", "# Playbook"])
      assert.ok(!lines.join("").includes(secret), `logs disclosed ${secret.slice(0, 8)}`);
    assert.ok(responses.every(r => r.headers.get("x-request-id")));
  } finally {
    await Promise.all(clients.map(c => c.close()));
    await app.close();
    db.close();
    capture.mock.restore();
  }
});

// Phase 4a: an unlisted deck's slug is a capability URL, and a handle names a
// person; neither reaches the logs, and a signed-out request is "anonymous".
import { setVisibility } from "../src/sharing.ts";
import { newDeck, add } from "./helpers.ts";
test("shared pages and public routes log :value for slugs and handles, and anonymous for the principal", async () => {
  const db = testDb();
  const deck = newDeck(db, "paper", "Logged deck");
  add(db, deck, "id-forest");
  const { slug } = setVisibility(db, OWNER, deck, "unlisted");
  const lines: string[] = [];
  const originalWrite = process.stdout.write.bind(process.stdout);
  const capture = mock.method(process.stdout, "write", (chunk: any, ...args: any[]) => {
    if (typeof chunk !== "string" || !chunk.startsWith('{"ts":')) return (originalWrite as any)(chunk, ...args);
    lines.push(chunk); return true;
  });
  const app = createApp(db, { resolveUiActor: () => null });
  try {
    await new Promise<void>(resolve => app.server.listen(0, "127.0.0.1", resolve));
    const base = `http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
    for (const path of [`/d/${slug}`, `/d/${slug}/`, "/u/owner", `/api/public/decks/${slug}`, `/api/public/decks/${slug}/export`, "/api/public/users/owner", "/api/search?q=SEARCH_SECRET"])
      await (await fetch(base + path, { headers: { "X-Real-IP": "203.0.113.9" } })).text();
    const requests = lines.flatMap(l => l.trim().split("\n")).filter(l => l.startsWith('{"ts":')).map(l => JSON.parse(l)).filter(e => e.event === "request");
    assert.deepEqual(requests.map(e => e.path), ["/d/:value", "/d/:value", "/u/:value", "/api/public/decks/:value", "/api/public/decks/:value/export", "/api/public/users/:value", "/api/search"]);
    assert.ok(requests.every(e => e.label === "anonymous" && e.user_id === null), "a signed-out request is anonymous");
    for (const secret of [slug!, "owner", "SEARCH_SECRET", "203.0.113.9", "127.0.0.1"])
      assert.ok(!lines.join("").includes(secret), `logs disclosed ${secret}`);
  } finally {
    await app.close();
    db.close();
    capture.mock.restore();
  }
});
