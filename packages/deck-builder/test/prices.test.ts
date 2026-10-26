import { test, describe } from "node:test";
import assert from "node:assert/strict";
import type { Database } from "../src/sqlite.ts";
import { OWNER } from "./actors.ts";
import { resolveCard } from "../src/cards.ts";
import { createSlot, getDeck, shortlistCard } from "../src/deck/service.ts";
import { ingestCards } from "../src/ingest.ts";
import { callTool } from "../src/mcp/tools.ts";
import {
  type ScryfallPrinting,
  cheapestPrintings,
  formatPrice,
  ingestPrices,
  priceStatus,
  priceResolver, getPriceSource, setPriceSource,
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
    assert.deepEqual(cheapest.get("a:tcgplayer"), {
      cents: 205, store: "tcgplayer", currency: "USD",
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
    assert.equal(cheapest.get("a:tcgplayer")?.cents, 30000);
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
    assert.equal(cheapest.get("a:tcgplayer")?.cents, 400);
    assert.equal(cheapest.get("b:tcgplayer")?.cents, 400);
  });

  test("equal prices resolve to the same printing whatever order they arrive in", async () => {
    const one = printing("a", { set: "t02", collector_number: "7" });
    const two = printing("a", { set: "t01", collector_number: "9" });
    assert.deepEqual((await cheapestPrintings([one, two])).get("a:tcgplayer"), (await cheapestPrintings([two, one])).get("a:tcgplayer"));
    assert.equal((await cheapestPrintings([one, two])).get("a:tcgplayer")?.set_code, "t01");
  });

  test("reads an async stream of printings", async () => {
    async function* stream() {
      yield printing("a", { prices: { usd: "0.25" } });
    }
    assert.equal((await cheapestPrintings(stream())).get("a:tcgplayer")?.cents, 25);
  });
});

describe("stored prices", () => {
  test("a refresh replaces every price, so a card that lost its price is unpriced", async () => {
    const db = testDb();
    await priced(db, [printing("id-solring", { prices: { usd: "1.19" } }), printing("id-llanowar")]);
    assert.equal(priceResolver(db, OWNER).resolve([resolveCard(db, "Sol Ring")])[0].cents, 119);

    const result = await priced(db, [printing("id-llanowar", { prices: { usd: "0.40" } })]);
    assert.equal(priceResolver(db, OWNER).resolve([resolveCard(db, "Sol Ring")])[0].cents, null);
    assert.equal(priceResolver(db, OWNER).resolve([resolveCard(db, "Llanowar Elves")])[0].cents, 40);
    assert.equal(result.priced, 1);
    assert.deepEqual(priceStatus(db), { currency: "USD", store: "tcgplayer", as_of: AS_OF, priced: 1, unpriced: result.unpriced });
  });

  test("no prices loaded: nothing is priced and there is no date", () => {
    const db = testDb();
    assert.equal(priceStatus(db).as_of, null);
    assert.equal(priceStatus(db).priced, 0);
    assert.equal(priceResolver(db, OWNER).resolve([resolveCard(db, "Sol Ring")])[0].cents, null);
  });

  test("a card that enters the database later is already priced", async () => {
    const db = testDb();
    await priced(db, [printing("id-new", { prices: { usd: "2.00" } })]);
    assert.equal(priceStatus(db).priced, 0);
    ingestCards(db, [...FIXTURES, { ...FIXTURES[0], oracle_id: "id-new", name: "Test Card New" }]);
    assert.equal(priceResolver(db, OWNER).resolve([resolveCard(db, "id-new")])[0].cents, 200);
  });

  test("re-ingesting the cards keeps their prices", async () => {
    const db = testDb();
    await priced(db, [printing("id-solring", { prices: { usd: "1.19" } })]);
    ingestCards(db, FIXTURES);
    assert.equal(priceResolver(db, OWNER).resolve([resolveCard(db, "Sol Ring")])[0].cents, 119);
  });

  test("formatPrice", () => {
    assert.equal(formatPrice(5), "$0.05");
    assert.equal(formatPrice(119), "$1.19");
    assert.equal(formatPrice(162623), "$1,626.23");
  });
});

describe("searching by price", () => {
  const db = testDb();
  const names = (q: string) => search(db, OWNER, q).map((c) => c.name);

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
    assert.equal(search(db, OWNER, "usd=1.19")[0].cents, 119);
  });
  test("a card with no price matches no price comparison", () => {
    assert.ok(!names("usd<1000").includes("Counterspell"));
    assert.ok(!names("usd>=0").includes("Counterspell"));
  });
  test("a value that is not an amount is a search error", () => {
    assert.throws(() => search(db, OWNER, "usd<cheap"), SearchError);
  });
  test("usd cannot be a slot name, or usd: would stop parsing", () => {
    assert.throws(() => createSlot(db, OWNER, newDeck(db), "usd"), /reserved/);
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
    shortlistCard(db, OWNER, deck, "id-seedborn");
    return { db, deck };
  }

  test("copies count, the maybe list does not, and unpriced cards are reported", async () => {
    const { db, deck } = await build();
    assert.deepEqual(getDeck(db, OWNER, deck).computed.cost, { cents: 250 + 119 + 15, unpriced: 1, as_of: AS_OF, currency: "USD", store: "tcgplayer", source: "Scryfall" });
    assert.equal(getDeck(db, OWNER, deck).cards.find((c) => c.name === "Sol Ring")?.cents, 119);
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
      /- Estimated cost: \$3\.84 at the cheapest printing of each card; 1 card has no price and is not counted/,
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

describe("per-user store prices (D18)", () => {
  test("reduces finishes independently per store, including optional EUR etched", async () => {
    const rows = await cheapestPrintings([
      printing("a", { prices: { usd: "5.00", usd_etched: "2.00", eur: "3.00", eur_foil: "1.20" } }),
      printing("a", { set: "t02", prices: { usd: "1.19", eur_etched: "0.90" } }),
      printing("a", { set_type: "memorabilia", prices: { usd: "0.01", eur: "0.01" } }),
    ]);
    assert.deepEqual(rows.get("a:tcgplayer"), { cents: 119, store: "tcgplayer", currency: "USD", set_code: "t02", collector_number: "1", finish: "nonfoil" });
    assert.deepEqual(rows.get("a:cardmarket"), { cents: 90, store: "cardmarket", currency: "EUR", set_code: "t02", collector_number: "1", finish: "etched" });
  });

  test("settings are isolated, atomic, validate currency/store, and block both agents", () => {
    const db = testDb();
    db.exec("INSERT INTO users(id,handle,display_name) VALUES(2,'second','Second')");
    const second = { ...OWNER, user_id: 2 };
    assert.deepEqual(getPriceSource(db, second), { currency: "USD", store: "lowest" });
    setPriceSource(db, second, { currency: "EUR", store: "cardmarket" });
    assert.deepEqual(getPriceSource(db, OWNER), { currency: "USD", store: "lowest" });
    for (const user_id of [1,2]) {
      assert.throws(() => setPriceSource(db, { ...AGENT, user_id }, { currency: "USD", store: "lowest" }), /Only the owner/);
    }
    for (const value of [{ currency: "EUR", store: "tcgplayer" }, { currency: "USD", store: "cardmarket" }, { currency: "GBP", store: "lowest" }, { currency: "USD", store: "unknown" }]) {
      assert.throws(() => setPriceSource(db, second, value as any), /Choose USD/);
      assert.deepEqual(getPriceSource(db, second), { currency: "EUR", store: "cardmarket" });
    }
  });

  test("EUR follows the principal across cards, deck totals, proposals, sourcing and agent responses", async () => {
    const { createProposal, getProposal } = await import("../src/deck/proposals.ts");
    const { createDeck } = await import("../src/deck/service.ts");
    const { compareDeckSources } = await import("../src/deck/sourcing.ts");
    const { applyChanges } = await import("../src/deck/membership.ts");
    const db = testDb();
    db.exec("INSERT INTO users(id,handle,display_name) VALUES(2,'second','Second')");
    const second = { ...OWNER, user_id: 2 }, agent = { ...AGENT, user_id: 2 };
    setPriceSource(db, second, { currency: "EUR", store: "lowest" });
    await priced(db, [printing("id-solring", { prices: { usd: "1.19", eur: "0.80" } }), printing("id-forest", { prices: { usd: "0.05" } })]);
    const target = createDeck(db, second, "EUR target", { category: "theorycrafted" });
    const source = createDeck(db, second, "EUR source", { category: "theorycrafted" });
    applyChanges(db, second, target, [{ action: "add", oracle_id: "id-solring", quantity: 1 }, { action: "add", oracle_id: "id-forest", quantity: 3 }], { kind: "edit" });
    const view = getDeck(db, agent, target);
    assert.deepEqual(view.computed.cost, { cents: 80, unpriced: 1, currency: "EUR", store: "cardmarket", source: "Scryfall", as_of: AS_OF });
    assert.equal(view.cards.find(c => c.oracle_id === "id-forest")!.cents, null, "USD never fills a missing EUR price");
    const proposal = createProposal(db, agent, target, { items: [{ action: "cut", oracle_id: "id-solring", reason: "Budget preference" }] });
    assert.equal(getProposal(db, agent, proposal)!.items[0].cents, 80);
    assert.equal(getDeck(db, agent, target).computed.pending.cost_delta.cents, -80);
    assert.equal(getDeck(db, agent, target).computed.pending.projected_cost.cents, 0);
    assert.equal(getDeck(db, agent, target).computed.pending.cost_delta.currency, "EUR");
    assert.equal(getProposal(db, agent, proposal)!.items[0].currency, "EUR");
    assert.equal(compareDeckSources(db, agent, target, [source]).summary.remaining.cost.cents, 80);
    const found = callTool({ db, actor: agent }, "get_cards", { cards: ["Sol Ring"] });
    assert.match(found.text, /€0\.80/);
    assert.match(found.text, /Prices: Cardmarket \(EUR\), Scryfall daily estimates as of 2026-10-11\./);
    assert.equal((found.structuredContent as any).cards[0].cents, 80);
    assert.equal((found.structuredContent as any).cards[0].store, "cardmarket");
    for (const [name,args] of [["start_session",{}],["get_deck",{deck:target,sections:["list"]}],["get_deck",{deck:target}],["search_cards",{query:"eur<=1"}]] as const) {
      const out = callTool({db,actor:agent},name,args).text;
      assert.equal(out.split("Prices: Cardmarket").length-1,1);
      assert.match(out,/Scryfall daily estimates as of 2026-10-11/);
    }
    assert.equal(search(db, second, "eur<=0.80")[0].currency, "EUR");
    assert.equal(search(db, OWNER, "eur<=0.80")[0].cents, 119, "query currency does not change displayed currency");
    assert.equal(search(db, second, "usd<=0.05")[0].cents, null);
    assert.deepEqual(search(db, second, "eur!=0.80"), [], "unpriced never matches !=");
    assert.match(callTool({db,actor:AGENT},"get_cards",{cards:["Sol Ring"]}).text,/\$1\.19/);
    assert.throws(() => createSlot(db, OWNER, newDeck(db), "eur"), /reserved/);
  });

  test("failed staging preserves both stores and publication time", async () => {
    const db = testDb();
    await priced(db, [printing("id-solring", { prices: { usd: "1.19", eur: "0.80" } })]);
    const before = db.prepare("SELECT * FROM card_prices ORDER BY store").all();
    const bad = await cheapestPrintings([printing("id-solring")]);
    bad.get("id-solring:tcgplayer")!.cents = -1;
    assert.throws(() => ingestPrices(db, bad, "2026-10-26"), /CHECK/);
    assert.deepEqual(db.prepare("SELECT * FROM card_prices ORDER BY store").all(), before);
    assert.equal(priceStatus(db).as_of, AS_OF);
    assert.equal(db.prepare("SELECT name FROM sqlite_master WHERE name='card_prices_staging'").get(), undefined);
  });
});

test("projected price matches actual approval for copies, parking, unpriced cards and maybe promotion", async () => {
  const { createProposal, acceptItems, pendingItemIds } = await import("../src/deck/proposals.ts");
  const db = testDb(), deck = newDeck(db, "theorycrafted");
  await priced(db, [printing("id-forest", { prices: { usd: "0.05" } }), printing("id-solring", { prices: { usd: "1.19" } })]);
  add(db, deck, "id-forest", { quantity: 3 });
  add(db, deck, "id-counterspell");
  shortlistCard(db, OWNER, deck, "id-solring");
  const id = createProposal(db, AGENT, deck, { items: [
    { action: "cut", oracle_id: "id-forest", reason: "One fewer copy" },
    { action: "cut", oracle_id: "id-counterspell", to_maybe: true, reason: "Park this card" },
    { action: "add", oracle_id: "id-solring", reason: "Promote from maybe" },
  ] });
  const pending = getDeck(db, OWNER, deck).computed.pending;
  assert.equal(pending.cost_delta.cents, 114);
  assert.equal(pending.cost_delta.unpriced, -1);
  assert.equal(pending.projected_cost.cents, 129);
  acceptItems(db, OWNER, pendingItemIds(db, OWNER, id));
  assert.deepEqual(getDeck(db, OWNER, deck).computed.cost, pending.projected_cost);
});

for (const currency of ["usd", "eur"]) test(`${currency} negation keeps unpriced cards while direct != excludes them`, async () => {
  const db = testDb();
  try {
    await priced(db, [printing("id-solring", { prices: { usd: "1.19", eur: "0.80" } }),
      printing("id-llanowar", { prices: { usd: "0.40", eur: "0.20" } }),
      printing("id-seedborn", { prices: { usd: "14.00", eur: "12.00" } })]);
    const ids = (query: string) => search(db, OWNER, query, { limit: 1000 }).map(c => c.oracle_id).sort();
    const all = (db.prepare("SELECT oracle_id FROM cards WHERE retired=0").all() as {oracle_id:string}[]).map(c => c.oracle_id).sort();
    const aboveFive = ids(`${currency}>5`);
    assert.deepEqual(ids(`-${currency}>5`), all.filter(id => !aboveFive.includes(id)));
    assert.ok(ids(`-${currency}>5`).includes("id-counterspell"));
    assert.ok(!ids(`${currency}!=5`).includes("id-counterspell"));
    assert.deepEqual(ids(`--${currency}>5`), aboveFive);
    const cheapCreatures = ids(`t:creature ${currency}<1`);
    assert.deepEqual(ids(`t:creature -${currency}<1`), ids("t:creature").filter(id => !cheapCreatures.includes(id)));
    const unpriced = all.filter(id => !ids(`${currency}>=0`).includes(id));
    assert.deepEqual(ids(`(${currency}>100 or -${currency}>0)`), unpriced);
  } finally { db.close(); }
});

test("invalid stored price pairs fall back together to USD/lowest", async () => {
  const { setSetting } = await import("../src/db.ts");
  const db = testDb();
  try {
    for (const [currency, store] of [["EUR", "tcgplayer"], ["USD", "cardmarket"], ["GBP", "lowest"], ["EUR", "unknown"]]) {
      setSetting(db, OWNER, "price_currency", currency); setSetting(db, OWNER, "price_store", store);
      assert.deepEqual(getPriceSource(db, OWNER), { currency: "USD", store: "lowest" });
      const price = priceResolver(db, OWNER).resolve([{ oracle_id: "id-solring" }])[0];
      assert.equal(price.currency, "USD"); assert.equal(price.store, "tcgplayer"); assert.equal(price.cents, null);
    }
    setPriceSource(db, OWNER, { currency: "EUR", store: "lowest" });
    assert.deepEqual(getPriceSource(db, OWNER), { currency: "EUR", store: "lowest" });
  } finally { db.close(); }
});

test("public prices use the signed-in viewer and anonymous USD/lowest independently of any owner", async () => {
  const db = testDb();
  try {
    db.exec("INSERT INTO users(id,handle,display_name) VALUES(2,'viewer','Viewer')");
    const viewer = { ...OWNER, user_id: 2 };
    await priced(db, [printing("id-solring", { prices: { usd: "1.19", eur: "0.80" } })]);
    setPriceSource(db, OWNER, { currency: "EUR", store: "cardmarket" });
    assert.deepEqual(getPriceSource(db, null), { currency: "USD", store: "lowest" });
    const anon = priceResolver(db, null).resolve([{ oracle_id: "id-solring" }])[0];
    assert.deepEqual(anon, { oracle_id: "id-solring", cents: 119, currency: "USD", store: "tcgplayer", as_of: AS_OF, source: "Scryfall" });
    assert.equal(priceResolver(db, viewer).resolve([{ oracle_id: "id-solring" }])[0].cents, 119);
    setPriceSource(db, viewer, { currency: "EUR", store: "lowest" });
    assert.equal(priceResolver(db, viewer).resolve([{ oracle_id: "id-solring" }])[0].cents, 80);
    assert.equal(priceResolver(db, null).resolve([{ oracle_id: "id-solring" }])[0].cents, 119);
  } finally { db.close(); }
});


test("anonymous source setters return 401 without changing any principal's preferences", () => {
  const db = testDb();
  try {
    setPriceSource(db, OWNER, { currency: "EUR", store: "cardmarket" });
    const before = db.prepare("SELECT * FROM user_settings ORDER BY user_id,key").all();
    assert.throws(() => setPriceSource(db, null, { currency: "USD", store: "lowest" }), { status: 401, message: "Sign in to change the price source" });
    assert.deepEqual(db.prepare("SELECT * FROM user_settings ORDER BY user_id,key").all(), before);
    assert.deepEqual(getPriceSource(db, null), { currency: "USD", store: "lowest" });
    assert.deepEqual(getPriceSource(db, OWNER), { currency: "EUR", store: "cardmarket" });
  } finally { db.close(); }
});
