#!/usr/bin/env node
// Disposable note-density fixture. Card references come only from the seed DB;
// deck and shortlist writes use the app's owner capabilities.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Database, backup } from "../src/sqlite.ts";
import { UX_EVAL_ROOT } from "./lib/paths.mjs";
import { installClock, installRandom, installSqliteClock } from "./lib/determinism.ts";

const directory = join(UX_EVAL_ROOT, "long-note-fixture-v1");
const target = join(directory, "seed.db");
if (existsSync(target)) throw Error("Keep the existing fixture immutable; choose a new fixture version before regenerating");
mkdirSync(join(directory, "app-data"), { recursive: true });
Object.assign(process.env, { DECKBUILDER_DATA: join(directory, "app-data"), DECKBUILDER_DB: join(directory, "unused.db"), DECKBUILDER_MODE: "local" });
installClock(); installRandom(0x10a6707e);
const ro = new Database(join(UX_EVAL_ROOT, "seed", "seed.db"), { readOnly: true });
await backup(ro, target); ro.close();
const { openDb } = await import("../src/db.ts");
const { forkDeck } = await import("../src/deck/fork.ts");
const { getDeck, shortlistCard, organizeCard } = await import("../src/deck/service.ts");
const db = openDb(target); installSqliteClock(db as any);
const actor = { user_id: 1, via: "ui" as const, label: "ui" };
const info = JSON.parse(readFileSync(join(UX_EVAL_ROOT, "seed", "seed.json"), "utf8"));
const deck = forkDeck(db, actor, info.decks.najeela, "Shortlist study");
const initial = getDeck(db, actor, deck);
const originalMaybe = initial.cards.filter(card => card.maybe);
const needed = 20 - originalMaybe.length;
if (needed < 3) throw Error("Expected the standard shortlist to leave room for blank-note cases");
const extra = db.prepare(`SELECT oracle_id, name FROM cards WHERE retired = 0 AND commander_legality = 'legal'
  AND oracle_id NOT IN (SELECT oracle_id FROM deck_cards WHERE deck_id = ?)
  ORDER BY (mana_cost IS NULL) DESC, name LIMIT ?`).all(deck, needed) as unknown as { oracle_id: string; name: string }[];
if (extra.length !== needed) throw Error("Not enough verified card facts");
const link = initial.cards.find(card => !card.maybe && card.role === "card")!;
const note = (name: string, n: number) => `Option ${n}: hold [[${name}]] for a later test. Compare the space it would occupy with [[${link.name}]] before changing the saved list. This is a shortlist note, not an approved change.\n\nAfter the next playtest, record whether this option addresses the observed problem and whether another candidate already covers the same job. Keep the existing plan until there is evidence to support a change.\n\nIf it stays shortlisted, retain these observations and revisit the decision with the full card text. There is no automatic promotion or rules assumption in this note.`;
originalMaybe.slice(2).forEach((card, i) => shortlistCard(db, actor, deck, card.oracle_id, note(card.name, i + 1)));
extra.forEach((card, i) => shortlistCard(db, actor, deck, card.oracle_id, i < 3 ? "" : note(card.name, i + originalMaybe.length)));
const current = getDeck(db, actor, deck);
const slots = current.slots;
if (slots.length < 2) throw Error("Assignment QA needs at least two real fixture slots");
current.cards.filter(card => card.maybe).forEach((card, i) => organizeCard(db, actor, deck, card.oracle_id, { slotId: slots[i % slots.length].id, secondarySlotIds: [slots[(i + 1) % slots.length].id] }));
const final = getDeck(db, actor, deck), maybe = final.cards.filter(card => card.maybe);
const report = { deck, main_count: final.computed.card_count, maybe_count: maybe.length, long_notes: maybe.filter(card => card.maybe_note.includes("\n\n")).length, blank_notes: maybe.filter(card => !card.maybe_note).length, costless_cards: maybe.filter(card => !card.mana_cost).length, max_note_chars: Math.max(...maybe.map(card => card.maybe_note.length)), cards: maybe.map(card => ({ oracle_id: card.oracle_id, name: card.name, note: card.maybe_note, mana_cost: card.mana_cost, slot_id: card.slot_id, secondary_slot_ids: card.secondary_slot_ids })) };
if (report.maybe_count !== 20 || report.long_notes < 12 || report.blank_notes < 3 || report.costless_cards < 1) throw Error("Incomplete density coverage");
info.decks.najeela = deck;
writeFileSync(join(directory, "seed.json"), JSON.stringify(info, null, 2) + "\n");
writeFileSync(join(directory, "coverage.json"), JSON.stringify(report, null, 2) + "\n");
db.close();
console.log(JSON.stringify({ directory, deck, maybe_count: report.maybe_count, long_notes: report.long_notes, blank_notes: report.blank_notes, costless_cards: report.costless_cards, max_note_chars: report.max_note_chars }));
