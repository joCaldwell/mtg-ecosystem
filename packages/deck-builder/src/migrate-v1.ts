#!/usr/bin/env node
// One-way copy of a v1 deck-builder database into a v2 one.
//
//   node src/migrate-v1.ts [--from <v1.db>] [--to <v2.db>] [--dry-run]
//
// The v1 file is opened READ-ONLY and is never written, moved or deleted, so
// this can be run against the live file and re-run after deleting the v2 one.
// The target must contain no decks: this is a first-time copy, not a merge.
//
// What comes across: card data, decks (as active), slots, tags, cards with
// their slot/tags/quantity/role, the maybe list, the owned flag (as "in
// paper"), briefs and engines, the decision log (as history, already
// reviewed), and open proposals.
//
// What v2 has no table for becomes a deck memory, so nothing the owner wrote
// is lost: hard filters, playtest findings on a card, and playtest notes.
//
// What is left behind, because v2 has no place for it: the chat transcripts,
// compaction summaries, and recorded audit runs. They stay in the v1 file.

import { existsSync } from "node:fs";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";
import { DATA_DIR, DEFAULT_DB_PATH, openDb, openReadOnly, withTransaction } from "./db.ts";
import { recomputeIdentity } from "./deck/service.ts";
import { importMemory } from "./memory.ts";

export interface MigrationReport {
  cards: number;
  decks: number;
  deck_cards: number;
  slots: number;
  tags: number;
  briefs: number;
  engines: number;
  log_entries: number;
  open_proposals: number;
  pending_brief_edits: number;
  memories_from_hard_filters: number;
  memories_from_card_notes: number;
  memories_from_playtest_notes: number;
  left_behind: { chat_messages: number; consolidations: number; audit_runs: number };
  skipped_cards: string[];
}

const all = <T>(db: DatabaseSync, sql: string, ...params: Array<string | number>) =>
  db.prepare(sql).all(...params) as unknown as T[];

const count = (db: DatabaseSync, table: string) => {
  try {
    return (db.prepare(`SELECT COUNT(*) n FROM ${table}`).get() as { n: number }).n;
  } catch {
    return 0; // a table from a later v1 revision that this file predates
  }
};

function columns(db: DatabaseSync, table: string): Set<string> {
  return new Set(all<{ name: string }>(db, `PRAGMA table_info(${table})`).map((c) => c.name));
}

const REJECTION_LABEL: Record<string, string> = {
  hard_filter: "a hard no",
  thesis_change: "not what this deck is",
  playtest_finding: "a playtest finding",
  soft: "not now",
};

export function migrate(v1: DatabaseSync, v2: DatabaseSync): MigrationReport {
  if (!v1.prepare("SELECT 1 FROM sqlite_master WHERE name = 'decision_log'").get())
    throw new Error("The source is not a v1 deck-builder database (no decision_log table).");
  if (count(v2, "decks") > 0)
    throw new Error(
      "The target v2 database already has decks. This is a first-time copy, not a merge: " +
        "point --to at a new file, or delete the v2 database to start again.",
    );

  const report: MigrationReport = {
    cards: 0, decks: 0, deck_cards: 0, slots: 0, tags: 0, briefs: 0, engines: 0, log_entries: 0,
    open_proposals: 0, pending_brief_edits: 0, memories_from_hard_filters: 0,
    memories_from_card_notes: 0, memories_from_playtest_notes: 0,
    left_behind: {
      chat_messages: count(v1, "chat_messages"),
      consolidations: count(v1, "consolidations"),
      audit_runs: count(v1, "audit_runs"),
    },
    skipped_cards: [],
  };

  withTransaction(v2, () => {
    // ---- card data: the same columns in both versions ----
    if (count(v2, "cards") === 0) {
      const cols = [...columns(v2, "cards")].filter((c) => columns(v1, "cards").has(c));
      const ins = v2.prepare(
        `INSERT INTO cards (${cols.join(", ")}) VALUES (${cols.map(() => "?").join(", ")})`,
      );
      for (const row of all<Record<string, any>>(v1, `SELECT ${cols.join(", ")} FROM cards`))
        ins.run(...cols.map((c) => row[c]));
      const insName = v2.prepare("INSERT OR IGNORE INTO card_names (name_norm, oracle_id) VALUES (?, ?)");
      for (const r of all<{ name_norm: string; oracle_id: string }>(v1, "SELECT name_norm, oracle_id FROM card_names"))
        insName.run(r.name_norm, r.oracle_id);
      const insFts = v2.prepare("INSERT INTO cards_fts (name, search_text, oracle_id) VALUES (?, ?, ?)");
      for (const r of all<{ name: string; search_text: string; oracle_id: string }>(
        v1,
        "SELECT name, search_text, oracle_id FROM cards",
      ))
        insFts.run(r.name, r.search_text, r.oracle_id);
      const ingested = v1.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get() as
        | { value: string }
        | undefined;
      if (ingested)
        v2.prepare("INSERT OR REPLACE INTO meta (key, value) VALUES ('last_ingest', ?)").run(ingested.value);
    }
    report.cards = count(v2, "cards");
    const known = v2.prepare("SELECT name FROM cards WHERE oracle_id = ?");
    const nameOf = (oracleId: string) => (known.get(oracleId) as { name: string } | undefined)?.name;

    const hasMaybe = columns(v1, "deck_cards").has("maybeboard");
    const deckMap = new Map<number, number>();

    for (const deck of all<{ id: number; name: string; revision: number; created_at: string }>(
      v1,
      "SELECT id, name, revision, created_at FROM decks ORDER BY id",
    )) {
      // Existing decks are real decks: they arrive active, behind the gate.
      const deckId = Number(
        v2
          .prepare("INSERT INTO decks (name, status, revision, created_at) VALUES (?, 'active', ?, ?)")
          .run(deck.name, deck.revision, deck.created_at).lastInsertRowid,
      );
      deckMap.set(deck.id, deckId);
      report.decks++;

      const slotMap = new Map<number, number>();
      for (const s of all<any>(v1, "SELECT * FROM slots WHERE deck_id = ? ORDER BY position, id", deck.id)) {
        slotMap.set(
          s.id,
          Number(
            v2
              .prepare("INSERT INTO slots (deck_id, name, target_min, target_max, position) VALUES (?, ?, ?, ?, ?)")
              .run(deckId, s.name, s.target_min, s.target_max, s.position).lastInsertRowid,
          ),
        );
        report.slots++;
      }
      const tagMap = new Map<number, number>();
      for (const t of all<any>(v1, "SELECT * FROM tags WHERE deck_id = ?", deck.id)) {
        tagMap.set(t.id, Number(v2.prepare("INSERT INTO tags (deck_id, name) VALUES (?, ?)").run(deckId, t.name).lastInsertRowid));
        report.tags++;
      }

      for (const c of all<any>(v1, "SELECT * FROM deck_cards WHERE deck_id = ?", deck.id)) {
        if (!nameOf(c.oracle_id)) {
          report.skipped_cards.push(`${deck.name}: ${c.oracle_id}`);
          continue;
        }
        v2.prepare(
          `INSERT INTO deck_cards (deck_id, oracle_id, slot_id, role, maybe, in_paper, quantity, added_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        ).run(
          deckId,
          c.oracle_id,
          c.slot_id == null ? null : (slotMap.get(c.slot_id) ?? null),
          c.role,
          hasMaybe && c.maybeboard ? 1 : 0,
          c.owned ? 1 : 0,
          c.quantity,
          c.added_at,
        );
        report.deck_cards++;
      }
      for (const ct of all<any>(v1, "SELECT oracle_id, tag_id FROM deck_card_tags WHERE deck_id = ?", deck.id)) {
        const tagId = tagMap.get(ct.tag_id);
        if (tagId != null && nameOf(ct.oracle_id))
          v2.prepare("INSERT OR IGNORE INTO deck_card_tags (deck_id, oracle_id, tag_id) VALUES (?, ?, ?)").run(deckId, ct.oracle_id, tagId);
      }
      recomputeIdentity(v2, deckId);

      const brief = v1.prepare("SELECT * FROM briefs WHERE deck_id = ?").get(deck.id) as any;
      if (brief) {
        v2.prepare("INSERT INTO briefs (deck_id, thesis, constraints_md, updated_at) VALUES (?, ?, ?, ?)").run(
          deckId, brief.thesis, brief.constraints_md, brief.updated_at,
        );
        report.briefs++;
      }
      for (const e of all<any>(v1, "SELECT * FROM engines WHERE deck_id = ?", deck.id)) {
        const engineId = Number(
          v2.prepare("INSERT INTO engines (deck_id, name, description) VALUES (?, ?, ?)").run(deckId, e.name, e.description).lastInsertRowid,
        );
        for (const p of all<any>(v1, "SELECT * FROM engine_pieces WHERE engine_id = ?", e.id))
          if (nameOf(p.oracle_id))
            v2.prepare("INSERT OR IGNORE INTO engine_pieces (engine_id, oracle_id, note) VALUES (?, ?, ?)").run(engineId, p.oracle_id, p.note);
        report.engines++;
      }

      // ---- the decision log, as history. v1's undo snapshots are not carried
      // across: undoing a migrated cut puts the card back without the slot and
      // tags it had, and a migrated import has no cards to reverse.
      const logMap = new Map<number, number>();
      for (const l of all<any>(v1, "SELECT * FROM decision_log WHERE deck_id = ? ORDER BY id", deck.id)) {
        const isImport = l.action === "import";
        const kind =
          l.kind === "accept" ? (isImport ? "import" : "accept")
          : l.kind === "reject" ? "reject"
          : l.kind === "undo" ? "undo"
          : "edit";
        const card = l.card_name ?? "";
        const summary =
          l.kind === "reject"
            ? `Rejected ${l.action === "add" ? "adding" : l.action === "cut" ? "cutting" : ""} ${card}`.replace(/\s+/g, " ").trim() +
              (l.rejection_type ? ` (v1 reason type: ${REJECTION_LABEL[l.rejection_type] ?? l.rejection_type})` : "") +
              (!l.oracle_id && l.rationale ? ` — ${l.rationale}` : "")
            : l.kind === "filter_removed" ? `Hard filter removed: ${card}`
            : l.kind === "maybe_move" ? `Maybe list: ${l.rationale ?? card}`
            : l.oracle_id && !isImport
              ? `${l.kind === "undo" ? "Undo: " : ""}${l.action === "add" ? "Added" : "Cut"} ${card}`
              : (l.rationale ?? "");
        const logId = Number(
          v2
            .prepare(
              `INSERT INTO log (deck_id, revision, kind, summary, ruling_reason, proposal_id, author, reviewed_at, ts)
               VALUES (?, ?, ?, ?, ?, NULL, 'owner (from v1)', ?, ?)`,
            )
            .run(deckId, l.revision, kind, summary, l.rejection_reason ?? "", l.ts, l.ts).lastInsertRowid,
        );
        logMap.set(l.id, logId);
        if (l.oracle_id && (l.action === "add" || l.action === "cut") && nameOf(l.oracle_id))
          v2.prepare(
            `INSERT INTO log_cards (log_id, deck_id, oracle_id, card_name, action, quantity, reason)
             VALUES (?, ?, ?, ?, ?, 1, ?)`,
          ).run(logId, deckId, l.oracle_id, card || nameOf(l.oracle_id)!, l.action, l.rationale ?? "");
        report.log_entries++;
      }
      for (const l of all<any>(v1, "SELECT id, undo_of, undone_by FROM decision_log WHERE deck_id = ?", deck.id))
        v2.prepare("UPDATE log SET undo_of = ?, undone_by = ? WHERE id = ?").run(
          l.undo_of == null ? null : (logMap.get(l.undo_of) ?? null),
          l.undone_by == null ? null : (logMap.get(l.undone_by) ?? null),
          logMap.get(l.id)!,
        );

      // ---- what v2 has no table for becomes a deck memory ----
      for (const f of all<any>(v1, "SELECT * FROM hard_filters WHERE deck_id = ?", deck.id)) {
        if (!nameOf(f.oracle_id)) continue;
        importMemory(v2, {
          text: `Ruled out [[${nameOf(f.oracle_id)}]] for this deck as a hard no${f.reason ? `: ${f.reason}` : "."}`,
          deck_id: deckId, deck_revision: null, cards: [f.oracle_id], created_at: f.created_at,
        });
        report.memories_from_hard_filters++;
      }
      for (const n of all<any>(v1, "SELECT * FROM card_notes WHERE deck_id = ?", deck.id)) {
        if (!nameOf(n.oracle_id)) continue;
        importMemory(v2, {
          text: `Playtest finding on [[${nameOf(n.oracle_id)}]]: ${n.note}`,
          deck_id: deckId, deck_revision: null, cards: [n.oracle_id], created_at: n.created_at,
        });
        report.memories_from_card_notes++;
      }
      for (const n of all<any>(v1, "SELECT * FROM playtest_notes WHERE deck_id = ?", deck.id)) {
        importMemory(v2, {
          text: `Playtest note: ${n.note}`,
          deck_id: deckId, deck_revision: n.revision, cards: [], created_at: n.created_at,
        });
        report.memories_from_playtest_notes++;
      }

      // ---- what is still waiting on the owner ----
      for (const p of all<any>(v1, "SELECT * FROM proposals WHERE deck_id = ? AND status = 'open'", deck.id)) {
        const items = all<any>(
          v1,
          "SELECT * FROM proposal_items WHERE proposal_id = ? AND status = 'pending' AND action IN ('add', 'cut')",
          p.id,
        ).filter((i) => nameOf(i.oracle_id));
        if (!items.length) continue;
        const proposalId = Number(
          v2
            .prepare("INSERT INTO proposals (deck_id, summary, author, created_at) VALUES (?, ?, ?, ?)")
            .run(deckId, p.note ?? "", p.source === "manual" ? "owner (from v1)" : "the v1 agent", p.created_at).lastInsertRowid,
        );
        for (const i of items)
          v2.prepare(
            "INSERT INTO proposal_items (proposal_id, action, oracle_id, slot_id, reason, group_id) VALUES (?, ?, ?, ?, ?, ?)",
          ).run(proposalId, i.action, i.oracle_id, i.slot_id == null ? null : (slotMap.get(i.slot_id) ?? null), i.rationale, i.group_id);
        report.open_proposals++;
      }
      for (const e of all<any>(v1, "SELECT * FROM brief_edits WHERE deck_id = ? AND status = 'pending'", deck.id)) {
        v2.prepare(
          "INSERT INTO document_edits (deck_id, kind, payload_json, reason, author, created_at) VALUES (?, ?, ?, ?, 'the v1 agent', ?)",
        ).run(deckId, e.kind, e.payload_json, e.rationale, e.created_at);
        report.pending_brief_edits++;
      }
    }
  });
  return report;
}

function printReport(r: MigrationReport, from: string, to: string, dryRun: boolean) {
  const lines = [
    dryRun ? `DRY RUN — nothing was written. From ${from}:` : `Copied ${from}\n    into ${to}:`,
    `  ${r.decks} deck(s), ${r.deck_cards} deck cards, ${r.slots} slots, ${r.tags} tags`,
    `  ${r.briefs} brief(s), ${r.engines} engine(s)`,
    `  ${r.log_entries} decision-log entries, as history`,
    `  ${r.open_proposals} open proposal(s), ${r.pending_brief_edits} pending brief edit(s)`,
    `  memories made from: ${r.memories_from_hard_filters} hard filter(s), ${r.memories_from_card_notes} card note(s), ${r.memories_from_playtest_notes} playtest note(s)`,
    `  card data: ${r.cards} cards`,
    `Left in the v1 file (v2 has no place for them): ${r.left_behind.chat_messages} chat message(s), ` +
      `${r.left_behind.consolidations} compaction summar(ies), ${r.left_behind.audit_runs} audit run(s).`,
  ];
  if (r.skipped_cards.length)
    lines.push(`Skipped ${r.skipped_cards.length} deck card(s) whose card is not in the card data:\n  ${r.skipped_cards.join("\n  ")}`);
  lines.push("The v1 file was opened read-only and is unchanged.");
  console.log(lines.join("\n"));
}

function main() {
  const arg = (name: string) => {
    const i = process.argv.indexOf(name);
    return i > -1 ? process.argv[i + 1] : undefined;
  };
  const from = arg("--from") ?? join(DATA_DIR, "deck-builder.db");
  const to = arg("--to") ?? DEFAULT_DB_PATH;
  const dryRun = process.argv.includes("--dry-run");
  if (!existsSync(from)) {
    console.error(`No v1 database at ${from}. Pass --from <path>.`);
    process.exit(1);
  }
  const v1 = openReadOnly(from);
  // A dry run does the whole copy into memory and throws it away.
  const v2 = openDb(dryRun ? ":memory:" : to);
  try {
    printReport(migrate(v1, v2), from, to, dryRun);
  } catch (e: any) {
    console.error(`Migration failed, and nothing was written: ${e.message}`);
    process.exitCode = 1;
  } finally {
    v1.close();
    v2.close();
  }
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/^.*\//, ""))) main();
