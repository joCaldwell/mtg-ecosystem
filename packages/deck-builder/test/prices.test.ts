import { test, describe } from "node:test";
import assert from "node:assert/strict";
import type { Database } from "../src/sqlite.ts";
import { OWNER } from "../src/actor.ts";
import { resolveCard } from "../src/cards.ts";
import { createSlot, getDeck, shortlistCard } from "../src/deck/service.ts";
import { ingestCards } from "../src/ingest.ts";
import { callTool } from "../src/mcp/tools.ts";
import {
  type ScryfallPrinting,
  cheapestPrintings,
  formatUsd,
  ingestPrices,
  priceStatus,
} from "../src/prices.ts";
import { SearchError, search } from "../src/search/index.ts";
import { FIXTURES } from "./fixtures.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

// Set codes and collector numbers here are synthetic: no real printing appears.
function printing(oracleId: string, over: Partial<ScryfallPrinting> = {}): ScryfallPrinting {
  return {
    oracle_id: oracleId,
    set: "t01",
    set_type: "expansion",
    collector_number: "1",
    games: ["paper"],
    prices: { usd: "1.00", usd_foil: null, usd_etched: null },
    ...over,
  };
}

const AS_OF = "2026-10-11T09:05:44.341+00:00";

async function priced(db: Database, printings: ScryfallPrinting[]) {
  return ingestPrices(db, await cheapestPrintings(printings), AS_OF);
}

describe("the cheapest printing (spec §4.4)", () => {
  test("is the lowest price across every printing and finish", async () => {
    const cheapest = await cheapestPrintings([
      printing("a", { set: "t01", prices: { usd: "12.50" } }),
      printing("a", { set: "t02", collector_number: "40", prices: { usd: "3.10", usd_foil: "2.05" } }),
      printing("a", { set: "t03", prices: { usd: "2.99", usd_etched: "9.00" } }),
    ]);
    assert.deepEqual(cheapest.get("a"), {
      usd_cents: 205,
      set_code: "t02",
      collector_number: "40",
      finish: "foil",
    });
  });

  test("ignores printings a deck could not play", async () => {
    const cheapest = await cheapestPrintings([
      printing("a", { prices: { usd: "300.00" } }),
      printing("a", { set: "t02", set_type: "memorabilia", prices: { usd: "70.00" } }),
      printing("a", { set: "t03", oversized: true, prices: { usd: "5.00" } }),
      printing("a", { set: "t04", games: ["mtgo", "arena"], prices: { usd: "0.02" } }),
    ]);
    assert.equal(cheapest.get("a")?.usd_cents, 30000);
  });

  test("a card priced on no printing has no entry", async () => {
    const cheapest = await cheapestPrintings([
      printing("a", { prices: { usd: null, usd_foil: null, usd_etched: null } }),
      printing("b", { prices: undefined }),
    ]);
    assert.equal(cheapest.size, 0);
  });

  test("a reversible card prices the card on each face", async () => {
    const cheapest = await cheapestPrintings([
      printing("a", { prices: { usd: "9.00" } }),
      { ...printing("unused", { prices: { usd: "4.00" } }), oracle_id: undefined, card_faces: [{ oracle_id: "a" }, { oracle_id: "b" }] },
    ]);
    assert.equal(cheapest.get("a")?.usd_cents, 400);
    assert.equal(cheapest.get("b")?.usd_cents, 400);
  });

  test("equal prices resolve to the same printing whatever order they arrive in", async () => {
    const one = printing("a", { set: "t02", collector_number: "7" });
    const two = printing("a", { set: "t01", collector_number: "9" });
    assert.deepEqual((await cheapestPrintings([one, two])).get("a"), (await cheapestPrintings([two, one])).get("a"));
    assert.equal((await cheapestPrintings([one, two])).get("a")?.set_code, "t01");
  });

  test("reads an async stream of printings", async () => {
    async function* stream() {
      yield printing("a", { prices: { usd: "0.25" } });
    }
    assert.equal((await cheapestPrintings(stream())).get("a")?.usd_cents, 25);
  });
});

describe("stored prices", () => {
  test("a refresh replaces every price, so a card that lost its price is unpriced", async () => {
    const db = testDb();
    await priced(db, [printing("id-solring", { prices: { usd: "1.19" } }), printing("id-llanowar")]);
    assert.equal(resolveCard(db, "Sol Ring").usd_cents, 119);

    const result = await priced(db, [printing("id-llanowar", { prices: { usd: "0.40" } })]);
    assert.equal(resolveCard(db, "Sol Ring").usd_cents, null);
    assert.equal(resolveCard(db, "Llanowar Elves").usd_cents, 40);
    assert.equal(result.priced, 1);
    assert.deepEqual(priceStatus(db), { as_of: AS_OF, priced: 1, unpriced: result.unpriced });
  });

  test("no prices loaded: nothing is priced and there is no date", () => {
    const db = testDb();
    assert.equal(priceStatus(db).as_of, null);
    assert.equal(priceStatus(db).priced, 0);
    assert.equal(resolveCard(db, "Sol Ring").usd_cents, null);
  });

  test("a card that enters the database later is already priced", async () => {
    const db = testDb();
    await priced(db, [printing("id-new", { prices: { usd: "2.00" } })]);
    assert.equal(priceStatus(db).priced, 0);
    ingestCards(db, [...FIXTURES, { ...FIXTURES[0], oracle_id: "id-new", name: "Test Card New" }]);
    assert.equal(resolveCard(db, "id-new").usd_cents, 200);
  });

  test("re-ingesting the cards keeps their prices", async () => {
    const db = testDb();
    await priced(db, [printing("id-solring", { prices: { usd: "1.19" } })]);
    ingestCards(db, FIXTURES);
    assert.equal(resolveCard(db, "Sol Ring").usd_cents, 119);
  });

  test("formatUsd", () => {
    assert.equal(formatUsd(5), "$0.05");
    assert.equal(formatUsd(119), "$1.19");
    assert.equal(formatUsd(162623), "$1,626.23");
  });
});

describe("searching by price", () => {
  const db = testDb();
  const names = (q: string) => search(db, q).map((c) => c.name);

  test("usd compares the cheapest printing, in dollars", async () => {
    await priced(db, [
      printing("id-solring", { prices: { usd: "1.19" } }),
      printing("id-llanowar", { prices: { usd: "0.40" } }),
      printing("id-seedborn", { prices: { usd: "14.00" } }),
    ]);
    assert.deepEqual(names("usd<1"), ["Llanowar Elves"]);
    assert.deepEqual(names("usd<=1.19"), ["Llanowar Elves", "Sol Ring"]);
    assert.deepEqual(names("usd>=10"), ["Seedborn Muse"]);
    assert.deepEqual(names("t:creature usd<5"), ["Llanowar Elves"]);
    assert.equal(search(db, "usd=1.19")[0].usd_cents, 119);
  });
  test("a card with no price matches no price comparison", () => {
    assert.ok(!names("usd<1000").includes("Counterspell"));
    assert.ok(!names("usd>=0").includes("Counterspell"));
  });
  test("a value that is not an amount is a search error", () => {
    assert.throws(() => search(db, "usd<cheap"), SearchError);
  });
  test("usd cannot be a slot name, or usd: would stop parsing", () => {
    assert.throws(() => createSlot(db, newDeck(db), "usd"), /reserved/);
  });
});

describe("what a deck costs", () => {
  async function build() {
    const db = testDb();
    const deck = newDeck(db, "theorycrafted");
    await priced(db, [
      printing("id-teferi", { prices: { usd: "2.50" } }),
      printing("id-solring", { prices: { usd: "1.19" } }),
      printing("id-forest", { prices: { usd: "0.05" } }),
      printing("id-seedborn", { prices: { usd: "14.00" } }),
    ]);
    add(db, deck, "id-teferi", { role: "commander" });
    add(db, deck, "id-solring");
    add(db, deck, "id-forest", { quantity: 3 });
    add(db, deck, "id-counterspell");
    shortlistCard(db, deck, "id-seedborn");
    return { db, deck };
  }

  test("copies count, the maybe list does not, and unpriced cards are reported", async () => {
    const { db, deck } = await build();
    assert.deepEqual(getDeck(db, deck).computed.cost, { usd_cents: 250 + 119 + 15, unpriced: 1, as_of: AS_OF });
    assert.equal(getDeck(db, deck).cards.find((c) => c.name === "Sol Ring")?.usd_cents, 119);
  });



  test("agents are shown prices by default, with the app's totals", async () => {
    const { db, deck } = await build();
    const text = (name: string, args: unknown) => callTool({ db, actor: AGENT }, name, args).text;

    const read = text("get_deck", { deck });
    assert.match(read, /- Sol Ring \{1\} — Artifact · \$1\.19/);
    assert.match(read, /- 3x Forest — Basic Land — Forest · \$0\.05 each/);
    assert.match(read, /- Counterspell \{U\}\{U\} — Instant\n/);
    assert.match(
      read,
      /- Estimated cost: \$3\.84 at the cheapest printing of each card \(Scryfall prices of 2026-10-11\); 1 card has no price and is not counted/,
    );
    assert.match(text("search_cards", { query: "usd<=2 t:artifact" }), /- Sol Ring \{1\} — Artifact · \$1\.19/);
    assert.match(text("get_cards", { cards: ["Seedborn Muse"] }), /Creature — Spirit 2\/4 · \$14\.00/);
    assert.match(text("edit_cards", { deck, add: [{ card: "Llanowar Elves" }] }), /Estimated cost: \$3\.84 at/);

  });

  test("before any prices are loaded, nothing about cost is said", () => {
    const db = testDb();
    const deck = newDeck(db, "theorycrafted");
    add(db, deck, "id-solring");
    const read = callTool({ db, actor: AGENT }, "get_deck", { deck }).text;
    assert.doesNotMatch(read, /Estimated cost|\$/);
  });
});
