// Archidekt interop (spec §12). Deck construction happens here; playtesting
// and goldfishing happen in Archidekt. One-way export, one-way import. There
// is deliberately NO sync layer — Archidekt never silently becomes the source
// of truth.
//
// ---------------------------------------------------------------------------
// FORMAT — verified against Archidekt's own documentation and forum on
// 2026-07-26, because the spec forbids relying on model memory for it:
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
//  * Archidekt's plain text export carries no per-line category at all. It
//    groups the cards under whole-line headers separated by blank lines:
//        Commander / Mainboard / Maybeboard
//    We found no Archidekt page documenting this; it is read off a list the
//    owner exported from Archidekt on 2026-10-10. The header is therefore the
//    only thing saying which cards are the commander and which are outside
//    the deck, so the parser applies it to every line beneath it.
//
// EXPORT therefore emits only `QTY Name [Category]`: we key cards on
// oracle_id and store neither set code nor finish, so there is nothing to
// gain from the risky fields. IMPORT accepts the whole superset, because the
// text being pasted back was produced by Archidekt, not by us.
// ---------------------------------------------------------------------------

// Category syntax rechecked 2026-10-17:
// https://archidekt.com/forum/thread/6024990/1
// https://archidekt.com/forum/thread/3223199

import type { DatabaseSync } from "node:sqlite";
import { ServiceError } from "../errors.ts";
import { type Actor, requireOwner } from "../actor.ts";
import { type CardRow, CardResolutionError, type ResolutionFailure, tryResolveCard } from "../cards.ts";
import { type Change, applyChanges } from "./membership.ts";
import { type Role, getDeck, shortlistCard } from "./service.ts";

// Categories we emit for cards that aren't in a user slot. "Commander" is
// Archidekt's own command-zone category name.
const COMMANDER_CATEGORY = "Commander";
const COMPANION_CATEGORY = "Companion";
const UNSLOTTED_CATEGORY = "Unslotted";
const SIDEBOARD_CATEGORY = "Sideboard";
const MAYBEBOARD_CATEGORY = "Maybeboard";

// Categories that mean "listed with the deck but not in it". Archidekt ships
// with Maybeboard and Sideboard categories that it excludes from the deck
// count; we could not find that stated in its documentation, so the import
// preview reports every card it sets aside this way rather than doing it
// silently.
const OUTSIDE_CATEGORIES = new Set(["maybeboard", "sideboard"]);

export interface ExportOptions {
  // Emit [Category] tags derived from slots / role. Off = bare "QTY Name".
  categories?: boolean;
  /** Restrict the export to these deck-card identities. An empty list exports nothing. */
  oracleIds?: string[];
  includeMaybe?: boolean;
}

export interface ExportResult {
  text: string;
  card_count: number;
  line_count: number;
  revision: number;
}

/** The deck as Archidekt-importable text, optionally restricted to a selection. */
export function exportDeck(
  db: DatabaseSync,
  deckId: number,
  opts: ExportOptions = {},
): ExportResult {
  const { deck, slots, cards } = getDeck(db, deckId);
  const slotName = new Map(slots.map((s) => [s.id, s.name]));
  const selected = opts.oracleIds === undefined ? null : new Set(opts.oracleIds);

  const categoryOf = (c: (typeof cards)[number]) => {
    if (c.maybe) return MAYBEBOARD_CATEGORY;
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
  order.set(MAYBEBOARD_CATEGORY, slots.length + 3);

  const rows = cards
    .filter((c) => (!c.maybe || opts.includeMaybe) && (!selected || selected.has(c.oracle_id)))
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
    revision: deck.revision,
  };
}

// ---------- parsing (permissive — accepts the whole Archidekt superset) ----------

export interface ParsedLine {
  line_no: number;
  raw: string;
  quantity: number;
  name: string;
  // The line's own [Category] tags, then the section header it sits under.
  categories: string[];
}

export interface ParseResult {
  entries: ParsedLine[];
  // Lines we could not read as a card at all (as opposed to names that simply
  // don't resolve — those come back from the diff, with suggestions).
  unparsed: Array<{ line_no: number; raw: string }>;
}

// Whole-line section headers Archidekt and other exporters emit, and the
// category each one gives the lines beneath it (null: the deck itself). None
// of these are Magic card names, so reading them as headers cannot swallow a
// real card.
const SECTIONS = new Map<string, string | null>([
  ["deck", null],
  ["decklist", null],
  ["main", null],
  ["mainboard", null],
  ["about", null],
  ["token", null],
  ["tokens", null],
  ["commander", COMMANDER_CATEGORY],
  ["commanders", COMMANDER_CATEGORY],
  ["companion", COMPANION_CATEGORY],
  ["sideboard", SIDEBOARD_CATEGORY],
  ["maybeboard", MAYBEBOARD_CATEGORY],
]);
const SECTION_HEADER = /^([A-Za-z]+)\s*:?\s*$/;
// "Creatures (24)" style count headers — documented to break the importer,
// so they turn up in lists people hand-edit.
const COUNT_HEADER = /^([A-Za-z][A-Za-z '\/-]*?)\s*\(\d+\)\s*$/;

export function parseArchidektList(text: string): ParseResult {
  const entries: ParsedLine[] = [];
  const unparsed: Array<{ line_no: number; raw: string }> = [];
  // The category of the header the current line sits under. It holds until
  // the next header, not the next blank line.
  let section: string | null = null;

  text.split(/\r?\n/).forEach((raw, i) => {
    const lineNo = i + 1;
    let line = raw.trim();
    if (!line) return;
    if (line.startsWith("//") || line.startsWith("#")) return;
    const header = line.match(SECTION_HEADER);
    if (header && SECTIONS.has(header[1].toLowerCase())) {
      section = SECTIONS.get(header[1].toLowerCase()) ?? null;
      return;
    }
    // A count header we don't know ("Creatures (24)") is a grouping inside
    // the deck, so it ends a Commander or Maybeboard section.
    const counted = line.match(COUNT_HEADER);
    if (counted) {
      section = SECTIONS.get(counted[1].toLowerCase()) ?? null;
      return;
    }
    const implied = section ? [section] : [];
    // MTGO sideboard prefix.
    if (/^SB:\s*/i.test(line)) {
      line = line.replace(/^SB:\s*/i, "");
      implied.push(SIDEBOARD_CATEGORY);
    }

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
    for (const c of implied)
      if (!categories.some((own) => own.toLowerCase() === c.toLowerCase())) categories.push(c);
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
    role: Role;
  }>;
  cuts: Array<{ oracle_id: string; name: string; quantity: number; role: string }>;
  quantity_changes: Array<{ oracle_id: string; name: string; from: number; to: number }>;
  // Cards already in the deck that the list names as commander or companion.
  // A list only ever promotes: a bare list carries no roles at all, so its
  // silence about the current commander is not an instruction to demote it.
  role_changes: Array<{ oracle_id: string; name: string; from: Role; to: Role }>;
  // Physical card counts, not distinct database rows. Quantity increases and
  // decreases contribute to these totals, so 30 Forests really reads as 30
  // cards to add in the preview.
  add_count: number;
  cut_count: number;
  unchanged: number;
  // Names the exact resolver could not match. Never guessed — suggestions are
  // shown for the owner to fix by hand (spec §4.3 applies to pasted text too:
  // exact match or nothing).
  unresolved: Array<{ line_no: number; name: string; suggestions: string[] }>;
  // The same printed name on several cards; needs the owner to pick.
  ambiguous: Array<{
    line_no: number;
    name: string;
    candidates: Array<{ name: string; oracle_id: string }>;
  }>;
  // Lines in a Maybeboard or Sideboard category or section: shortlisted, not added.
  set_aside: Array<{ oracle_id: string; name: string; category: string }>;
  unparsed: Array<{ line_no: number; raw: string }>;
  revision: number;
}

export function diffImport(db: DatabaseSync, deckId: number, text: string): ImportDiff {
  const { deck, slots, cards } = getDeck(db, deckId);
  const { entries, unparsed } = parseArchidektList(text);

  const slotByName = new Map(slots.map((s) => [s.name.toLowerCase(), s]));
  // The maybe list is not the deck: a pasted card that is only shortlisted
  // here is an add.
  const main = cards.filter((c) => !c.maybe);
  const inDeck = new Map(main.map((c) => [c.oracle_id, c]));

  const unresolved: ImportDiff["unresolved"] = [];
  const ambiguous: ImportDiff["ambiguous"] = [];
  const setAside = new Map<string, { oracle_id: string; name: string; category: string }>();

  // oracle_id → total quantity in the pasted list (a card may appear on more
  // than one line, e.g. split across categories).
  const pasted = new Map<string, { name: string; quantity: number; categories: string[] }>();

  for (const entry of entries) {
    const hit = tryResolveCard(db, entry.name);
    if ("problem" in hit) {
      const failure = hit as ResolutionFailure;
      if (failure.problem === "ambiguous")
        ambiguous.push({
          line_no: entry.line_no,
          name: entry.name,
          candidates: failure.candidates,
        });
      else
        unresolved.push({ line_no: entry.line_no, name: entry.name, suggestions: failure.suggestions });
      continue;
    }
    const card = hit as CardRow;
    const outside = entry.categories.find((c) => OUTSIDE_CATEGORIES.has(c.toLowerCase()));
    if (outside) {
      setAside.set(card.oracle_id, { oracle_id: card.oracle_id, name: card.name, category: outside });
      continue;
    }
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
  const roleChanges: ImportDiff["role_changes"] = [];
  let unchanged = 0;

  // Command-zone capacity left over once the cards absent from the pasted list
  // are gone. A list carrying its own Commander category or section should
  // land its commander in the command zone — that's what sets the deck's color
  // identity — but never past the 2/1 caps, which would throw mid-import.
  let commanderRoom =
    2 - main.filter((c) => c.role === "commander" && pasted.has(c.oracle_id)).length;
  let companionRoom =
    1 - main.filter((c) => c.role === "companion" && pasted.has(c.oracle_id)).length;
  const claimRole = (categories: string[]): Role => {
    const lowered = categories.map((c) => c.toLowerCase());
    if (lowered.some((c) => c === "commander" || c === "commanders") && commanderRoom > 0) {
      commanderRoom--;
      return "commander";
    }
    if (lowered.includes("companion") && companionRoom > 0) {
      companionRoom--;
      return "companion";
    }
    return "card";
  };

  for (const [oracleId, p] of pasted) {
    const existing = inDeck.get(oracleId);
    if (existing) {
      unchanged += Math.min(existing.quantity, p.quantity);
      if (existing.quantity !== p.quantity)
        quantityChanges.push({
          oracle_id: oracleId,
          name: p.name,
          from: existing.quantity,
          to: p.quantity,
        });
      if (existing.role === "card") {
        const role = claimRole(p.categories);
        if (role !== "card")
          roleChanges.push({ oracle_id: oracleId, name: p.name, from: existing.role, to: role });
      }
      continue;
    }
    // First category that names an existing slot wins; Commander/Companion/
    // Unslotted are our own export categories and map to no slot.
    const slot = p.categories.map((c) => slotByName.get(c.toLowerCase())).find(Boolean) ?? null;
    const role = claimRole(p.categories);
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

  const cuts = main
    .filter((c) => !pasted.has(c.oracle_id))
    .map((c) => ({
      oracle_id: c.oracle_id,
      name: c.name,
      quantity: c.quantity,
      role: c.role,
    }));

  const addCount =
    adds.reduce((total, card) => total + card.quantity, 0) +
    quantityChanges.reduce((total, change) => total + Math.max(0, change.to - change.from), 0);
  const cutCount =
    cuts.reduce((total, card) => total + card.quantity, 0) +
    quantityChanges.reduce((total, change) => total + Math.max(0, change.from - change.to), 0);

  return {
    adds: adds.sort((a, b) => a.name.localeCompare(b.name)),
    cuts: cuts.sort((a, b) => a.name.localeCompare(b.name)),
    quantity_changes: quantityChanges.sort((a, b) => a.name.localeCompare(b.name)),
    role_changes: roleChanges.sort((a, b) => a.name.localeCompare(b.name)),
    add_count: addCount,
    cut_count: cutCount,
    unchanged,
    unresolved,
    ambiguous,
    set_aside: [...setAside.values()].filter((c) => !pasted.has(c.oracle_id)),
    unparsed,
    revision: deck.revision,
  };
}

export interface ImportResult {
  applied: { added: number; cut: number; quantity_changed: number; role_changed: number };
  // Null when the list matched the deck exactly — nothing was written.
  log_id: number | null;
  diff: ImportDiff;
}

function changesFor(diff: ImportDiff): Change[] {
  // Cuts first, so a commander leaving frees its seat before one arrives.
  return [
    ...diff.cuts.map((c) => ({ action: "cut" as const, oracle_id: c.oracle_id, quantity: "all" as const })),
    ...diff.quantity_changes
      .filter((q) => q.to < q.from)
      .map((q) => ({ action: "cut" as const, oracle_id: q.oracle_id, quantity: q.from - q.to })),
    ...diff.role_changes.map((r) => ({ action: "role" as const, oracle_id: r.oracle_id, role: r.to })),
    ...diff.adds.map((a) => ({
      action: "add" as const,
      oracle_id: a.oracle_id,
      quantity: a.quantity,
      slot_id: a.slot_id ?? undefined,
      role: a.role,
    })),
    ...diff.quantity_changes
      .filter((q) => q.to > q.from)
      .map((q) => ({ action: "add" as const, oracle_id: q.oracle_id, quantity: q.to - q.from })),
  ];
}

function land(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  diff: ImportDiff,
  kind: "import" | "edit",
  note?: string,
): ImportResult {
  const applied = {
    added: diff.add_count,
    cut: diff.cut_count,
    quantity_changed: diff.quantity_changes.length,
    role_changed: diff.role_changes.length,
  };
  const summary =
    note?.trim() ||
    `Imported a list at revision ${diff.revision}: +${applied.added}/−${applied.cut}` +
      (applied.quantity_changed ? `, ${applied.quantity_changed} quantity change(s)` : "") +
      (applied.role_changed ? `, ${applied.role_changed} role change(s)` : "");
  const logId = applyChanges(db, actor, deckId, changesFor(diff), { kind, summary });
  for (const c of diff.set_aside) {
    const existing = getDeck(db, deckId).cards.find((x) => x.oracle_id === c.oracle_id);
    if (!existing) shortlistCard(db, deckId, c.oracle_id, `From the pasted list's ${c.category}`);
  }
  return { applied, log_id: logId, diff };
}

// The owner's paste-back (spec §12). It applies in full and immediately: a
// pasted list is the owner's own list, the preview diff is the confirmation,
// and the whole import is ONE log entry, so one undo reverts it. Names that do
// not resolve are reported and left out; they never block the rest.
export function applyImport(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  text: string,
  note?: string,
): ImportResult {
  requireOwner(actor, "import a list over an existing deck");
  return land(db, actor, deckId, diffImport(db, deckId, text), "import", note);
}

// Filling a deck from a pasted list on an agent's behalf (create_deck). The
// agent rule applies in full: one name that does not resolve fails the whole
// call (spec §4.3), and applyChanges refuses unless the deck is Theorycrafted.
export function fillFromList(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  text: string,
): ImportResult {
  const diff = diffImport(db, deckId, text);
  if (diff.unresolved.length || diff.ambiguous.length)
    throw new CardResolutionError([
      ...diff.unresolved.map((u) => ({
        ref: u.name,
        problem: "unknown" as const,
        suggestions: u.suggestions,
        candidates: [],
      })),
      ...diff.ambiguous.map((a) => ({
        ref: a.name,
        problem: "ambiguous" as const,
        suggestions: [],
        candidates: a.candidates,
      })),
    ]);
  if (diff.unparsed.length)
    throw new ServiceError(
      `Could not read these lines as cards, so nothing was added: ` +
        diff.unparsed.map((u) => `line ${u.line_no} "${u.raw}"`).join("; ") +
        `. Use one card per line as "1 Card Name".`,
    );
  return land(db, actor, deckId, diff, "edit", "Filled from a pasted list");
}
