import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openDb, withTransaction } from "../src/db.ts";
import { ingestCards, type ScryfallCard } from "../src/ingest.ts";
import { stageTokenCatalog, tokenStatus, type TokenPrinting } from "../src/token-catalog.ts";
import { deckTokens } from "../src/deck/tokens.ts";
import { deckState, publicDeckState } from "../src/state.ts";
import { createDeck, shortlistCard } from "../src/deck/service.ts";
import { setVisibility } from "../src/sharing.ts";
import { renderDeck } from "../src/mcp/render.ts";
import { add, cut } from "./helpers.ts";
import { OWNER, agent } from "./actors.ts";

// Consumed facts/links from Scryfall default_cards, 2026-10-29:
// https://api.scryfall.com/bulk-data/default_cards
const fixture = JSON.parse(readFileSync(new URL("./fixtures/deck-tokens.json", import.meta.url), "utf8")) as {producers: TokenPrinting[]; tokens: TokenPrinting[]};
const [avenger, manufactor, teferi, knave] = fixture.producers;
const asOf = "2026-10-29T21:05:56.154+00:00";
function populated() {
  const db = openDb(":memory:");
  ingestCards(db, fixture.producers as ScryfallCard[]);
  const stage = stageTokenCatalog(db);
  try { for (const card of [...fixture.producers, ...fixture.tokens]) stage.add(card); stage.finish(asOf); }
  finally { stage.close(); }
  return db;
}

test("catalog resolves printing links, dedupes editions, keeps distinct same-name tokens and emblems outside card search", () => {
  const db = populated();
  try {
    const deck = createDeck(db, OWNER, "Token reference");
    add(db, deck, manufactor!.oracle_id!);
    add(db, deck, teferi!.oracle_id!, {role: "commander"});
    add(db, deck, knave!.oracle_id!);
    const view = deckState(db, OWNER, deck);
    assert.equal(view.computed.card_count, 3);
    assert.deepEqual(view.tokens.map(t => t.name), ["Clue", "Food", "Teferi, Temporal Archmage Emblem", "Treasure"]);
    assert.equal(view.tokens[2]!.kind, "emblem");
    assert.equal(view.tokens.find(t => t.name === "Treasure")!.producer_count, 2);
    assert.ok(view.tokens.every(t => t.producer_count === t.made_by.length));
    assert.ok(view.tokens.every(t => t.image_normal?.startsWith("https://cards.scryfall.io/")));
    assert.equal((db.prepare("SELECT COUNT(*) n FROM token_catalog WHERE json_extract(facts_json, '$.name') = 'Beast'").get() as {n:number}).n, 2);
    assert.equal((db.prepare("SELECT COUNT(*) n FROM token_catalog WHERE json_extract(facts_json, '$.name') = 'Treasure'").get() as {n:number}).n, 1);
    assert.equal((db.prepare("SELECT COUNT(*) n FROM cards").get() as {n:number}).n, 4);
    assert.match(renderDeck(db, agent("test"), deck, ["tokens"]), /Treasure.*from 2: Academy Manufactor; Ruthless Knave/);
  } finally { db.close(); }
});

test("deck and allow-listed public payloads track membership, omit maybes and reject other principals", () => {
  const db = populated();
  try {
    const deck = createDeck(db, OWNER, "Shared token reference");
    shortlistCard(db, OWNER, deck, avenger!.oracle_id!, "");
    assert.deepEqual(deckState(db, OWNER, deck).tokens, []);
    add(db, deck, manufactor!.oracle_id!, {quantity: 2, role: "companion"});
    const view = deckState(db, OWNER, deck);
    assert.equal(view.tokens[0]!.made_by[0]!.quantity, 2);
    assert.equal(view.tokens[0]!.made_by[0]!.role, "companion");
    const slug = setVisibility(db, OWNER, deck, "unlisted").slug!;
    assert.deepEqual(publicDeckState(db, null, slug).tokens, view.tokens);
    assert.deepEqual(view.tokens_status, {ready: true, as_of: asOf, source: "Scryfall"});
    assert.throws(() => deckTokens(db, agent("foreign", 2), deck), {status: 404});
    cut(db, deck, manufactor!.oracle_id!, {quantity: 2});
    assert.deepEqual(publicDeckState(db, null, slug).tokens, []);
    setVisibility(db, OWNER, deck, "private");
    assert.throws(() => publicDeckState(db, null, slug), {status: 404});
  } finally { db.close(); }
});

test("failed/truncated refresh retains facts, links and freshness; corrected links replace cleanly", () => {
  const db = populated();
  const snapshot = () => ({catalog: db.prepare("SELECT * FROM token_catalog ORDER BY oracle_id").all(), links: db.prepare("SELECT * FROM card_token_links ORDER BY oracle_id, token_oracle_id").all(), status: tokenStatus(db)});
  try {
    const before = snapshot();
    let stage = stageTokenCatalog(db);
    try {
      stage.add(fixture.tokens[0]!);
      assert.throws(() => stage.finish("new"), /truncated/);
      assert.deepEqual(snapshot(), before);
    } finally { stage.close(); }
    stage = stageTokenCatalog(db);
    try {
      const omitted = fixture.tokens.find(c => c.name === "Clue")!;
      for (const card of [...fixture.producers, ...fixture.tokens.filter(c => c.id !== omitted.id)]) stage.add(card);
      assert.deepEqual(stage.missingPrintingIds(), [omitted.id]);
      assert.throws(() => stage.finish("new"), /unresolved/);
      assert.deepEqual(snapshot(), before);
    } finally { stage.close(); }
    stage = stageTokenCatalog(db);
    try {
      for (const card of [...fixture.producers, ...fixture.tokens]) stage.add(card);
      assert.throws(() => withTransaction(db, () => { stage.finish("new"); throw new Error("price failure"); }), /price failure/);
      assert.deepEqual(snapshot(), before);
    } finally { stage.close(); }
    stage = stageTokenCatalog(db);
    try {
      for (const card of [...fixture.producers.map(c => ({...c, all_parts: []})), ...fixture.tokens]) stage.add(card);
      stage.finish("new");
      assert.deepEqual(db.prepare("SELECT * FROM card_token_links").all(), []);
    } finally { stage.close(); }
  } finally { db.close(); }
});

test("v16 migration preserves existing decks and distinguishes an uninitialized catalog from an empty one", () => {
  const dir = mkdtempSync(join(tmpdir(), "token-migration-"));
  try {
    const path = join(dir, "deck.db");
    const old = openDb(path);
    ingestCards(old, fixture.producers as ScryfallCard[]);
    const deck = createDeck(old, OWNER, "Existing hosted deck");
    add(old, deck, teferi!.oracle_id!, {role: "commander"});
    old.exec("DROP TABLE card_token_links; DROP TABLE token_catalog; UPDATE meta SET value = '15' WHERE key = 'schema_version'");
    const before = old.prepare("SELECT * FROM decks").all();
    old.close();
    const migrated = openDb(path);
    try {
      assert.deepEqual(migrated.prepare("SELECT * FROM decks").all(), before);
      assert.equal(deckState(migrated, OWNER, deck).cards[0]!.oracle_id, teferi!.oracle_id);
      assert.deepEqual(tokenStatus(migrated), {ready: false, as_of: null, source: "Scryfall"});
      assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    } finally { migrated.close(); }
  } finally { rmSync(dir, {recursive: true, force: true}); }
});
