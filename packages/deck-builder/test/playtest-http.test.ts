import { test } from "node:test";
import assert from "node:assert/strict";
import type { AddressInfo } from "node:net";
import { randomUUID } from "node:crypto";
import { createApp } from "../src/app.ts";
import { add, newDeck, testDb } from "./helpers.ts";
test("playtest HTTP workflows return persisted composites, validate actions and isolate deck writes", async (t) => {
  const db = testDb(),
    deck = newDeck(db);
  add(db, deck, "id-forest", { quantity: 20 });
  const app = createApp(db, {});
  await new Promise<void>((resolve) =>
    app.server.listen(0, "127.0.0.1", resolve),
  );
  t.after(() => {
    app.close();
    db.close();
  });
  const base = `http://127.0.0.1:${(app.server.address() as AddressInfo).port}/api/decks/${deck}/playtest`;
  const call = async (path = "", method = "GET", body?: unknown) => {
    const r = await fetch(base + path, {
      method,
      headers: { "Content-Type": "application/json" },
      body: body ? JSON.stringify(body) : undefined,
    });
    return { status: r.status, data: await r.json() };
  };
  assert.equal((await call()).data.playtest, null);
  assert.equal((await call("/actions", "POST", null)).status, 400);
  assert.equal((await call("/ui", "PATCH", null)).status, 400);
  let r = await call("", "POST", { mode: "commander" });
  assert.equal(r.status, 200);
  let view = r.data.playtest;
  r = await call("/actions", "POST", {
    session_id: view.session.id,
    expected_version: view.session.version,
    action: { type: "keep" },
  });
  assert.equal(r.status, 200);
  view = r.data.playtest;
  r = await call("/actions", "POST", {
    session_id: view.session.id,
    expected_version: 0,
    action: { type: "draw", count: 1 },
  });
  assert.equal(r.status, 409);
  r = await call("/actions", "POST", {
    session_id: view.session.id,
    expected_version: view.session.version,
    action: { type: "move", ids: ["missing"], zone: "hand" },
  });
  assert.equal(r.status, 404);
  r = await call("/ui", "PATCH", {
    session_id: view.session.id,
    expected_version: view.session.version,
    ui: {
      draft: { request_id: randomUUID(), text: "HTTP draft", oracle_id: null },
    },
  });
  assert.equal(r.status, 200);
  view = r.data.playtest;
  assert.deepEqual((await call()).data.playtest, view);
  r = await call("/observations", "POST", {
    ...view.session.ui.draft,
    session_id: view.session.id,
  });
  assert.equal(r.status, 200);
  assert.equal(r.data.memory.deck_revision, view.session.deck_revision);
  assert.equal(
    (
      db
        .prepare("SELECT quantity FROM deck_cards WHERE deck_id = ?")
        .get(deck) as { quantity: number }
    ).quantity,
    20,
  );
  const hostile = await fetch(base, {
    headers: { Origin: "https://example.com" },
  });
  assert.equal(hostile.status, 403);
});
