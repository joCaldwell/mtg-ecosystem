// Archidekt interop (spec §9). Deck construction happens here; playtesting
// and goldfishing happen in Archidekt. One-way export, one-way import, both
// through the approval gate. There is deliberately NO sync layer — Archidekt
// never silently becomes the source of truth.
//
// ---------------------------------------------------------------------------
// FORMAT — verified against Archidekt's own documentation and forum on
// 2026-07-26, per the spec's explicit "do not rely on model memory" warning:
//
//  * Base import format is MTGO-style "QTY Name", one card per line, exact
//    names. Archidekt's FAQ points at the "Formatting Examples" block inside
//    the Import menu itself and states the importer "will add all the cards
//    found in the list to the deck and report any errors".
//  * Archidekt's full round-trip field template, posted by an Archidekt staff
//    account in forum thread 6024990 (a user surprised the default export had
//    changed to "include all available site fields"), is:
//        1x Card Name (code) *F* [Category] ^Label,#000000^
//  * Forum thread 3223199 (Archidekt's own post on the Import menu) states
//    Ctrl+Shift+C copies "the selected cards and their full Archidekt syntax
//    (i.e. what appears in the text box on the Import menu)" — so that same
//    template IS the import syntax, and [Category] round-trips.
//  * Third-party formatting guides additionally document a backtick category
//    form (1 Sol Ring `Maybeboard`). Unverified against Archidekt itself, so
//    the parser ACCEPTS it and the exporter never EMITS it.
//  * The same guides report that set codes, foil markers (*F*) and count
//    headers ("Creatures (24)") each break some importers.
//
// EXPORT therefore emits only `QTY Name [Category]`: we key cards on
// oracle_id and store neither set code nor finish, so there is nothing to
// gain from the risky fields. IMPORT accepts the whole superset, because the
// text being pasted back was produced by Archidekt, not by us.
// ---------------------------------------------------------------------------

import type { DatabaseSync } from "node:sqlite";
import { ServiceError, getDeck } from "./service.ts";
import { createProposal, isHardFiltered } from "./proposals.ts";
import { resolveExactName, suggestNames } from "../search/index.ts";

// Categories we emit for cards that aren't in a user slot. "Commander" is
// Archidekt's own command-zone category name.
const COMMANDER_CATEGORY = "Commander";
const COMPANION_CATEGORY = "Companion";
const UNSLOTTED_CATEGORY = "Unslotted";

export interface ExportOptions {
  // Emit [Category] tags derived from slots / role. Off = bare "QTY Name".
  categories?: boolean;
  // Buy list: only cards not marked owned (spec §9). The owned flag is a
  // user-facing shopping concern and never reaches agent context.
  onlyUnowned?: boolean;
}

export interface ExportResult {
  text: string;
  card_count: number;
  line_count: number;
  omitted_owned: number;
  revision: number;
}

export function exportDeck(
  db: DatabaseSync,
  deckId: number,
  opts: ExportOptions = {},
): ExportResult {
  const { deck, slots, cards } = getDeck(db, deckId);
  const slotName = new Map(slots.map((s) => [s.id, s.name]));

  const categoryOf = (c: (typeof cards)[number]) => {
    if (c.role === "commander") return COMMANDER_CATEGORY;
    if (c.role === "companion") return COMPANION_CATEGORY;
    return c.slot_id != null ? (slotName.get(c.slot_id) ?? UNSLOTTED_CATEGORY) : UNSLOTTED_CATEGORY;
  };

  // Command zone first, then slots in their configured order, then the rest —
  // grouping is expressed through [Category] only. Count headers such as
  // "Creatures (24)" are known to break the importer, so we emit none.
  const order = new Map<string, number>();
  order.set(COMMANDER_CATEGORY, 0);
  slots.forEach((s, i) => order.set(s.name, i + 1));
  order.set(UNSLOTTED_CATEGORY, slots.length + 1);
  order.set(COMPANION_CATEGORY, slots.length + 2);

  let omittedOwned = 0;
  const rows = cards
    .filter((c) => {
      if (opts.onlyUnowned && c.owned) {
        omittedOwned++;
        return false;
      }
      return true;
    })
    .map((c) => ({ card: c, category: categoryOf(c) }))
    .sort(
      (a, b) =>
        (order.get(a.category) ?? 999) - (order.get(b.category) ?? 999) ||
        a.card.name.localeCompare(b.card.name),
    );

  const lines = rows.map(({ card, category }) =>
    opts.categories === false
      ? `${card.quantity} ${card.name}`
      : `${card.quantity} ${card.name} [${category}]`,
  );

  return {
    text: lines.join("\n") + (lines.length ? "\n" : ""),
    card_count: rows.reduce((n, r) => n + r.card.quantity, 0),
    line_count: lines.length,
    omitted_owned: omittedOwned,
    revision: deck.revision,
  };
}

// ---------- parsing (permissive — accepts the whole Archidekt superset) ----------

export interface ParsedLine {
  line_no: number;
  raw: string;
  quantity: number;
  name: string;
  categories: string[];
}

export interface ParseResult {
  entries: ParsedLine[];
  // Lines we could not read as a card at all (as opposed to names that simply
  // don't resolve — those come back from the diff, with suggestions).
  unparsed: Array<{ line_no: number; raw: string }>;
}

// Whole-line section headers Archidekt and other exporters emit. None of
// these are Magic card names, so skipping them cannot swallow a real card.
const SECTION_HEADER =
  /^(deck|decklist|sideboard|maybeboard|commander|commanders|companion|tokens?|main(board)?|about)\s*:?\s*$/i;
// "Creatures (24)" style count headers — documented to break the importer,
// so they turn up in lists people hand-edit.
const COUNT_HEADER = /^[A-Za-z][A-Za-z '\/-]*\s*\(\d+\)\s*$/;

export function parseArchidektList(text: string): ParseResult {
  const entries: ParsedLine[] = [];
  const unparsed: Array<{ line_no: number; raw: string }> = [];

  text.split(/\r?\n/).forEach((raw, i) => {
    const lineNo = i + 1;
    let line = raw.trim();
    if (!line) return;
    if (line.startsWith("//") || line.startsWith("#")) return;
    if (SECTION_HEADER.test(line) || COUNT_HEADER.test(line)) return;
    // MTGO sideboard prefix.
    line = line.replace(/^SB:\s*/i, "");

    // Leading quantity: "1", "1x", "1 x". Absent means one copy.
    let quantity = 1;
    const qty = line.match(/^(\d+)\s*[xX]?\s+(?=\S)/);
    if (qty) {
      quantity = Number(qty[1]);
      line = line.slice(qty[0].length);
    }

    // Strip trailing metadata in any order. Each field is anchored to the end
    // so a name containing e.g. "//" is never eaten.
    const categories: string[] = [];
    for (;;) {
      const before = line;
      // ^Label,#rrggbb^
      line = line.replace(/\s*\^[^^]*\^\s*$/, "");
      // [Cat] or [Cat1,Cat2]
      const bracket = line.match(/\s*\[([^\[\]]*)\]\s*$/);
      if (bracket) {
        categories.unshift(
          ...bracket[1]
            .split(",")
            .map((c) => c.trim())
            // Archidekt category modifiers ({top}, {noDeck}, …) are reported
            // by users but we could not verify their syntax — strip anything
            // brace-wrapped rather than guess at its meaning.
            .map((c) => c.replace(/\{[^}]*\}/g, "").trim())
            .filter(Boolean),
        );
        line = line.slice(0, bracket.index).trimEnd();
      }
      // `Category`
      const backtick = line.match(/\s*`([^`]*)`\s*$/);
      if (backtick) {
        const c = backtick[1].trim();
        if (c) categories.unshift(c);
        line = line.slice(0, backtick.index).trimEnd();
      }
      // *F*, *E*, *F**E*
      line = line.replace(/(\s*\*[A-Za-z]\*)+\s*$/, "");
      // (set) optionally followed by a collector number
      line = line.replace(/\s*\([0-9A-Za-z_]{2,6}\)(\s+[0-9]+[a-z★]?)?\s*$/, "");
      if (line === before) break;
    }

    const name = line.trim();
    if (!name || quantity < 1) {
      unparsed.push({ line_no: lineNo, raw });
      return;
    }
    entries.push({ line_no: lineNo, raw, quantity, name, categories });
  });

  return { entries, unparsed };
}

// ---------- diff ----------

export interface ImportDiff {
  adds: Array<{
    oracle_id: string;
    name: string;
    quantity: number;
    slot_id: number | null;
    slot_name: string | null;
    category: string | null;
  }>;
  cuts: Array<{ oracle_id: string; name: string; quantity: number; role: string }>;
  quantity_changes: Array<{ oracle_id: string; name: string; from: number; to: number }>;
  unchanged: number;
  // Names the exact resolver could not match. Never guessed — suggestions are
  // shown for the owner to fix by hand (spec §6.4's rule applies to pasted
  // text too: exact match or nothing).
  unresolved: Array<{ line_no: number; name: string; suggestions: string[] }>;
  // Ambiguous exact matches (same printed name, several oracle ids).
  ambiguous: Array<{ line_no: number; name: string; oracle_ids: string[] }>;
  // Adds we refuse to propose because the owner hard-filtered them.
  blocked: Array<{ oracle_id: string; name: string; reason: string }>;
  unparsed: Array<{ line_no: number; raw: string }>;
  revision: number;
}

export function diffImport(db: DatabaseSync, deckId: number, text: string): ImportDiff {
  const { deck, slots, cards } = getDeck(db, deckId);
  const { entries, unparsed } = parseArchidektList(text);

  const slotByName = new Map(slots.map((s) => [s.name.toLowerCase(), s]));
  const inDeck = new Map(cards.map((c) => [c.oracle_id, c]));

  const unresolved: ImportDiff["unresolved"] = [];
  const ambiguous: ImportDiff["ambiguous"] = [];
  const blocked: ImportDiff["blocked"] = [];

  // oracle_id → total quantity in the pasted list (a card may appear on more
  // than one line, e.g. split across categories).
  const pasted = new Map<
    string,
    { name: string; quantity: number; categories: string[] }
  >();

  for (const entry of entries) {
    const matches = resolveExactName(db, entry.name);
    if (matches.length === 0) {
      unresolved.push({
        line_no: entry.line_no,
        name: entry.name,
        suggestions: suggestNames(db, entry.name, 5),
      });
      continue;
    }
    if (matches.length > 1) {
      ambiguous.push({
        line_no: entry.line_no,
        name: entry.name,
        oracle_ids: matches.map((m) => m.oracle_id),
      });
      continue;
    }
    const card = matches[0];
    const prev = pasted.get(card.oracle_id);
    if (prev) {
      prev.quantity += entry.quantity;
      prev.categories.push(...entry.categories);
    } else {
      pasted.set(card.oracle_id, {
        name: card.name,
        quantity: entry.quantity,
        categories: [...entry.categories],
      });
    }
  }

  const adds: ImportDiff["adds"] = [];
  const quantityChanges: ImportDiff["quantity_changes"] = [];
  let unchanged = 0;

  for (const [oracleId, p] of pasted) {
    const existing = inDeck.get(oracleId);
    if (existing) {
      if (existing.quantity !== p.quantity)
        quantityChanges.push({
          oracle_id: oracleId,
          name: p.name,
          from: existing.quantity,
          to: p.quantity,
        });
      else unchanged++;
      continue;
    }
    if (isHardFiltered(db, deckId, oracleId)) {
      const row = db
        .prepare("SELECT reason FROM hard_filters WHERE deck_id = ? AND oracle_id = ?")
        .get(deckId, oracleId) as { reason: string } | undefined;
      blocked.push({ oracle_id: oracleId, name: p.name, reason: row?.reason ?? "" });
      continue;
    }
    // First category that names an existing slot wins; Commander/Companion/
    // Unslotted are our own export categories and map to no slot.
    const slot = p.categories.map((c) => slotByName.get(c.toLowerCase())).find(Boolean) ?? null;
    adds.push({
      oracle_id: oracleId,
      name: p.name,
      quantity: p.quantity,
      slot_id: slot?.id ?? null,
      slot_name: slot?.name ?? null,
      category: p.categories[0] ?? null,
    });
  }

  const cuts = cards
    .filter((c) => !pasted.has(c.oracle_id))
    .map((c) => ({
      oracle_id: c.oracle_id,
      name: c.name,
      quantity: c.quantity,
      role: c.role,
    }));

  return {
    adds: adds.sort((a, b) => a.name.localeCompare(b.name)),
    cuts: cuts.sort((a, b) => a.name.localeCompare(b.name)),
    quantity_changes: quantityChanges.sort((a, b) => a.name.localeCompare(b.name)),
    unchanged,
    unresolved,
    ambiguous,
    blocked,
    unparsed,
    revision: deck.revision,
  };
}

// The diff becomes a proposal — every change comes back through the same
// approval gate with a reason attached (spec §9). Import proposals are exempt
// from the 3–5 item cap (settled deviation from §7.1): these are the owner's
// own changes re-entering, not the agent ranking ideas.
export function createImportProposal(
  db: DatabaseSync,
  deckId: number,
  text: string,
  note?: string,
): { proposal_id: number | null; diff: ImportDiff } {
  const diff = diffImport(db, deckId, text);
  const items = [
    ...diff.adds.map((a) => ({
      action: "add" as const,
      oracle_id: a.oracle_id,
      slot_id: a.slot_id,
      rationale: `Import: in the pasted Archidekt list, not in the deck${
        a.category ? ` (category “${a.category}”)` : ""
      }.`,
    })),
    ...diff.cuts.map((c) => ({
      action: "cut" as const,
      oracle_id: c.oracle_id,
      rationale: `Import: in the deck${
        c.role !== "card" ? ` as ${c.role}` : ""
      }, absent from the pasted Archidekt list.`,
    })),
  ];

  if (!items.length) return { proposal_id: null, diff };

  const proposalId = createProposal(db, deckId, items, {
    source: "import",
    note:
      note?.trim() ||
      `Archidekt import diff at revision ${diff.revision}: +${diff.adds.length}/−${diff.cuts.length}`,
  });
  return { proposal_id: proposalId, diff };
}

// ---------- playtest notes (spec §9) ----------

export interface PlaytestNote {
  id: number;
  revision: number;
  note: string;
  cards: string[];
  created_at: string;
}

export function addPlaytestNote(db: DatabaseSync, deckId: number, note: string): number {
  const trimmed = note.trim();
  if (!trimmed) throw new ServiceError("A playtest note cannot be empty");
  const { deck, cards } = getDeck(db, deckId);
  // Snapshot the exact list the note is about. The decision log already gives
  // version history, but pinning the names here means the note still reads
  // correctly years later without replaying the log.
  const snapshot = cards
    .filter((c) => c.role !== "companion")
    .flatMap((c) => (c.quantity > 1 ? [`${c.quantity}x ${c.name}`] : [c.name]))
    .sort();
  const r = db
    .prepare("INSERT INTO playtest_notes (deck_id, revision, note, cards_json) VALUES (?, ?, ?, ?)")
    .run(deckId, deck.revision, trimmed, JSON.stringify(snapshot));
  return Number(r.lastInsertRowid);
}

export function listPlaytestNotes(db: DatabaseSync, deckId: number): PlaytestNote[] {
  return (
    db
      .prepare(
        "SELECT id, revision, note, cards_json, created_at FROM playtest_notes WHERE deck_id = ? ORDER BY id DESC",
      )
      .all(deckId) as unknown as Array<{
      id: number;
      revision: number;
      note: string;
      cards_json: string;
      created_at: string;
    }>
  ).map((r) => ({
    id: r.id,
    revision: r.revision,
    note: r.note,
    cards: JSON.parse(r.cards_json) as string[],
    created_at: r.created_at,
  }));
}

export function deletePlaytestNote(db: DatabaseSync, deckId: number, noteId: number): void {
  const r = db
    .prepare("DELETE FROM playtest_notes WHERE id = ? AND deck_id = ?")
    .run(noteId, deckId);
  if (!r.changes) throw new ServiceError(`Playtest note ${noteId} not found`, 404);
}
