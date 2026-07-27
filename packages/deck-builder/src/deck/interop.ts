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
import { ServiceError, addCard, getDeck, removeCard, updateCard } from "./service.ts";
import { isHardFiltered, logImport } from "./proposals.ts";
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

// Resolve one line of a plain-text deck list to a card.
//
// `resolveExactName` matches full names AND face names, so a staple can come
// back "ambiguous" purely because some double-faced card has a back face with
// the same name — "Swords to Plowshares" and "Rampant Growth" both do. On a
// deck-list line, "1 Swords to Plowshares" unambiguously means the card
// actually called that, so a unique full-name match wins. 25 of the 47
// colliding names in the current snapshot resolve this way; the rest (un-set
// variants, split-card halves) stay genuinely ambiguous and are reported.
export function resolveListName(db: DatabaseSync, name: string) {
  const matches = resolveExactName(db, name);
  if (matches.length <= 1) return matches;
  const exact = matches.filter(
    (m) => m.name.trim().toLowerCase() === name.trim().toLowerCase(),
  );
  return exact.length === 1 ? exact : matches;
}

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
    role: "card" | "commander" | "companion";
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
    const matches = resolveListName(db, entry.name);
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

  // Command-zone capacity left over once the cards absent from the pasted list
  // are gone. A list carrying its own [Commander] category should land its
  // commander in the command zone — that's what sets the deck's color
  // identity — but never past the 2/1 caps, which would throw mid-import.
  let commanderRoom =
    2 - cards.filter((c) => c.role === "commander" && pasted.has(c.oracle_id)).length;
  let companionRoom =
    1 - cards.filter((c) => c.role === "companion" && pasted.has(c.oracle_id)).length;

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
    const lowered = p.categories.map((c) => c.toLowerCase());
    let role: "card" | "commander" | "companion" = "card";
    if (lowered.some((c) => c === "commander" || c === "commanders") && commanderRoom > 0) {
      role = "commander";
      commanderRoom--;
    } else if (lowered.includes("companion") && companionRoom > 0) {
      role = "companion";
      companionRoom--;
    }
    adds.push({
      oracle_id: oracleId,
      name: p.name,
      quantity: p.quantity,
      slot_id: slot?.id ?? null,
      slot_name: slot?.name ?? null,
      category: p.categories[0] ?? null,
      role,
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

// An import applies in full, immediately (settled deviation from the original
// §9 wording). The approval gate exists so the AGENT cannot change the deck
// without a ruling; a pasted list is the owner's own list, already ruled on by
// the act of pasting it, and routing 99 cards through per-card accept clicks
// was pure friction. The preview diff is the confirmation step, the whole
// import lands atomically, and it is recorded as ONE decision-log entry that
// carries a full snapshot — so "undo" on that row restores the prior list
// exactly. One log line also keeps a 99-card import from flushing every real
// ruling out of the agent's retention window (§12).
export interface ImportResult {
  applied: { added: number; cut: number; quantity_changed: number };
  // Null when the list matched the deck exactly — nothing was written.
  log_id: number | null;
  diff: ImportDiff;
}

export function applyImport(
  db: DatabaseSync,
  deckId: number,
  text: string,
  note?: string,
): ImportResult {
  const diff = diffImport(db, deckId, text);
  const applied = {
    added: diff.adds.length,
    cut: diff.cuts.length,
    quantity_changed: diff.quantity_changes.length,
  };
  if (!applied.added && !applied.cut && !applied.quantity_changed)
    return { applied, log_id: null, diff };

  // Snapshot every card BEFORE touching anything — this is the undo payload.
  const before = getDeck(db, deckId).cards.map((c) => ({
    oracle_id: c.oracle_id,
    slot_id: c.slot_id,
    role: c.role,
    owned: c.owned,
    quantity: c.quantity,
    tag_ids: c.tag_ids,
  }));

  db.exec("BEGIN");
  try {
    for (const c of diff.cuts) removeCard(db, deckId, c.oracle_id);
    for (const a of diff.adds) {
      // Slots deleted between preview and apply must not fail the import.
      const slotId =
        a.slot_id != null &&
        db.prepare("SELECT 1 FROM slots WHERE id = ? AND deck_id = ?").get(a.slot_id, deckId)
          ? a.slot_id
          : null;
      addCard(db, deckId, a.oracle_id, { slotId, role: a.role });
      if (a.quantity > 1) updateCard(db, deckId, a.oracle_id, { quantity: a.quantity });
    }
    for (const q of diff.quantity_changes)
      updateCard(db, deckId, q.oracle_id, { quantity: q.to });

    const summary =
      note?.trim() ||
      `Imported an Archidekt list at revision ${diff.revision}: +${applied.added}/−${applied.cut}` +
        (applied.quantity_changed ? `, ${applied.quantity_changed} quantity change(s)` : "");
    const logId = logImport(db, deckId, summary, before);
    db.exec("COMMIT");
    return { applied, log_id: logId, diff };
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
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
