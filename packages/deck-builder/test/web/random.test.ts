import { test } from "node:test";
import assert from "node:assert/strict";
import {
  RANDOM_BOUND_LIMIT,
  parseRange,
  rollInRange,
} from "../../web/src/domain/random.ts";

test("random ranges accept inclusive whole-number bounds, including negatives", () => {
  assert.deepEqual(parseRange(" 1 ", "20"), { min: 1, max: 20 });
  assert.deepEqual(parseRange("-5", "-5"), { min: -5, max: -5 });
  assert.deepEqual(parseRange(String(-RANDOM_BOUND_LIMIT), String(RANDOM_BOUND_LIMIT)), {
    min: -RANDOM_BOUND_LIMIT,
    max: RANDOM_BOUND_LIMIT,
  });
});

test("random ranges reject blanks, fractions, oversized values and reversed bounds", () => {
  for (const [lower, upper] of [["", "6"], ["1.5", "6"], ["1e3", "6"], ["1", String(RANDOM_BOUND_LIMIT + 1)]])
    assert.match((parseRange(lower, upper) as { error: string }).error, /whole numbers/);
  assert.match((parseRange("7", "6") as { error: string }).error, /must not exceed/);
});

test("rolls stay within both bounds and reach each end", () => {
  const range = { min: -2, max: 3 };
  assert.equal(rollInRange(range, () => 0), -2);
  assert.equal(rollInRange(range, () => 5), 3);
  const seen = new Set<number>();
  for (let i = 0; i < 2000; i++) {
    const n = rollInRange(range);
    assert.ok(Number.isInteger(n) && n >= -2 && n <= 3);
    seen.add(n);
  }
  assert.equal(seen.size, 6);
  assert.equal(rollInRange({ min: 9, max: 9 }), 9);
});

test("rolls redraw values from the biased tail instead of wrapping them", () => {
  // span 3: 2^32 % 3 = 1, so only the single top value 2^32 - 1 is rejected.
  const draws = [2 ** 32 - 1, 4];
  assert.equal(rollInRange({ min: 1, max: 3 }, () => draws.shift()!), 2);
  assert.equal(draws.length, 0);
});
