import { requireDeckOwner } from "../actor.ts";
import { requireDeck } from "./service.ts";
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
//
// ---------------------------------------------------------------------------
// MOXFIELD — import only (spec §12), verified 2026-10-25 against Moxfield's
// help center and 72 real exports (12 public Commander decks × every button
// of the deck page's Export dialog; fixtures in test/fixtures/moxfield/):
//
//  * Line syntax, from Moxfield's "Creating decks" help article
//    (https://moxfield.com/help/creating-decks, updated 2026-06-16): "The
//    format to list them is: AMOUNT CARDNAME (SETCODE) NUMBER *F*. The set
//    code, collector number and foil indicator (*F*) are optional." Its
//    "Adding cards" article (https://moxfield.com/help/adding-cards) adds
//    tags: "1 Lightning Bolt (A25) *F* #OneDrop #!Removal".
//  * What the exporter writes: *F* for foil and *E* for etched; set codes of
//    3–5 characters (PLST, P30M); collector numbers that are not plain
//    digits (1492★, 1F★, 2018A, 259p, CSP-105, 2023-8); multi-face names with
//    ONE slash, "Wear / Tear" ("Copy for Moxfield") or "Wear/Tear" ("Copy for
//    MTGO"), where Scryfall writes "Wear // Tear".
//  * Boards other than the main deck follow a blank line under an upper-case
//    header with a colon: SIDEBOARD:, ATTRACTIONS:, STICKERS:. The
//    companion is an unmarked line under SIDEBOARD:. The maybeboard
//    ("Considering") is never exported.
//  * Names: "Copy Plain Text" and "Copy for Moxfield" use the printed name
//    Scryfall uses ("Spider-Punk"); "Copy for MTGO" uses MTGO's own name where
//    it differs ("Kraza, the Swarm as One"), which then fails to resolve and
//    is reported like any unknown name. Plain Text is the one to paste.
//    Every variant writes a flavor-named printing by its flavor name ("The
//    Ball" for Fellwar Stone); those are reported unresolved too (spec §12).
//  * The commander: "Copy for MTGO", "Download for MTGO" and "Copy Plain
//    Text" put it in a final block after a blank line, with no header (all
//    12 decks). "Copy for Moxfield" lists it first with no marker at all, so
//    it cannot be told from the deck; we do not guess (the alphabetical-order
//    break that would hint at it is wrong for 2 of the 12 decks), and the
//    preview says so. "Copy for Arena" uses About / Name / Companion /
//    Commander / Deck / Sideboard headings and drops cards Arena lacks.
//
// Moxfield publishes no import grammar beyond the help text, and its API is
// not public; nothing here fetches from Moxfield.
// ---------------------------------------------------------------------------

import type { Database } from "../sqlite.ts";
import { ServiceError } from "../errors.ts";
import { type Actor, requireUi } from "../actor.ts";
import { type CardRow, CardResolutionError, type ResolutionFailure, tryResolveCard } from "../cards.ts";
import { type Change, applyChanges } from "./membership.ts";
import { type Role, getDeck, shortlistCard } from "./service.ts";
import { publicDeckState } from "../state.ts";

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
// Moxfield boards that are separate decks in their own right, not cards for
// the 100 nor candidates for it. Listed in the preview and left out.
const IGNORED_CATEGORY = "Not part of the deck";

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
  db: Database, actor: Actor,
  deckId: number,
  opts: ExportOptions = {},
): ExportResult {
  requireDeck(db, actor, deckId);
  const view = getDeck(db, actor, deckId);
  return { ...exportLines(view, opts), revision: view.deck.revision };
}

/** A shared deck's list as Archidekt text, for anyone who can read it (§18.5).
 * Built from the public projection, so it carries nothing the page does not. */
export function exportPublicDeck(db: Database, viewer: Actor | null, slug: string, opts: Pick<ExportOptions, "categories" | "includeMaybe"> = {}): Omit<ExportResult, "revision"> & { name: string } {
  const shared = publicDeckState(db, viewer, slug);
  return { name: shared.deck.name, ...exportLines(shared, opts) };
}

function exportLines(
  { slots, cards }: { slots: Array<{ id: number; name: string }>; cards: Array<{ oracle_id: string; name: string; quantity: number; role: Role; maybe: number; slot_id: number | null }> },
  opts: ExportOptions,
): Omit<ExportResult, "revision"> {
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
  /** Read as Moxfield's unlabelled command zone; kept only if the card can be a commander. */
  guessed_commander?: boolean;
}

/** A Commander list is a few hundred lines; a paste far past that is not a deck. */
export const MAX_LIST_LINES = 5000;
/** The longest card name is 141 characters; a line this long is not a card. */
export const MAX_LINE_LENGTH = 300;
/** Trailing fields one line may carry (^label^, [categories], `category`, #tags, *F*, (SET) number). */
const MAX_LINE_FIELDS = 12;

export interface ParseResult {
  entries: ParsedLine[];
  // Lines we could not read as a card at all (as opposed to names that simply
  // don't resolve — those come back from the diff, with suggestions).
  unparsed: Array<{ line_no: number; raw: string }>;
  // Lines under a board that is not part of a Commander deck (Moxfield's
  // ATTRACTIONS:, STICKERS:), left out and reported.
  ignored: Array<{ line_no: number; name: string; section: string }>;
  /** What the text says about where it came from, for the preview's notes. */
  notes: string[];
}

// Whole-line section headers Archidekt, Moxfield and other exporters emit,
// and the category each gives the lines beneath it (null: the deck itself).
// None of these are Magic card names, so reading them as headers cannot
// swallow a real card.
const SECTIONS = new Map<string, string | null>([
  ["deck", null],
  ["decklist", null],
  ["main", null],
  ["mainboard", null],
  ["token", null],
  ["tokens", null],
  ["commander", COMMANDER_CATEGORY],
  ["commanders", COMMANDER_CATEGORY],
  ["companion", COMPANION_CATEGORY],
  ["sideboard", SIDEBOARD_CATEGORY],
  ["maybeboard", MAYBEBOARD_CATEGORY],
  // Moxfield's name for its maybeboard.
  ["considering", MAYBEBOARD_CATEGORY],
  ["attractions", IGNORED_CATEGORY],
  ["stickers", IGNORED_CATEGORY],
  ["contraptions", IGNORED_CATEGORY],
]);
// Arena's "About" block names the deck ("Name Tom Bombadil + Jegantha"); its
// lines are not cards.
const ABOUT = "about";
const SECTION_HEADER = /^([A-Za-z]+)\s*:?\s*$/;
// Moxfield's board headers: upper case with a colon. A board ends at the next
// blank line, after which an unlabelled block is the command zone.
const BOARD_HEADER = /^[A-Z]+:$/;
// "Creatures (24)" style count headers — documented to break the importer,
// so they turn up in lists people hand-edit.
const COUNT_HEADER = /^([A-Za-z][A-Za-z '\/-]*?)\s*\(\d+\)\s*$/;
// "(SET) NUMBER": 2–6 character set codes, collector numbers with letters,
// stars and dashes as Moxfield writes them (259p, 1492★, CSP-105).
// Tested on the text from the line's last "(", so it is anchored at both ends.
const PRINTING = /^\([0-9A-Za-z_]{2,6}\)( [0-9A-Za-z][0-9A-Za-z★†-]*)?$/;

export function parseDeckList(text: string): ParseResult {
  const lines = text.split(/\r?\n/);
  // Bounded before any regex runs: the end-anchored patterns below are
  // quadratic on long runs of whitespace, and nothing here may stall the server.
  if (lines.length > MAX_LIST_LINES)
    throw new ServiceError(`A pasted list can have at most ${MAX_LIST_LINES.toLocaleString("en")} lines; this one has ${lines.length.toLocaleString("en")}.`);
  const entries: ParsedLine[] = [];
  const unparsed: Array<{ line_no: number; raw: string }> = [];
  const ignored: ParseResult["ignored"] = [];
  // The category of the header the current line sits under. It holds until
  // the next header, not the next blank line — except a Moxfield board, which
  // a blank line ends.
  let section: string | null = null;
  // The header's own words, for reporting ignored lines ("STICKERS").
  let sectionLabel = "";
  let skipping = false;
  let board = false;
  let explicitCommandZone = false;
  let wordHeaders = false;
  // Blocks are runs of lines between blank lines; the last one may be
  // Moxfield's unlabelled command zone.
  let block = 0;
  let blockLabelled = false;
  const blocks: Array<{ labelled: boolean; entries: ParsedLine[] }> = [];
  let printed = 0;

  lines.forEach((raw, i) => {
    const lineNo = i + 1;
    // trim() is linear; a blank line (whatever its length) still separates
    // blocks, and a comment of any length is skipped, before the cap applies.
    let line = raw.trim();
    if (!line) {
      if (blocks[block]?.entries.length || blockLabelled) { block++; blockLabelled = false; }
      if (board) { section = null; board = false; sectionLabel = ""; }
      return;
    }
    if (line.startsWith("//") || line.startsWith("#")) return;
    if (line.length > MAX_LINE_LENGTH) {
      unparsed.push({ line_no: lineNo, raw: `${raw.slice(0, 80)}…` });
      return;
    }
    // Every pattern below is anchored at the end and begins with \s*: on a
    // long run of spaces each start position would rescan the run. One
    // space per run makes each pattern linear (card names never hold two).
    line = line.replace(/\s+/g, " ");
    const header = line.match(SECTION_HEADER);
    const key = header?.[1].toLowerCase();
    if (header && (SECTIONS.has(key!) || key === ABOUT)) {
      skipping = key === ABOUT;
      section = skipping ? null : SECTIONS.get(key!) ?? null;
      sectionLabel = header[1];
      board = BOARD_HEADER.test(line);
      if (!board) wordHeaders = true;
      if (section === COMMANDER_CATEGORY) explicitCommandZone = true;
      blockLabelled = true;
      return;
    }
    if (skipping) return;
    // A count header we don't know ("Creatures (24)") is a grouping inside
    // the deck, so it ends a Commander or Maybeboard section.
    const counted = line.match(COUNT_HEADER);
    if (counted) {
      section = SECTIONS.get(counted[1].toLowerCase()) ?? null;
      sectionLabel = counted[1];
      wordHeaders = true;
      if (section === COMMANDER_CATEGORY) explicitCommandZone = true;
      blockLabelled = true;
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

    // Strip trailing metadata in any order. Each field is found from the end
    // (endsWith / lastIndexOf) and only its own tail is tested, so a pass is
    // one linear look and a name containing e.g. "//" is never eaten.
    const categories: string[] = [];
    // Passes are capped: a real line has a handful of fields (label,
    // category, tags, finish, printing). Whatever is left over stays in the
    // name and fails to resolve, visibly.
    for (let pass = 0; pass < MAX_LINE_FIELDS; pass++) {
      const before = line;
      const cut = (at: number) => { line = line.slice(0, at).trimEnd(); };
      // ^Label,#rrggbb^
      if (line.endsWith("^")) {
        const at = line.lastIndexOf("^", line.length - 2);
        if (at >= 0) cut(at);
      }
      // [Cat] or [Cat1,Cat2]
      if (line.endsWith("]")) {
        const at = line.lastIndexOf("[");
        const inside = at >= 0 ? line.slice(at + 1, -1) : "]";
        if (!inside.includes("]")) {
          categories.unshift(
            ...inside
              .split(",")
              .map((c) => c.trim())
              // Archidekt category modifiers ({top}, {noDeck}, …) are reported
              // by users but we could not verify their syntax — strip anything
              // brace-wrapped rather than guess at its meaning.
              .map((c) => c.replace(/\{[^}]*\}/g, "").trim())
              .filter(Boolean),
          );
          cut(at);
        }
      }
      // `Category`
      if (line.endsWith("`")) {
        const at = line.lastIndexOf("`", line.length - 2);
        if (at >= 0) {
          const c = line.slice(at + 1, -1).trim();
          if (c) categories.unshift(c);
          cut(at);
        }
      }
      // Moxfield tags: #Tag (this deck) and #!Tag (every deck). Like an
      // Archidekt category, a tag naming one of the deck's slots files it.
      const space = line.lastIndexOf(" ");
      const tag = space >= 0 ? line.slice(space + 1).match(/^#!?([^\s#]+)$/) : null;
      if (tag) {
        categories.unshift(tag[1]);
        cut(space);
      }
      // *F*, *E*, *F**E*
      while (/^\*[A-Za-z]\*$/.test(line.slice(-3))) cut(line.length - 3);
      // (set) optionally followed by a collector number
      const open = line.lastIndexOf("(");
      if (open >= 0 && PRINTING.test(line.slice(open))) {
        printed++;
        cut(open);
      }
      if (line === before) break;
    }

    const name = line.trim();
    if (!name || quantity < 1) {
      unparsed.push({ line_no: lineNo, raw });
      return;
    }
    for (const c of implied)
      if (!categories.some((own) => own.toLowerCase() === c.toLowerCase())) categories.push(c);
    if (categories.includes(IGNORED_CATEGORY)) {
      ignored.push({ line_no: lineNo, name, section: sectionLabel });
      return;
    }
    const entry = { line_no: lineNo, raw, quantity, name, categories };
    entries.push(entry);
    blocks[block] ??= { labelled: blockLabelled, entries: [] };
    blocks[block].entries.push(entry);
  });

  // Moxfield's MTGO and plain-text exports end with the command zone: one or
  // two lines in a final block of their own, after a blank line, under no
  // header. Read that way only when the text has the export's shape: no
  // command zone named anywhere (header or [Commander]), no word headers
  // (Archidekt's way of marking one), and a deck of 90–100 cards counting the
  // block. diffImport then keeps only cards that can be commanders.
  const filled = blocks.filter(Boolean);
  const last = filled.at(-1);
  const named = explicitCommandZone || entries.some((e) => e.categories.some((c) => /^commanders?$/i.test(c)));
  const deckCopies = entries
    .filter((e) => !e.categories.some((c) => OUTSIDE_CATEGORIES.has(c.toLowerCase())) || last?.entries.includes(e))
    .reduce((n, e) => n + e.quantity, 0);
  let guessed = false;
  if (!named && !wordHeaders && last && filled.length > 1 && !last.labelled && last.entries.length <= 2 && deckCopies >= 90 && deckCopies <= 100) {
    guessed = true;
    for (const entry of last.entries) {
      entry.categories = entry.categories.filter((c) => !OUTSIDE_CATEGORIES.has(c.toLowerCase()));
      entry.categories.unshift(COMMANDER_CATEGORY);
      entry.guessed_commander = true;
    }
  }

  const notes: string[] = [];
  if (entries.length && printed >= entries.length * 0.9 && !explicitCommandZone && !entries.some((e) => e.categories.includes(COMMANDER_CATEGORY)))
    notes.push(
      "This looks like Moxfield's “Copy for Moxfield” export, which does not mark the commander. " +
        "Set the commander after importing, or paste Moxfield's “Copy Plain Text” export instead: it lists the commander last.",
    );
  if (guessed && entries.some((e) => e.categories.includes(SIDEBOARD_CATEGORY)))
    notes.push(
      "Moxfield keeps a companion under SIDEBOARD:, so sideboard cards go to the maybe list. " +
        "If one of them is the companion, set it as the companion by hand after importing.",
    );
  if (ignored.length)
    notes.push(`${ignored.length} line${ignored.length === 1 ? " is" : "s are"} on a board that is not part of a Commander deck (attractions, stickers or contraptions) and ${ignored.length === 1 ? "is" : "are"} left out.`);
  return { entries, unparsed, ignored, notes };
}

/** Commander-eligible (§5.1), or a Background, which shares the command zone. */
function canLeadADeck(card: CardRow): boolean {
  return !!card.is_commander || /\bBackground\b/.test(card.type_line);
}

/** Distinct unresolved names that get suggestions in one paste. A miss's
 * suggestion search is its costly part; past this a list is not a typo. */
const MAX_SUGGESTED = 25;

/** A name as written, then as Moxfield's one-slash multi-face form (§12). */
function resolveListed(db: Database, name: string, suggest: boolean) {
  const hit = tryResolveCard(db, name, { suggest });
  if (!("problem" in hit) || !name.includes("/") || name.includes("//")) return hit;
  const faces = tryResolveCard(db, name.split(/\s*\/\s*/).join(" // "), { suggest: false });
  return "problem" in faces ? hit : faces;
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
  ignored: ParseResult["ignored"];
  notes: string[];
  revision: number;
}

export function diffImport(db: Database, actor: Actor, deckId: number, text: string): ImportDiff {
  requireDeck(db, actor, deckId);
  const { deck, slots, cards } = getDeck(db, actor, deckId);
  const { entries, unparsed, ignored, notes } = parseDeckList(text);

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

  const resolved = new Map<string, ReturnType<typeof resolveListed>>();
  let suggested = 0;
  for (const entry of entries) {
    // Each distinct name is resolved once per paste.
    const key = entry.name.toLowerCase();
    let hit = resolved.get(key);
    if (!hit) {
      hit = resolveListed(db, entry.name, suggested < MAX_SUGGESTED);
      if ("problem" in hit && hit.problem === "unknown") suggested++;
      resolved.set(key, hit);
    }
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
    // A guessed command zone holds only what could be a commander there.
    if (entry.guessed_commander && !canLeadADeck(card))
      entry.categories = entry.categories.filter((c) => c !== COMMANDER_CATEGORY);
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
    ignored,
    notes,
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
  db: Database,
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
    const existing = getDeck(db, actor, deckId).cards.find((x) => x.oracle_id === c.oracle_id);
    if (!existing) shortlistCard(db, actor, deckId, c.oracle_id, `From the pasted list's ${c.category}`);
  }
  return { applied, log_id: logId, diff };
}

// The owner's paste-back (spec §12). It applies in full and immediately: a
// pasted list is the owner's own list, the preview diff is the confirmation,
// and the whole import is ONE log entry, so one undo reverts it. Names that do
// not resolve are reported and left out; they never block the rest.
export function applyImport(
  db: Database,
  actor: Actor,
  deckId: number,
  text: string,
  note?: string,
): ImportResult {
  requireDeckOwner(db, actor, deckId);
  requireUi(actor, "import a list over an existing deck");
  return land(db, actor, deckId, diffImport(db, actor, deckId, text), "import", note);
}

// Filling a deck from a pasted list on an agent's behalf (create_deck). The
// agent rule applies in full: one name that does not resolve fails the whole
// call (spec §4.3), and applyChanges refuses unless the deck is Theorycrafted.
export function fillFromList(
  db: Database,
  actor: Actor,
  deckId: number,
  text: string,
): ImportResult {
  requireDeckOwner(db, actor, deckId);
  const diff = diffImport(db, actor, deckId, text);
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
