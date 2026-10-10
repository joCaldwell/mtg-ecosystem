import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { ago, clockTime, dayLabel, parseServerTime } from "../../web/src/domain/time.ts";

// Dates are built from local parts so the expectations hold in any time zone.
const local = (y: number, m: number, d: number, h = 12, min = 0) => new Date(y, m - 1, d, h, min).getTime();
const iso = (t: number) => new Date(t).toISOString();

describe("parseServerTime", () => {
  test("a bare SQLite timestamp is UTC", () => {
    assert.equal(parseServerTime("2026-03-02 10:00:00"), Date.UTC(2026, 2, 2, 10, 0, 0));
    assert.equal(parseServerTime("2026-03-02T10:00:00"), Date.UTC(2026, 2, 2, 10, 0, 0));
  });

  test("a stamp with a zone is read as written", () => {
    assert.equal(parseServerTime("2026-03-02T10:00:00Z"), Date.UTC(2026, 2, 2, 10, 0, 0));
    assert.equal(parseServerTime("2026-03-02T10:00:00+02:00"), Date.UTC(2026, 2, 2, 8, 0, 0));
    assert.equal(parseServerTime("2026-03-02T10:00:00.250Z"), Date.UTC(2026, 2, 2, 10, 0, 0, 250));
  });

  test("unreadable text is NaN", () => {
    assert.ok(Number.isNaN(parseServerTime("")));
    assert.ok(Number.isNaN(parseServerTime("not a time")));
  });
});

describe("ago", () => {
  const now = Date.UTC(2026, 2, 20, 12, 0, 0);
  const before = (ms: number) => iso(now - ms);
  const SEC = 1000, MIN = 60 * SEC, HOUR = 60 * MIN, DAY = 24 * HOUR;

  test("under a minute is just now, including a clock slightly ahead", () => {
    assert.equal(ago(before(0), now), "just now");
    assert.equal(ago(before(59 * SEC), now), "just now");
    assert.equal(ago(before(-5 * MIN), now), "just now");
  });

  test("minutes", () => {
    assert.equal(ago(before(60 * SEC), now), "1 min ago");
    assert.equal(ago(before(12 * MIN), now), "12 min ago");
    assert.equal(ago(before(59 * MIN + 59 * SEC), now), "59 min ago");
  });

  test("hours", () => {
    assert.equal(ago(before(HOUR), now), "1 h ago");
    assert.equal(ago(before(3 * HOUR + 30 * MIN), now), "3 h ago");
    assert.equal(ago(before(DAY - SEC), now), "23 h ago");
  });

  test("yesterday, then days", () => {
    assert.equal(ago(before(DAY), now), "yesterday");
    assert.equal(ago(before(2 * DAY - SEC), now), "yesterday");
    assert.equal(ago(before(2 * DAY), now), "2 days ago");
    assert.equal(ago(before(6 * DAY + 5 * HOUR), now), "6 days ago");
  });

  test("a week or more is a short date, with the year when it is not this one", () => {
    const sameYear = local(2026, 8, 26);
    assert.equal(ago(iso(sameYear), local(2026, 10, 10)), "26 Aug");
    assert.equal(ago(iso(local(2025, 8, 26)), local(2026, 10, 10)), "26 Aug 2025");
  });

  test("works on SQLite stamps", () => {
    assert.equal(ago("2026-03-20 11:48:00", now), "12 min ago");
  });

  test("unreadable input is returned as it came", () => {
    assert.equal(ago("soon", now), "soon");
  });
});

describe("clockTime and dayLabel", () => {
  test("clockTime is local, zero-padded", () => {
    assert.equal(clockTime(iso(local(2026, 3, 2, 16, 42))), "16:42");
    assert.equal(clockTime(iso(local(2026, 3, 2, 7, 5))), "07:05");
    assert.equal(clockTime("nope"), "");
  });

  test("dayLabel is by local calendar day", () => {
    const now = local(2026, 3, 20, 9, 0);
    assert.equal(dayLabel(iso(local(2026, 3, 20, 0, 5)), now), "Today");
    assert.equal(dayLabel(iso(local(2026, 3, 19, 23, 55)), now), "Yesterday");
    assert.equal(dayLabel(iso(local(2026, 3, 18, 12, 0)), now), "18 Mar");
    assert.equal(dayLabel(iso(local(2025, 12, 31, 12, 0)), now), "31 Dec 2025");
  });
});
