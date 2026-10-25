import type { Database } from "../src/sqlite.ts";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { OWNER, agent } from "../src/actor.ts";
import { type Change, applyChanges } from "../src/deck/membership.ts";
import { type DeckCategory, type Role, createDeck } from "../src/deck/service.ts";
import { FIXTURES } from "./fixtures.ts";

export const AGENT = agent("test-agent");

/** A fresh in-memory database with the fixture cards. */
export function testDb(): Database {
  const db = openDb(":memory:");
  ingestCards(db, FIXTURES);
  return db;
}

let n = 0;
export function newDeck(db: Database, category: DeckCategory = "paper", name?: string): number {
  return createDeck(db, OWNER, name ?? `Deck ${++n}`, { category });
}

/** The owner adding one card directly, as the UI does. */
export function add(
  db: Database,
  deckId: number,
  oracleId: string,
  opts: { slotId?: number | null; role?: Role; quantity?: number } = {},
): number | null {
  return applyChanges(
    db,
    OWNER,
    deckId,
    [{ action: "add", oracle_id: oracleId, slot_id: opts.slotId, role: opts.role, quantity: opts.quantity }],
    { kind: "edit" },
  );
}

export function cut(db: Database, deckId: number, oracleId: string, opts: Partial<Extract<Change, { action: "cut" }>> = {}) {
  return applyChanges(db, OWNER, deckId, [{ action: "cut", oracle_id: oracleId, ...opts }], {
    kind: "edit",
  });
}
