import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { groupByDay } from "../../web/src/domain/activity.ts";
import { activity } from "./fixtures.ts";

describe("groupByDay", () => {
  const now = Date.parse("2026-03-02T23:00:00Z");
  // Server times are UTC; noon local time is the same calendar day in any zone.
  const localNoon = (y: number, m: number, d: number) => {
    const t = new Date(y, m - 1, d, 12).getTime();
    return new Date(t).toISOString().replace("T", " ").slice(0, 19);
  };

  test("consecutive entries on one local day share a heading and keep feed order", () => {
    const feed = [
      activity({ id: 3, ts: localNoon(2026, 3, 2) }),
      activity({ id: 2, ts: localNoon(2026, 3, 2) }),
      activity({ id: 1, ts: localNoon(2026, 3, 1) }),
    ];
    const days = groupByDay(feed, new Date(2026, 2, 2, 18).getTime());
    assert.deepEqual(days.map((d) => d.label), ["Today", "Yesterday"]);
    assert.deepEqual(days.map((d) => d.entries.map((e) => e.id)), [[3, 2], [1]]);
  });

  test("older days read as a short date", () => {
    const days = groupByDay([activity({ ts: localNoon(2026, 2, 20) })], new Date(2026, 2, 2, 18).getTime());
    assert.equal(days[0].label, "20 Feb");
  });

  test("an empty feed has no days", () => {
    assert.deepEqual(groupByDay([], now), []);
  });

  test("an unreadable time never joins a day", () => {
    const feed = [activity({ id: 2, ts: "garbage" }), activity({ id: 1, ts: "garbage" })];
    assert.equal(groupByDay(feed, now).length, 2);
  });
});
