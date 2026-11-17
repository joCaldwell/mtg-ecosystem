import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createCardCache } from "../web/src/mtg/cardCache.ts";
import type { CardInfo } from "../web/src/mtg/CardInfo.ts";

// Use exact retained producer facts; the test never invents a card or text.
const fixture = JSON.parse(readFileSync(new URL("../../game/fixtures/foundations-opening/presentation.json", import.meta.url), "utf8"));
const source = fixture.cards[0];
const card: CardInfo = { ...source.faces[0], oracle_id: source.oracle_id, power: null, toughness: null, loyalty: null };

test("concurrent card lookups share a request and failed reads can be retried", async () => {
  let calls = 0;
  const cache = createCardCache(async () => { if (++calls === 1) throw new Error("offline"); return card; });
  const first = cache.lookup(card.name);
  assert.equal(cache.lookup(` ${card.name.toUpperCase()} `), first);
  assert.equal(await first, null);
  assert.equal(await cache.lookup(card.name), card);
  assert.equal(calls, 2);
});

test("an old failed lookup cannot evict facts primed by a newer deck read", async () => {
  let reject!: (reason: Error) => void;
  const cache = createCardCache(() => new Promise((_, fail) => { reject = fail; }));
  const pending = cache.lookup(card.name);
  await Promise.resolve();
  cache.prime([card]);
  reject(new Error("late failure"));
  assert.equal(await pending, null);
  assert.equal(await cache.lookup(card.name), card);
});

test("an invalidated failure cannot delete the replacement request", async () => {
  let reject!: (reason: Error) => void;
  let calls = 0;
  const cache = createCardCache(() => ++calls === 1 ? new Promise((_, fail) => { reject = fail; }) : Promise.resolve(card));
  const old = cache.lookup(card.name);
  await Promise.resolve();
  cache.clear();
  const replacement = cache.lookup(card.name);
  reject(new Error("invalidated failure"));
  await old;
  assert.equal(cache.lookup(card.name), replacement);
  assert.equal(await replacement, card);
  assert.equal(calls, 2);
});
