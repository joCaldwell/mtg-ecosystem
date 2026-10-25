import { removeIdentity } from "./legacy-db.ts";
// A proposed cut that parks the card on the maybe list instead of removing it
// (spec §8.1). Off unless the agent asks for it.

import { test, describe, before, after } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { OWNER } from "./actors.ts";
import { openDb } from "../src/db.ts";
import { undo } from "../src/deck/membership.ts";
import { acceptItems, createProposal, getProposal, pendingItemIds, rejectItems } from "../src/deck/proposals.ts";
import { createSlot, createTag, getDeck, organizeCard } from "../src/deck/service.ts";
import { ingestCards } from "../src/ingest.ts";
import { callTool } from "../src/mcp/tools.ts";
import { FIXTURES } from "./fixtures.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

const db = testDb();
const cardIn = (deck: number, oracleId: string) => getDeck(db, OWNER, deck).cards.find((c) => c.oracle_id === oracleId);

function seeded() {
  const id = newDeck(db);
  add(db, id, "id-teferi", { role: "commander" });
  add(db, id, "id-counterspell");
  return id;
}

describe("a cut to the maybe list (spec §8.1)", () => {
  test("a plain cut still takes the card out of the deck altogether", () => {
    const deck = seeded();
    const pid = createProposal(db, AGENT, deck, {
      items: [{ action: "cut", oracle_id: "id-counterspell", reason: "Too reactive." }],
    });
    assert.equal(getProposal(db, OWNER, pid)!.items[0].to_maybe, 0);
    acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid));
    assert.equal(cardIn(deck, "id-counterspell"), undefined);
  });

  test("with to_maybe, accepting moves the card to the maybe list with its slot and tags", () => {
    const deck = seeded();
    const slot = createSlot(db, OWNER, deck, "Interaction");
    const tag = createTag(db, OWNER, deck, "Stack");
    organizeCard(db, OWNER, deck, "id-counterspell", { slotId: slot, tagIds: [tag] });

    const pid = createProposal(db, AGENT, deck, {
      items: [{ action: "cut", oracle_id: "id-counterspell", to_maybe: true, reason: "Worth revisiting." }],
    });
    assert.equal(getProposal(db, OWNER, pid)!.items[0].to_maybe, 1);
    // Nothing moves until the owner rules.
    assert.equal(cardIn(deck, "id-counterspell")!.maybe, 0);

    const before = getDeck(db, OWNER, deck).computed.card_count;
    const logId = acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid));
    const parked = cardIn(deck, "id-counterspell")!;
    assert.deepEqual([parked.maybe, parked.slot_id, parked.tag_ids], [1, slot, [tag]]);
    assert.equal(getDeck(db, OWNER, deck).computed.card_count, before - 1);
    assert.equal(getDeck(db, OWNER, deck).computed.maybe_count, 1);

    undo(db, OWNER, logId);
    assert.equal(cardIn(deck, "id-counterspell")!.maybe, 0);
  });

  test("a swap can park the card it replaces", () => {
    const deck = seeded();
    const pid = createProposal(db, AGENT, deck, {
      items: [
        { action: "cut", oracle_id: "id-counterspell", to_maybe: true, reason: "Swap out.", group: "swap" },
        { action: "add", oracle_id: "id-solring", reason: "Swap in.", group: "swap" },
      ],
    });
    acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid).slice(0, 1));
    assert.equal(cardIn(deck, "id-counterspell")!.maybe, 1);
    assert.equal(cardIn(deck, "id-solring")!.maybe, 0);
  });

  test("rejecting leaves the card where it was", () => {
    const deck = seeded();
    const pid = createProposal(db, AGENT, deck, {
      items: [{ action: "cut", oracle_id: "id-counterspell", to_maybe: true, reason: "Worth revisiting." }],
    });
    rejectItems(db, OWNER, pendingItemIds(db, OWNER, pid));
    assert.equal(cardIn(deck, "id-counterspell")!.maybe, 0);
  });

  test("only a cut takes it, and not a card with several copies", () => {
    const deck = seeded();
    add(db, deck, "id-forest", { quantity: 3 });
    assert.throws(
      () => createProposal(db, AGENT, deck, { items: [{ action: "add", oracle_id: "id-solring", to_maybe: true, reason: "x" }] }),
      /Only a cut can move a card to the maybe list/,
    );
    assert.throws(
      () => createProposal(db, AGENT, deck, { items: [{ action: "cut", oracle_id: "id-forest", to_maybe: true, reason: "x" }] }),
      /has 3 copies in the deck.*without to_maybe/,
    );
  });

  test("the agent asks for it through propose_changes, and the pending item says so", () => {
    const deck = seeded();
    const text = (name: string, args: unknown) => callTool({ db, actor: AGENT }, name, args).text;
    text("propose_changes", {
      deck,
      items: [
        { action: "cut", card: "Counterspell", to_maybe: true, reason: "Worth revisiting." },
        { action: "add", card: "Sol Ring", reason: "Ramp." },
      ],
    });
    const pending = text("get_deck", { deck, sections: ["proposals"] });
    assert.match(pending, /- cut Counterspell, moving it to the maybe list: Worth revisiting\./);
    assert.match(pending, /- add Sol Ring: Ramp\./);
  });
});

describe("a database made before the option existed", () => {
  let dir: string;
  before(() => {
    dir = mkdtempSync(join(tmpdir(), "deckbuilder-tomaybe-"));
  });
  after(() => rmSync(dir, { recursive: true, force: true }));

  test("gains the column on open and keeps its pending items as plain cuts", () => {
    const path = join(dir, "old.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const deck = newDeck(old);
    add(old, deck, "id-counterspell");
    const pid = createProposal(old, AGENT, deck, {
      items: [{ action: "cut", oracle_id: "id-counterspell", reason: "Too reactive." }],
    });
    removeIdentity(old);
    old.exec("ALTER TABLE proposal_items DROP COLUMN to_maybe; UPDATE meta SET value = '10' WHERE key = 'schema_version'");
    old.close();

    const reopened = openDb(path);
    assert.equal(getProposal(reopened, OWNER, pid)!.items[0].to_maybe, 0);
    acceptItems(reopened, OWNER, pendingItemIds(reopened, OWNER, pid));
    assert.equal(getDeck(reopened, OWNER, deck).cards.length, 0);
    reopened.close();
  });
});
