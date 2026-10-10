import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { groupSessions } from "../../web/src/domain/activity.ts";
import { activity } from "./fixtures.ts";

const at = (time: string) => `2026-03-02 ${time}`;

describe("groupSessions", () => {
  test("consecutive entries by one actor inside the gap are one session, entries oldest first", () => {
    const feed = [
      activity({ id: 3, ts: at("10:40:00") }),
      activity({ id: 2, ts: at("10:20:00") }),
      activity({ id: 1, ts: at("10:00:00") }),
    ];
    const [session, ...rest] = groupSessions(feed);
    assert.equal(rest.length, 0);
    assert.deepEqual(session.entries.map((e) => e.id), [1, 2, 3]);
    assert.equal(session.start, at("10:00:00"));
    assert.equal(session.end, at("10:40:00"));
    assert.equal(session.key, "test-agent:1");
  });

  test("a gap of the limit or more splits; just under does not", () => {
    const split = groupSessions([activity({ id: 2, ts: at("10:30:00") }), activity({ id: 1, ts: at("10:00:00") })]);
    assert.equal(split.length, 2, "exactly 30 minutes apart is a new session");
    const joined = groupSessions([activity({ id: 2, ts: at("10:29:59") }), activity({ id: 1, ts: at("10:00:00") })]);
    assert.equal(joined.length, 1);
  });

  test("the gap is measured between neighbours, so a long run stays one session", () => {
    const feed = [at("11:20:00"), at("10:50:01"), at("10:25:00"), at("10:00:00")].map((ts, i) =>
      activity({ id: 4 - i, ts }),
    );
    assert.equal(groupSessions(feed).length, 1);
  });

  test("a different actor splits, and the same actor returning is a new session", () => {
    const feed = [
      activity({ id: 3, actor: "agent-a", ts: at("10:02:00") }),
      activity({ id: 2, actor: "agent-b", ts: at("10:01:00") }),
      activity({ id: 1, actor: "agent-a", ts: at("10:00:00") }),
    ];
    const sessions = groupSessions(feed);
    assert.deepEqual(sessions.map((s) => s.actor), ["agent-a", "agent-b", "agent-a"]);
  });

  test("sessions come back newest first", () => {
    const feed = [
      activity({ id: 2, ts: at("12:00:00") }),
      activity({ id: 1, ts: at("09:00:00") }),
    ];
    const sessions = groupSessions(feed);
    assert.deepEqual(sessions.map((s) => s.entries[0].id), [2, 1]);
  });

  test("a custom gap is honoured", () => {
    const feed = [activity({ id: 2, ts: at("10:10:00") }), activity({ id: 1, ts: at("10:00:00") })];
    assert.equal(groupSessions(feed, 5).length, 2);
    assert.equal(groupSessions(feed, 15).length, 1);
  });

  test("deck names are distinct, oldest first, and skip entries with no deck", () => {
    const feed = [
      activity({ id: 4, ts: at("10:03:00"), deck_name: "Test Deck One" }),
      activity({ id: 3, ts: at("10:02:00"), deck_name: null, deck_id: null }),
      activity({ id: 2, ts: at("10:01:00"), deck_name: "Test Deck Two" }),
      activity({ id: 1, ts: at("10:00:00"), deck_name: "Test Deck One" }),
    ];
    assert.deepEqual(groupSessions(feed)[0].deckNames, ["Test Deck One", "Test Deck Two"]);
  });

  test("an empty feed has no sessions", () => {
    assert.deepEqual(groupSessions([]), []);
  });

  test("an unreadable time never joins a session", () => {
    const feed = [activity({ id: 2, ts: "garbage" }), activity({ id: 1, ts: at("10:00:00") })];
    assert.equal(groupSessions(feed).length, 2);
  });
});
