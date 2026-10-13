import { test, type TestContext } from "node:test";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { OWNER } from "../src/actor.ts";
import { openDb } from "../src/db.ts";
import {
  deleteDeck,
  getDeck,
  organizeCard,
  shortlistCard,
  createTag,
} from "../src/deck/service.ts";
import {
  changePlaytest,
  getPlaytest,
  saveObservation,
  savePlaytestUi,
  startPlaytest,
} from "../src/playtest/service.ts";
import type { PlaytestAction } from "../src/playtest/service.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";
function setup(t: TestContext) {
  const db = testDb();
  t.after(() => db.close());
  const deck = newDeck(db);
  add(db, deck, "id-atraxa", { role: "commander" });
  add(db, deck, "id-teferi", { role: "commander" });
  add(db, deck, "id-llanowar", { role: "companion" });
  add(db, deck, "id-forest", { quantity: 40 });
  add(db, deck, "id-witch");
  add(db, deck, "id-solring");
  shortlistCard(db, deck, "id-counterspell");
  let current = startPlaytest(db, OWNER, deck, { mode: "commander" });
  const act = (action: PlaytestAction) =>
    (current = changePlaytest(db, OWNER, deck, {
      session_id: current.session.id,
      expected_version: current.session.version,
      action,
    }));
  return {
    db,
    deck,
    act,
    get v() {
      return current;
    },
  };
}
test("snapshot separates commanders, companion and maybe; preserves quantities and verified faces", (t) => {
  const x = setup(t);
  assert.equal(x.v.computed.counts.command, 2);
  assert.equal(x.v.computed.counts.companion, 1);
  assert.equal(x.v.computed.counts.hand, 7);
  assert.equal(x.v.computed.counts.library, 35);
  assert.equal(x.v.session.frame.life, 40);
  assert.equal(x.v.session.cards["id-witch"].faces.length, 2);
  assert.equal(x.v.session.cards["id-counterspell"], undefined);
  const snapshot = structuredClone(x.v.session);
  cut(x.db, x.deck, "id-solring");
  assert.deepEqual(getPlaytest(x.db, OWNER, x.deck)?.session, snapshot);
  assert.ok(getDeck(x.db, x.deck).deck.revision > snapshot.deck_revision);
  assert.throws(
    () => startPlaytest(x.db, OWNER, x.deck, { mode: "commander" }),
    /Resume|confirm/,
  );
});
test("Commander mulligans have a free first, ordered bottom choices and a forced stopping limit", (t) => {
  const x = setup(t);
  x.act({ type: "mulligan" });
  assert.equal(x.v.computed.bottom_required, 0);
  x.act({ type: "mulligan" });
  x.act({ type: "mulligan" });
  assert.equal(x.v.computed.bottom_required, 2);
  assert.throws(() => x.act({ type: "keep" }), /required/);
  const bottom = x.v.session.frame.zones.hand.slice(0, 2).reverse();
  x.act({ type: "bottom", ids: bottom });
  x.act({ type: "keep" });
  assert.deepEqual(x.v.session.frame.zones.library.slice(-2), bottom);
  assert.equal(x.v.computed.counts.hand, 5);
  assert.equal(x.v.session.frame.turn, 1);
  x.act({ type: "undo" });
  assert.equal(x.v.session.frame.opening.stage, "choosing");
  assert.deepEqual(x.v.session.frame.opening.bottom, bottom);
  x.act({ type: "bottom", ids: [] });
  while (x.v.computed.can_mulligan) x.act({ type: "mulligan" });
  assert.equal(x.v.session.frame.opening.mulligans, 8);
  assert.equal(x.v.computed.bottom_required, 7);
  assert.throws(() => x.act({ type: "mulligan" }), /limit/);
  x.act({ type: "bottom", ids: [...x.v.session.frame.zones.hand] });
  x.act({ type: "keep" });
  assert.equal(x.v.computed.counts.hand, 0);
});
test("practice redraws are unlimited and changing mode starts a clean sequence", (t) => {
  const x = setup(t);
  x.act({ type: "mulligan" });
  x.act({ type: "mode", mode: "practice" });
  assert.equal(x.v.session.frame.opening.mulligans, 0);
  for (let i = 0; i < 12; i++) x.act({ type: "mulligan" });
  assert.equal(x.v.computed.bottom_required, 0);
  assert.equal(x.v.computed.counts.hand, 7);
  x.act({ type: "mode", mode: "commander" });
  assert.equal(x.v.session.frame.opening.mulligans, 0);
  x.act({ type: "keep" });
  assert.throws(() => x.act({ type: "mulligan" }), /new opening/);
});
test("next turn untaps without drawing; exact shuffle undo and redo survive reload", (t) => {
  const x = setup(t);
  x.act({ type: "keep" });
  const ids = x.v.session.frame.zones.hand.slice(0, 2);
  x.act({ type: "move", ids, zone: "battlefield", row: "lands" });
  x.act({ type: "tap", ids, tapped: true });
  const before = structuredClone(x.v.session.frame);
  x.act({ type: "next-turn" });
  assert.deepEqual(x.v.session.frame.zones, before.zones);
  assert.equal(x.v.session.frame.turn, 2);
  assert.ok(ids.every((id) => !x.v.session.frame.objects[id].tapped));
  x.act({ type: "undo" });
  assert.deepEqual(x.v.session.frame, before);
  x.act({ type: "redo" });
  const order = [...x.v.session.frame.zones.library];
  x.act({ type: "shuffle" });
  const shuffled = [...x.v.session.frame.zones.library];
  x.act({ type: "undo" });
  assert.deepEqual(x.v.session.frame.zones.library, order);
  x.act({ type: "redo" });
  assert.deepEqual(x.v.session.frame.zones.library, shuffled);
  assert.deepEqual(getPlaytest(x.db, OWNER, x.deck), x.v);
});
test("instance state, attachment cycles and host movement are reversible; batch order stays explicit", (t) => {
  const x = setup(t);
  x.act({ type: "keep" });
  const [a, b] = x.v.session.frame.zones.hand;
  x.act({ type: "move", ids: [a, b], zone: "battlefield" });
  x.act({ type: "attach", id: a, host: b });
  assert.throws(() => x.act({ type: "attach", id: b, host: a }), /cycle/);
  x.act({ type: "counter", id: a, name: "+1/+1", delta: 3 });
  x.act({ type: "edit", id: a, power: "*", toughness: "4" });
  x.act({ type: "token", copy_id: a });
  const token = x.v.session.frame.zones.battlefield.at(-1)!;
  x.act({ type: "counter", id: token, name: "+1/+1", delta: -1 });
  assert.equal(x.v.session.frame.objects[a].counters["+1/+1"], 3);
  assert.equal(x.v.session.frame.objects[token].counters["+1/+1"], 2);
  assert.equal(x.v.session.frame.objects[token].attached_to, null);
  x.act({ type: "move", ids: [b], zone: "graveyard" });
  assert.equal(x.v.session.frame.objects[a].attached_to, null);
  x.act({ type: "undo" });
  assert.equal(x.v.session.frame.objects[a].attached_to, b);
  x.act({ type: "move", ids: [b, a], zone: "library", index: 0 });
  assert.deepEqual(x.v.session.frame.zones.library.slice(0, 2), [b, a]);
  x.act({ type: "remove-token", ids: [token] });
  assert.equal(x.v.session.frame.objects[token], undefined);
  x.act({ type: "undo" });
  assert.ok(x.v.session.frame.objects[token]);
});
test("faces and manual values use verified card facts and invalid actions are atomic", (t) => {
  const x = setup(t);
  x.act({ type: "keep" });
  x.act({ type: "token", oracle_id: "id-witch" });
  const id = x.v.session.frame.zones.battlefield.at(-1)!;
  x.act({ type: "edit", id, face: 1, power: "2", toughness: "3" });
  const before = structuredClone(x.v);
  assert.throws(() => x.act({ type: "edit", id, face: 2 }), /face/);
  assert.deepEqual(getPlaytest(x.db, OWNER, x.deck), before);
  assert.throws(
    () => x.act({ type: "token", oracle_id: "unknown" }),
    /database/,
  );
  assert.throws(() => x.act({ type: "draw", count: -1 }), /Too small/);
});
test("UI draft and selection survive game undo; observation records tested revision and retries are idempotent", (t) => {
  const x = setup(t);
  x.act({ type: "keep" });
  const oracleId = "id-solring",
    id = x.v.session.frame.zones.hand[0],
    requestId = randomUUID();
  const draft = {
    text: "The opening sequence needed an additional redraw.",
    oracle_id: oracleId,
    request_id: requestId,
  };
  const ui = savePlaytestUi(x.db, OWNER, x.deck, {
    session_id: x.v.session.id,
    expected_version: x.v.session.version,
    ui: { draft, selected: [id], pinned: id, viewer: "library" },
  });
  const changed = changePlaytest(x.db, OWNER, x.deck, {
    session_id: ui.session.id,
    expected_version: ui.session.version,
    action: { type: "undo" },
  });
  assert.deepEqual(changed.session.ui.draft, {
    ...draft,
    deck_revision: ui.session.deck_revision,
  });
  assert.deepEqual(changed.session.ui.selected, [id]);
  cut(x.db, x.deck, oracleId);
  const input = { ...draft, session_id: ui.session.id };
  const memory = saveObservation(x.db, OWNER, x.deck, input);
  assert.equal(memory.deck_revision, ui.session.deck_revision);
  assert.equal(memory.author, "owner");
  assert.ok(memory.owner_touched);
  assert.equal(memory.cards[0].oracle_id, oracleId);
  assert.equal(saveObservation(x.db, OWNER, x.deck, input).id, memory.id);
  assert.throws(
    () =>
      saveObservation(x.db, OWNER, x.deck, { ...input, text: "Changed text" }),
    /different text/,
  );
  startPlaytest(x.db, OWNER, x.deck, {
    mode: "practice",
    replace: true,
    session_id: changed.session.id,
    expected_version: changed.session.version,
  });
  assert.equal(
    (x.db.prepare("SELECT COUNT(*) n FROM memories").get() as { n: number }).n,
    1,
  );
});
test("tags remain outside game undo and owner permission / stale-window conflicts are enforced below HTTP", (t) => {
  const x = setup(t);
  x.act({ type: "keep" });
  const tag = createTag(x.db, x.deck, "early setup");
  organizeCard(x.db, x.deck, "id-forest", { tagIds: [tag] });
  x.act({ type: "undo" });
  assert.deepEqual(
    getDeck(x.db, x.deck).cards.find((c) => c.oracle_id === "id-forest")
      ?.tag_ids,
    [tag],
  );
  assert.throws(() => getPlaytest(x.db, AGENT, x.deck), /owner|Only you/);
  assert.throws(
    () => startPlaytest(x.db, AGENT, x.deck, { mode: "commander" }),
    /owner|Only you/,
  );
  assert.throws(
    () =>
      changePlaytest(x.db, OWNER, x.deck, {
        session_id: x.v.session.id,
        expected_version: x.v.session.version - 1,
        action: { type: "keep" },
      }),
    /another window/,
  );
  assert.throws(
    () =>
      savePlaytestUi(x.db, OWNER, x.deck, {
        session_id: x.v.session.id,
        expected_version: x.v.session.version,
        ui: { selected: ["missing"] },
      }),
    /no longer/,
  );
  deleteDeck(x.db, OWNER, x.deck);
  assert.equal(
    (
      x.db.prepare("SELECT COUNT(*) n FROM playtest_sessions").get() as {
        n: number;
      }
    ).n,
    0,
  );
});
test("schema 6 upgrades additively; table resumes from disk and corrupt payloads never silently shuffle", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "playtest-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const path = join(dir, "test.db");
  const db = openDb(path);
  db.exec(
    "UPDATE meta SET value = '6' WHERE key = 'schema_version'; DROP TABLE playtest_sessions; DROP TABLE playtest_observations;",
  );
  db.close();
  const migrated = openDb(path);
  assert.ok(
    migrated
      .prepare(
        "SELECT name FROM sqlite_master WHERE name = 'playtest_sessions'",
      )
      .get(),
  );
  const x = setup(t),
    snapshot = x.v.session;
  migrated.close();
  const live = openDb(path);
  const deck = newDeck(live);
  snapshot.deck_id = deck;
  live
    .prepare(
      "INSERT INTO playtest_sessions (deck_id, payload_json) VALUES (?, ?)",
    )
    .run(deck, JSON.stringify(snapshot));
  live.close();
  const reopened = openDb(path);
  assert.deepEqual(getPlaytest(reopened, OWNER, deck)?.session, snapshot);
  reopened
    .prepare("UPDATE playtest_sessions SET payload_json = ? WHERE deck_id = ?")
    .run("broken", deck);
  assert.throws(() => getPlaytest(reopened, OWNER, deck), /preserved/);
  assert.equal(
    (
      reopened
        .prepare("SELECT payload_json FROM playtest_sessions WHERE deck_id = ?")
        .get(deck) as { payload_json: string }
    ).payload_json,
    "broken",
  );
  reopened.close();
});

test("restart retains unfinished card observations with their original tested revision", (t) => {
  const x = setup(t),
    id = "id-solring",
    revision = x.v.session.deck_revision;
  const draft = {
    text: "Still considering this card in the prior opening.",
    oracle_id: id,
    request_id: randomUUID(),
    deck_revision: revision,
  };
  const saved = savePlaytestUi(x.db, OWNER, x.deck, {
    session_id: x.v.session.id,
    expected_version: x.v.session.version,
    ui: { draft },
  });
  cut(x.db, x.deck, id);
  const next = startPlaytest(x.db, OWNER, x.deck, {
    mode: "practice",
    replace: true,
    session_id: saved.session.id,
    expected_version: saved.session.version,
  });
  assert.ok(next.session.deck_revision > revision);
  assert.deepEqual(next.session.ui.draft, draft);
  assert.ok(next.session.cards[id]);
  assert.ok(
    !Object.values(next.session.frame.objects).some((o) => o.oracle_id === id),
  );
  const memory = saveObservation(x.db, OWNER, x.deck, {
    ...draft,
    session_id: next.session.id,
  });
  assert.equal(memory.deck_revision, revision);
});

test("opening face inspection is allowed without enabling game actions", (t) => {
  const x = setup(t),
    id = Object.values(x.v.session.frame.objects).find(
      (o) => o.oracle_id === "id-witch",
    )!.id;
  x.act({ type: "edit", id, face: 1 });
  assert.equal(x.v.session.frame.objects[id].face, 1);
  assert.equal(x.v.session.frame.objects[id].row, "creatures");
  assert.throws(() => x.act({ type: "draw", count: 1 }), /Keep an opening/);
  assert.throws(
    () => x.act({ type: "edit", id, face: 1, power: "3" }),
    /Keep an opening/,
  );
});
