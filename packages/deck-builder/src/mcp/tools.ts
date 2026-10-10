// The agent's tools (spec §10.3). Few, shaped around tasks, each a thin
// adapter: resolve names, call the capability layer, render text. Every rule
// an agent is held to is enforced below this file, in the same functions the
// UI calls — nothing here decides what an agent may do.
//
// A tool is a plain function of (context, arguments) so it can be tested
// without a transport; mcp/http.ts registers the same list with the SDK.

import type { DatabaseSync } from "node:sqlite";
import * as z from "zod/v4";
import { type Actor, PermissionError } from "../actor.ts";
import { recordActivity } from "../activity.ts";
import { type CardRow, lintProse, resolveCard, resolveCards, tryResolveCard } from "../cards.ts";
import { withTransaction } from "../db.ts";
import { ServiceError } from "../errors.ts";
import { getFocus } from "../focus.ts";
import { compareDecks, forkDeck } from "../deck/fork.ts";
import { exportDeck, fillFromList } from "../deck/interop.ts";
import { cardHistories, listLog, markReviewed, unreviewedCount } from "../deck/log.ts";
import { type Change, applyChanges } from "../deck/membership.ts";
import { paperStatus } from "../deck/paper.ts";
import { createProposal, getProposal } from "../deck/proposals.ts";
import {
  createDeck,
  defineSlots,
  fileCards,
  findDeckCard,
  getDeck,
  listDecks,
  requireDeck,
  requireSlotByName,
  shortlistCard,
  unshortlistCard,
} from "../deck/service.ts";
import { getProfile, proposeDocumentEdit } from "../documents.ts";
import {
  type MemoryView,
  listMemories,
  loadScope,
  memoriesForCards,
  remember,
  reviseMemory,
} from "../memory.ts";
import { SearchError, search } from "../search/index.ts";
import { playerStats } from "../stats.ts";
import { PLAYBOOK } from "./playbook.ts";
import {
  DECK_SECTIONS,
  afterWrite,
  cardWithContext,
  logLines,
  memoryLine,
  renderCompare,
  renderDeck,
  renderMemories,
  renderPaper,
  renderScope,
  renderStats,
  resolutionFailureLine,
  rosterLines,
  rulingLine,
} from "./render.ts";

export interface ToolContext {
  db: DatabaseSync;
  actor: Actor;
}

export interface ToolResult {
  text: string;
  /** Set by tools that wrote something, so the app can refresh what is on screen. */
  changed?: boolean;
}

export interface ToolDef {
  name: string;
  title: string;
  description: string;
  input: z.ZodRawShape;
  readOnly: boolean;
  run: (ctx: ToolContext, args: any) => string | ToolResult;
}

// ---------- shared argument shapes ----------

const deckRef = z
  .union([z.number().int(), z.string()])
  .describe(
    'A deck: its id (a number from the roster), its exact name, or "focus" for the deck the owner has open in the app.',
  );
const cardRef = z
  .string()
  .describe("A card: its exact name as a tool returned it, or its oracle_id. Never a guess.");

function deckId(db: DatabaseSync, ref: number | string): number {
  if (typeof ref === "number") return requireDeck(db, ref).id;
  const text = ref.trim();
  if (/^\d+$/.test(text)) return requireDeck(db, Number(text)).id;
  if (text.toLowerCase() === "focus") {
    const focus = getFocus(db);
    if (!focus)
      throw new ServiceError(
        `The owner has no deck open in the app, so "focus" names nothing. Use a deck id:\n${rosterLines(listDecks(db))}`,
      );
    return focus.deck_id;
  }
  const named = listDecks(db).filter((d) => d.name.toLowerCase() === text.toLowerCase());
  if (named.length === 1) return named[0].id;
  throw new ServiceError(`There is no deck named "${ref}". The decks are:\n${rosterLines(listDecks(db))}`);
}

const done = (text: string): ToolResult => ({ text, changed: true });

function myDecksFor(db: DatabaseSync, oracleIds: string[], exceptDeck?: number): Map<string, string[]> {
  const stmt = db.prepare(
    `SELECT d.name FROM deck_cards dc JOIN decks d ON d.id = dc.deck_id
     WHERE dc.oracle_id = ? AND dc.maybe = 0 AND dc.deck_id != ? ORDER BY d.name`,
  );
  const out = new Map<string, string[]>();
  for (const id of new Set(oracleIds)) {
    const names = (stmt.all(id, exceptDeck ?? -1) as unknown as { name: string }[]).map((r) => r.name);
    if (names.length) out.set(id, names);
  }
  return out;
}

// The context the app attaches to cards wherever they appear (spec §7.2,
// §8.3): linked memories always, prior rulings and membership when a deck is
// named, other decks only when the call asked (§4.2).
function contextFor(
  db: DatabaseSync,
  cards: CardRow[],
  deck: number | undefined,
  includeMyDecks: boolean,
) {
  const ids = cards.map((c) => c.oracle_id);
  const memories = memoriesForCards(db, ids, deck);
  const rulings = deck === undefined ? new Map() : cardHistories(db, deck, ids);
  const mine = includeMyDecks ? myDecksFor(db, ids, deck) : new Map<string, string[]>();
  return (card: CardRow) => {
    const row = deck === undefined ? undefined : findDeckCard(db, deck, card.oracle_id);
    return {
      memories: memories.get(card.oracle_id),
      rulings: rulings.get(card.oracle_id),
      myDecks: mine.get(card.oracle_id),
      membership: row ? (row.maybe ? "on this deck's maybe list" : "already in this deck") : undefined,
    };
  };
}

function requireDraftForAgent(db: DatabaseSync, actor: Actor, deck: number) {
  const row = requireDeck(db, deck);
  if (actor.kind === "agent" && row.status !== "draft")
    throw new PermissionError(
      `"${row.name}" is an active deck, so its cards change only through a proposal the owner accepts. ` +
        `Use propose_changes. (Agents edit draft decks directly; only the owner can return a deck to draft.)`,
    );
}

// ---------- the tools ----------

export const TOOLS: ToolDef[] = [
  {
    name: "start_session",
    title: "Start a session",
    readOnly: true,
    description:
      "Call this first, once per conversation. Returns the playbook (the rules for working with this app), " +
      "the owner's profile, your global memories, the deck roster with ids, the deck the owner has open " +
      "(the focus), and how many of the owner's rulings you have not read yet.",
    input: {},
    run: ({ db }) => {
      const profile = getProfile(db);
      const focus = getFocus(db);
      const unreviewed = unreviewedCount(db);
      return [
        PLAYBOOK,
        `# The owner's profile\n${profile.body_md || "(empty — the owner has not written one. If you learn durable things about them, propose a profile with propose_profile_edit.)"}` +
          (profile.over_cap
            ? `\n(The profile is ${profile.word_count} words, over its soft cap. Suggest tightening it.)`
            : ""),
        `# Your global memories\n${renderScope(loadScope(db, "global"), "global")}`,
        `# Decks\n${rosterLines(listDecks(db))}`,
        `# Focus\n${
          focus
            ? `The owner has deck ${focus.deck_id} "${focus.deck_name}" open` +
              (focus.selected.length ? `, with these cards selected: ${focus.selected.join(", ")}` : "") +
              ` (reported ${focus.age_minutes} minute(s) ago).`
            : "The owner has no deck open in the app right now."
        }`,
        `# Rulings you have not read\n${
          unreviewed
            ? `${unreviewed}. Read them with list_rulings (unreviewed_only: true) before proposing anything, then mark_rulings_reviewed.`
            : "None."
        }`,
      ].join("\n\n");
    },
  },
  {
    name: "player_stats",
    title: "Player statistics",
    readOnly: true,
    description:
      "What the owner builds, computed across all their decks: colours and identities, commanders and their " +
      "creature types, average curve and land counts, slot habits, and the cards they reuse across decks. " +
      "Use this for questions about the owner's tastes or what to build next. These are facts the app " +
      "computes; never store them as memories.",
    input: {},
    run: ({ db }) => renderStats(playerStats(db)),
  },
  {
    name: "get_deck",
    title: "Read a deck",
    readOnly: true,
    description:
      "One deck in full: its brief, the list grouped by slot with oracle text, the maybe list, the app's " +
      "computed counts, check results, the memories for this deck, what is awaiting the owner's ruling, " +
      "and recent rulings. Read a deck before discussing or changing it. To re-read cheaply after a " +
      "change, pass only the sections you need, or card_text: false for names without oracle text.",
    input: {
      deck: deckRef,
      sections: z
        .array(z.enum(DECK_SECTIONS))
        .optional()
        .describe("Which parts to return. Omit for everything."),
      card_text: z
        .boolean()
        .optional()
        .describe("Include each card's oracle text (default true)."),
    },
    run: ({ db }, a) => renderDeck(db, deckId(db, a.deck), a.sections ?? DECK_SECTIONS, a.card_text ?? true),
  },
  {
    name: "compare_decks",
    title: "Compare two decks",
    readOnly: true,
    description:
      "The cards only in one deck, only in the other, and how many they share. With one deck named, " +
      "compares a fork against the deck it was forked from. Use it to see how a fork has diverged, or how " +
      "two of the owner's decks overlap.",
    input: {
      deck: deckRef,
      other: deckRef.optional().describe("The deck to compare against. Omit to use the first deck's parent."),
    },
    run: ({ db }, a) =>
      renderCompare(compareDecks(db, deckId(db, a.deck), a.other === undefined ? undefined : deckId(db, a.other))),
  },
  {
    name: "search_cards",
    title: "Search cards",
    readOnly: true,
    description:
      "Search every Magic card with a subset of Scryfall syntax. Bare words match the name; " +
      't:creature (type), o:"draw a card" (oracle text), fo: (text including reminder text), ' +
      "mv<=2 or cmc>=5, c:rg (colours, at least these), id<=wub (colour identity within), " +
      "pow>=4, tou<3, loy=3, is:commander, f:commander, banned:commander. Combine with spaces (AND), " +
      "'or', '-' or 'not', and parentheses. With a deck named, results are limited to that deck's colour " +
      "identity, marked if already in it, and carry the owner's prior rulings on each card in that deck; " +
      "slot:ramp, slot:none and tag:name then filter to that deck's own cards. Memories linked to a card " +
      "are always attached. Results come back alphabetically, 20 at a time: make several narrow searches " +
      "rather than one broad one.",
    input: {
      query: z.string().describe('The search, e.g. t:artifact o:"add {C}" mv<=2'),
      deck: deckRef.optional().describe("Search in the context of this deck (recommended when building one)."),
      include_my_decks: z
        .boolean()
        .optional()
        .describe(
          "Also say which of the owner's other decks contain each card. Off by default, because it " +
            "biases recommendations toward cards they already use; turn it on only when that is the question.",
        ),
      ignore_identity: z
        .boolean()
        .optional()
        .describe("With a deck named, do not limit results to its colour identity."),
      limit: z.number().int().min(1).max(50).optional().describe("How many results (default 20, max 50)."),
    },
    run: ({ db }, a) => {
      const deck = a.deck === undefined ? undefined : deckId(db, a.deck);
      const limit = a.limit ?? 20;
      let results: CardRow[];
      try {
        results = search(db, a.query, {
          deckId: deck,
          filterByDeckIdentity: !a.ignore_identity,
          limit: limit + 1,
        }) as unknown as CardRow[];
      } catch (e) {
        if (e instanceof SearchError)
          throw new ServiceError(
            `That search could not be read: ${e.message}. Supported keys: t, o, fo, mv/cmc, c, id, pow, tou, loy, ` +
              `is:commander, f:commander, banned:commander, and with a deck: slot:, tag:. Quote phrases: o:"draw a card".`,
          );
        throw e;
      }
      const more = results.length > limit;
      const shown = results.slice(0, limit);
      if (!shown.length)
        return (
          `No cards match "${a.query}"` +
          (deck !== undefined && !a.ignore_identity
            ? ` within this deck's colour identity. If the card may be outside it, search again with ignore_identity: true.`
            : `. Loosen the query, or check the syntax.`)
        );
      const ctx = contextFor(db, shown, deck, !!a.include_my_decks);
      return [
        `${shown.length} result(s) for "${a.query}"${deck !== undefined ? ` in the context of deck ${deck}` : ""}` +
          (more ? ` — there are more; narrow the query to see them.` : "."),
        ...shown.map((c) => cardWithContext(c, ctx(c))),
      ].join("\n");
    },
  },
  {
    name: "get_cards",
    title: "Look up cards by name",
    readOnly: true,
    description:
      "Exact-name lookup of up to 25 cards: full oracle text, memories linked to each card, and, with a " +
      "deck named, whether it is in that deck and the owner's prior rulings on it there. This is also how " +
      "to check that names are real before you rely on them: a name that is not a card comes back marked " +
      "NOT A CARD with the closest real names, and the rest of the lookup still succeeds.",
    input: {
      cards: z.array(cardRef).min(1).max(25),
      deck: deckRef.optional(),
      include_my_decks: z
        .boolean()
        .optional()
        .describe("Also say which of the owner's other decks contain each card. Off by default."),
    },
    run: ({ db }, a) => {
      const deck = a.deck === undefined ? undefined : deckId(db, a.deck);
      const resolved = (a.cards as string[]).map((ref) => tryResolveCard(db, ref));
      const found = resolved.filter((r): r is CardRow => !("problem" in r));
      const ctx = contextFor(db, found, deck, !!a.include_my_decks);
      return resolved
        .map((r) => ("problem" in r ? resolutionFailureLine(r) : cardWithContext(r, ctx(r))))
        .join("\n");
    },
  },
  {
    name: "list_rulings",
    title: "List rulings and changes",
    readOnly: true,
    description:
      "The rulings log, newest first: every accept and reject with the owner's reason, plus direct edits, " +
      "imports and undos. Use unreviewed_only: true to read what the owner decided while you were not " +
      "running; after deciding what is worth a memory, call mark_rulings_reviewed.",
    input: {
      deck: deckRef.optional().describe("Only this deck. Omit for every deck and the profile."),
      unreviewed_only: z.boolean().optional(),
      since: z.string().optional().describe("Only entries on or after this date (YYYY-MM-DD)."),
      limit: z.number().int().min(1).max(200).optional().describe("Default 30."),
    },
    run: ({ db }, a) => {
      const entries = listLog(db, {
        deckId: a.deck === undefined ? undefined : deckId(db, a.deck),
        unreviewedOnly: a.unreviewed_only,
        since: a.since,
        limit: a.limit ?? 30,
      });
      const waiting = unreviewedCount(db);
      const unread = entries.filter((e) => e.reviewed_at == null);
      return (
        `${entries.length} entr${entries.length === 1 ? "y" : "ies"}` +
        (a.unreviewed_only ? " awaiting your review" : "") +
        `. ${waiting} unreviewed in total.\n${logLines(entries)}` +
        // The exact follow-up, because the first agent to do this guessed at
        // the arguments.
        (unread.length
          ? `\n\nOnce you have decided what here is worth a memory, call mark_rulings_reviewed with ` +
            `{"through": ${Math.max(...unread.map((e) => e.id))}}.`
          : "")
      );
    },
  },
  {
    name: "search_memories",
    title: "Search memories",
    readOnly: true,
    description:
      "Find memories beyond those already loaded: by words (full-text, best match first), by scope, by " +
      "deck, or by a linked card. Global memories load in start_session and a deck's load in get_deck; " +
      "use this when one of those said some did not fit, when you need another deck's memories, or " +
      "before writing a memory you suspect already exists.",
    input: {
      query: z.string().optional().describe("Words to look for."),
      scope: z.enum(["global", "deck"]).optional(),
      deck: deckRef.optional(),
      card: cardRef.optional().describe("Only memories linked to this card."),
      include_retired: z
        .boolean()
        .optional()
        .describe("Also return superseded and retired memories (the history)."),
      limit: z.number().int().min(1).max(100).optional().describe("Default 25."),
    },
    run: ({ db }, a) =>
      renderMemories(
        listMemories(db, {
          query: a.query,
          scope: a.scope,
          deckId: a.deck === undefined ? undefined : deckId(db, a.deck),
          cardId: a.card ? resolveCard(db, a.card).oracle_id : undefined,
          status: a.include_retired ? "any" : "active",
          limit: a.limit ?? 25,
        }),
      ),
  },
  {
    name: "paper_status",
    title: "Paper status of a deck",
    readOnly: true,
    description:
      "Which cards of a deck are missing from the physical deck, split into a pull list (another of the " +
      "owner's decks physically holds a copy) and a buy list (none does). This is the only place the " +
      "physical state of decks is visible. Use it when the owner asks what they still need to assemble a " +
      "deck; do not use it to decide what to recommend.",
    input: { deck: deckRef },
    run: ({ db }, a) => renderPaper(paperStatus(db, deckId(db, a.deck))),
  },
  {
    name: "export_deck",
    title: "Export a deck as text",
    readOnly: true,
    description:
      "The deck as plain text that Archidekt and most other sites import: one line per card, " +
      '"1 Card Name [Category]", command zone first. The maybe list is not included.',
    input: {
      deck: deckRef,
      categories: z.boolean().optional().describe("Include [Category] from slots (default true)."),
      only_missing: z
        .boolean()
        .optional()
        .describe("Only cards missing from the physical deck: a buy list."),
    },
    run: ({ db }, a) => {
      const r = exportDeck(db, deckId(db, a.deck), {
        categories: a.categories,
        onlyMissing: a.only_missing,
      });
      return `${r.card_count} card(s) at revision ${r.revision}:\n\n${r.text}`;
    },
  },

  // ---------- writes that need no ruling ----------

  {
    name: "create_deck",
    title: "Create a draft deck",
    readOnly: false,
    description:
      "Create a new deck. It starts as a draft, which you edit directly with edit_draft until the owner " +
      "makes it active. Optionally set the commanders, paste a whole list, and write the thesis in the " +
      "same call. If any card name does not resolve, nothing is created.",
    input: {
      name: z.string().describe("A name not already used by another deck."),
      commanders: z.array(cardRef).max(2).optional().describe("One commander, or two that may be paired."),
      list: z
        .string()
        .optional()
        .describe(
          'A decklist as text, one card per line: "1 Card Name". A line ending in [Commander] goes to the ' +
            "command zone; other [Category] names that match a slot file the card there.",
        ),
      thesis: z.string().optional().describe("What the deck is trying to do. Cards as [[Card Name]]."),
    },
    run: ({ db, actor }, a) => {
      const commanders = resolveCards(db, a.commanders ?? []);
      const id = withTransaction(db, () => {
        const deck = createDeck(db, a.name, { status: "draft" });
        if (a.list?.trim()) fillFromList(db, actor, deck, a.list);
        const changes: Change[] = commanders.map((c) => {
          const row = findDeckCard(db, deck, c.oracle_id);
          return row && !row.maybe
            ? { action: "role", oracle_id: c.oracle_id, role: "commander" }
            : { action: "add", oracle_id: c.oracle_id, role: "commander" };
        });
        if (changes.length) applyChanges(db, actor, deck, changes, { kind: "edit", summary: "Set the commander" });
        if (a.thesis?.trim()) proposeDocumentEdit(db, actor, deck, "thesis", { content: a.thesis }, "");
        return deck;
      });
      recordActivity(db, actor, "create_deck", id, `Created draft deck "${a.name}"`);
      return done(`Created draft deck ${id} "${a.name.trim()}".\n${afterWrite(db, id)}`);
    },
  },
  {
    name: "fork_deck",
    title: "Fork a deck",
    readOnly: false,
    description:
      "Copy a deck — its list, slots, tags, maybe list, brief and deck memories — into a new draft that " +
      "records where it came from. Use it when the owner wants to make big changes to a deck but keep the " +
      "original. The original is untouched; compare_decks shows how the two diverge.",
    input: { deck: deckRef, name: z.string().describe("The new deck's name.") },
    run: ({ db, actor }, a) => {
      const parent = deckId(db, a.deck);
      const id = forkDeck(db, actor, parent, a.name);
      recordActivity(db, actor, "fork_deck", id, `Forked "${requireDeck(db, parent).name}" as "${a.name}"`);
      return done(
        `Forked into draft deck ${id} "${a.name.trim()}". It is a draft: edit it with edit_draft.\n` +
          `It starts with the original's brief, memories and rulings history. If the fork has a different ` +
          `aim, rewrite its thesis first (edit_draft with thesis), then change the cards.\n${afterWrite(db, id)}`,
      );
    },
  },
  {
    name: "edit_draft",
    title: "Edit a draft deck",
    readOnly: false,
    description:
      "Change a DRAFT deck directly: add and remove cards, set the commanders, write the thesis and " +
      "constraints. Everything in one call lands together as one logged change, or none of it does. On an " +
      "active deck this fails; use propose_changes there. Returns the new counts and check results.",
    input: {
      deck: deckRef,
      add: z
        .array(
          z.object({
            card: cardRef,
            quantity: z.number().int().min(1).optional().describe("Copies to add (default 1)."),
            slot: z.string().optional().describe("An existing slot to file the card in."),
          }),
        )
        .optional(),
      remove: z
        .array(
          z.object({
            card: cardRef,
            quantity: z
              .number()
              .int()
              .min(1)
              .optional()
              .describe("Copies to remove. Omit to remove the card entirely."),
            to_maybe: z.boolean().optional().describe("Move it to the maybe list instead of out of the deck."),
          }),
        )
        .optional(),
      commanders: z
        .array(cardRef)
        .max(2)
        .optional()
        .describe(
          "Replace the command zone with exactly these cards (added if not in the deck). A former " +
            "commander stays in the deck as an ordinary card unless you also remove it.",
        ),
      thesis: z.string().optional().describe("Replace the brief's thesis."),
      constraints: z.string().optional().describe("Replace the brief's constraints."),
      note: z.string().optional().describe("One line for the log saying what this edit was."),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, a.deck);
      requireDraftForAgent(db, actor, deck);
      const adds = (a.add ?? []) as Array<{ card: string; quantity?: number; slot?: string }>;
      const removes = (a.remove ?? []) as Array<{ card: string; quantity?: number; to_maybe?: boolean }>;
      const addCards = resolveCards(db, adds.map((x) => x.card));
      const removeCards = resolveCards(db, removes.map((x) => x.card));
      const commanders = a.commanders === undefined ? null : resolveCards(db, a.commanders);

      const changes: Change[] = [];
      if (commanders) {
        const keep = new Set(commanders.map((c) => c.oracle_id));
        for (const c of getDeck(db, deck).cards)
          if (c.role === "commander" && !c.maybe && !keep.has(c.oracle_id))
            changes.push({ action: "role", oracle_id: c.oracle_id, role: "card" });
      }
      removes.forEach((r, i) =>
        changes.push({
          action: "cut",
          oracle_id: removeCards[i].oracle_id,
          quantity: r.quantity ?? "all",
          park: r.to_maybe,
        }),
      );
      adds.forEach((x, i) =>
        changes.push({
          action: "add",
          oracle_id: addCards[i].oracle_id,
          quantity: x.quantity,
          slot_id: x.slot ? requireSlotByName(db, deck, x.slot).id : undefined,
        }),
      );
      for (const c of commanders ?? []) {
        const row = findDeckCard(db, deck, c.oracle_id);
        const beingAdded = addCards.some((x) => x.oracle_id === c.oracle_id);
        changes.push(
          (row && !row.maybe) || beingAdded
            ? { action: "role", oracle_id: c.oracle_id, role: "commander" }
            : { action: "add", oracle_id: c.oracle_id, role: "commander" },
        );
      }

      withTransaction(db, () => {
        if (changes.length)
          applyChanges(db, actor, deck, changes, { kind: "edit", summary: a.note?.trim() || undefined });
        if (a.thesis !== undefined) proposeDocumentEdit(db, actor, deck, "thesis", { content: a.thesis }, "");
        if (a.constraints !== undefined)
          proposeDocumentEdit(db, actor, deck, "constraints", { content: a.constraints }, "");
      });
      const did = [
        adds.length ? `added ${adds.length}` : null,
        removes.length ? `removed ${removes.length}` : null,
        commanders ? "set commanders" : null,
        a.thesis !== undefined || a.constraints !== undefined ? "wrote the brief" : null,
      ].filter(Boolean);
      if (!did.length) throw new ServiceError("Nothing to do: give cards to add or remove, commanders, or brief text");
      recordActivity(db, actor, "edit_draft", deck, `${did.join(", ")}${a.note ? ` — ${a.note}` : ""}`);
      return done(`Done (${did.join(", ")}).\n${afterWrite(db, deck)}`);
    },
  },
  {
    name: "define_slots",
    title: "Define a deck's slots",
    readOnly: false,
    description:
      "Create, retarget, rename and remove slots on any deck, draft or active; this is organization and " +
      "needs no ruling. A slot says what a card does (ramp, interaction, card advantage) and may carry a " +
      "soft target count or range. A name you give that does not exist is created; one that exists is " +
      "updated. Card types (land, creatures…) cannot be slot names. All of it lands or none does.",
    input: {
      deck: deckRef,
      slots: z
        .array(
          z.object({
            name: z.string(),
            rename_to: z.string().optional(),
            target_min: z.number().int().min(0).nullable().optional().describe("null clears it."),
            target_max: z.number().int().min(0).nullable().optional().describe("null clears it."),
            remove: z.boolean().optional().describe("Remove the slot; its cards become unslotted."),
          }),
        )
        .min(1),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, a.deck);
      defineSlots(db, deck, a.slots);
      recordActivity(db, actor, "define_slots", deck, `Slots: ${a.slots.map((s: any) => (s.remove ? `removed ${s.name}` : s.rename_to ?? s.name)).join(", ")}`);
      return done(`Slots updated.\n${renderDeck(db, deck, ["state"])}`);
    },
  },
  {
    name: "file_cards",
    title: "File cards into slots and tags",
    readOnly: false,
    description:
      "Set the slot and tags of cards already in a deck (or on its maybe list), on any deck; this is " +
      "organization and needs no ruling. File many cards in one call; if any card or slot name is wrong, " +
      "nothing is filed. A tag that does not exist yet is created, unless it would duplicate an existing " +
      "tag (same words, different spelling), in which case the call fails and names the tag to use.",
    input: {
      deck: deckRef,
      filings: z
        .array(
          z.object({
            card: cardRef,
            slot: z.string().nullable().optional().describe("Slot name; null unslots; omit to leave it."),
            add_tags: z.array(z.string()).optional(),
            remove_tags: z.array(z.string()).optional(),
          }),
        )
        .min(1),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, a.deck);
      const filings = a.filings as Array<{ card: string; slot?: string | null; add_tags?: string[]; remove_tags?: string[] }>;
      const cards = resolveCards(db, filings.map((f) => f.card));
      fileCards(
        db,
        deck,
        filings.map((f, i) => ({ ...f, oracle_id: cards[i].oracle_id })),
      );
      recordActivity(db, actor, "file_cards", deck, `Filed ${cards.length} card(s)`);
      return done(`Filed ${cards.length} card(s).\n${renderDeck(db, deck, ["state"])}`);
    },
  },
  {
    name: "shortlist_cards",
    title: "Add to or remove from the maybe list",
    readOnly: false,
    description:
      "Put cards on a deck's maybe list, or take them off it, on any deck; needs no ruling. The maybe list " +
      "is outside the 100: use it to park candidates worth considering, each with a note saying why. " +
      "Moving a card from the deck itself to the maybe list is a cut and goes through edit_draft or " +
      "propose_changes.",
    input: {
      deck: deckRef,
      add: z
        .array(z.object({ card: cardRef, note: z.string().optional().describe("Why it is worth considering. Cards as [[Card Name]].") }))
        .optional(),
      remove: z.array(cardRef).optional(),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, a.deck);
      const adds = (a.add ?? []) as Array<{ card: string; note?: string }>;
      const addCards = resolveCards(db, adds.map((x) => x.card));
      const removeCards = resolveCards(db, a.remove ?? []);
      if (!adds.length && !removeCards.length) throw new ServiceError("Give cards to add or remove");
      withTransaction(db, () => {
        adds.forEach((x, i) => {
          if (x.note) lintProse(db, x.note);
          shortlistCard(db, deck, addCards[i].oracle_id, x.note ?? "");
        });
        for (const c of removeCards) unshortlistCard(db, deck, c.oracle_id);
      });
      recordActivity(
        db,
        actor,
        "shortlist_cards",
        deck,
        [
          addCards.length ? `shortlisted ${addCards.map((c) => c.name).join(", ")}` : null,
          removeCards.length ? `dropped ${removeCards.map((c) => c.name).join(", ")}` : null,
        ]
          .filter(Boolean)
          .join("; "),
      );
      return done(`Maybe list updated.\n${renderDeck(db, deck, ["maybe"], false)}`);
    },
  },
  {
    name: "remember",
    title: "Write a memory",
    readOnly: false,
    description:
      "Save one thing worth knowing next time: a preference, a correction, the reason behind a ruling, a " +
      "fact about the owner's playgroup, how a deck played. No approval is needed; the owner can see and " +
      "edit every memory. One statement and its reason per memory. Never save what a tool can tell you or " +
      "what a card says. The result lists existing memories that resemble this one: if one is the same " +
      "idea, use revise_memory on it instead of leaving both.",
    input: {
      text: z.string().describe("The statement and its reason. Card names as [[Card Name]]; they are linked automatically."),
      scope: z.enum(["global", "deck"]).describe("'global' is about the owner; 'deck' is about one deck."),
      deck: deckRef.optional().describe("Required when scope is 'deck'."),
      cards: z.array(cardRef).optional().describe("Other cards to link, beyond those named in the text."),
      basis: z
        .enum(["stated", "inferred"])
        .optional()
        .describe("'stated' only if the owner said it; 'inferred' (the default) for your own conclusion."),
      source_ruling: z.number().int().optional().describe("The id of the ruling that prompted this, if one did."),
    },
    run: ({ db, actor }, a) => {
      const deck = a.scope === "deck" ? (a.deck === undefined ? null : deckId(db, a.deck)) : null;
      const { memory, similar } = remember(db, actor, {
        text: a.text,
        scope: a.scope,
        deck_id: deck,
        cards: resolveCards(db, a.cards ?? []).map((c) => c.oracle_id),
        basis: a.basis,
        source_log_id: a.source_ruling,
      });
      recordActivity(db, actor, "remember", deck, `Memory #${memory.id}: ${memory.text}`);
      const loaded = loadScope(db, memory.scope, deck ?? undefined);
      return done(
        [
          `Saved.\n${memoryLine(memory)}`,
          similar.length
            ? `These existing memories resemble it. If one is the same idea, supersede or retire it with revise_memory so both do not stand:\n${similar.map(memoryLine).join("\n")}`
            : null,
          loaded.over
            ? `This scope is now over budget (${loaded.omitted} memories no longer load). Consolidate it.`
            : null,
        ]
          .filter(Boolean)
          .join("\n\n"),
      );
    },
  },
  {
    name: "revise_memory",
    title: "Revise a memory",
    readOnly: false,
    description:
      "Change an existing memory. 'update' edits it in place (a correction or tightening). 'supersede' " +
      "replaces it with new text and keeps the old one as history — use this when the fact has changed, " +
      "or with scope 'global' to promote a deck memory. 'retire' takes it out of use without deleting it. " +
      "'confirm' marks an inferred memory as stated once the owner has confirmed it. 'restore' brings a " +
      "retired memory back. You cannot change a memory the owner wrote or edited.",
    input: {
      id: z.number().int(),
      action: z.enum(["update", "supersede", "retire", "confirm", "restore"]),
      text: z.string().optional().describe("New text, for update and supersede."),
      cards: z.array(cardRef).optional().describe("Replace the linked cards."),
      basis: z.enum(["stated", "inferred"]).optional(),
      scope: z.enum(["global", "deck"]).optional().describe("For supersede: the replacement's scope."),
      deck: deckRef.optional().describe("For supersede into deck scope."),
    },
    run: ({ db, actor }, a) => {
      const cards = a.cards ? resolveCards(db, a.cards).map((c) => c.oracle_id) : undefined;
      let result: MemoryView;
      if (a.action === "update") {
        result = reviseMemory(db, actor, a.id, { action: "update", text: a.text, cards, basis: a.basis });
      } else if (a.action === "supersede") {
        if (!a.text?.trim()) throw new ServiceError("Superseding needs the replacement text");
        result = reviseMemory(db, actor, a.id, {
          action: "supersede",
          text: a.text,
          cards,
          basis: a.basis,
          scope: a.scope,
          deck_id: a.deck === undefined ? undefined : deckId(db, a.deck),
        });
      } else {
        result = reviseMemory(db, actor, a.id, { action: a.action });
      }
      recordActivity(db, actor, "revise_memory", result.deck_id, `${a.action} memory #${a.id}${result.id !== a.id ? ` → #${result.id}` : ""}: ${result.text}`);
      return done(
        `${a.action === "supersede" ? `Memory #${a.id} is now superseded by:` : "Done:"}\n${memoryLine(result)}`,
      );
    },
  },
  {
    name: "mark_rulings_reviewed",
    title: "Mark rulings as read",
    readOnly: false,
    description:
      "Tell the app you have read rulings from list_rulings and decided what, if anything, is worth a " +
      "memory. Give the ids, or 'through' to clear everything up to and including an id.",
    input: {
      ids: z.array(z.number().int()).optional(),
      through: z.number().int().optional(),
    },
    run: ({ db, actor }, a) => {
      const n = markReviewed(db, { ids: a.ids, through: a.through });
      recordActivity(db, actor, "mark_rulings_reviewed", null, `Reviewed ${n} ruling(s)`);
      return done(`Marked ${n} as reviewed. ${unreviewedCount(db)} still unreviewed.`);
    },
  },

  // ---------- proposals ----------

  {
    name: "propose_changes",
    title: "Propose changes to a deck",
    readOnly: false,
    description:
      "Propose adding and cutting cards. This is the only way to change which cards are in an ACTIVE deck: " +
      "the owner rules on each item in the app, and you learn the outcome from list_rulings. Propose as " +
      "many items as the request calls for. Every item needs its own reason (write cards as [[Card Name]], " +
      "and quote the oracle text you rely on). Give items that only make sense together the same group " +
      "so they are accepted or rejected as a unit. The result shows what the deck would become and any " +
      "prior rulings and memories on the cards involved.",
    input: {
      deck: deckRef,
      summary: z.string().optional().describe("One line on what this proposal is for."),
      items: z
        .array(
          z.object({
            action: z.enum(["add", "cut"]),
            card: cardRef,
            reason: z.string().describe("Why. The owner reads this when ruling."),
            slot: z.string().optional().describe("For an add: an existing slot to file the card in."),
            group: z.string().optional().describe("A label shared by items to be ruled on together."),
          }),
        )
        .min(1),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, a.deck);
      const items = a.items as Array<{ action: "add" | "cut"; card: string; reason: string; slot?: string; group?: string }>;
      const cards = resolveCards(db, items.map((i) => i.card));
      const id = createProposal(db, actor, deck, {
        summary: a.summary,
        items: items.map((i, n) => ({
          action: i.action,
          oracle_id: cards[n].oracle_id,
          reason: i.reason,
          slot_id: i.slot ? requireSlotByName(db, deck, i.slot).id : undefined,
          group: i.group,
        })),
      });
      const proposal = getProposal(db, id)!;
      recordActivity(db, actor, "propose_changes", deck, `Proposal #${id}: ${proposal.items.length} item(s)${a.summary ? ` — ${a.summary}` : ""}`);

      const view = getDeck(db, deck);
      const c = view.computed;
      const ids = cards.map((x) => x.oracle_id);
      const rulings = cardHistories(db, deck, ids);
      const memories = memoriesForCards(db, ids, deck);
      const context = cards
        .filter((x) => rulings.has(x.oracle_id) || memories.has(x.oracle_id))
        .map((x) =>
          [
            `- ${x.name}:`,
            ...(rulings.get(x.oracle_id) ?? []).map((r) => `  ${rulingLine(r)}`),
            ...(memories.get(x.oracle_id) ?? []).map((m) => `  ${memoryLine(m)}`),
          ].join("\n"),
        );
      return done(
        [
          `Proposal #${id} is waiting for the owner's ruling in the app (${proposal.items.length} item(s)).`,
          `The deck is ${c.card_count}/100. Awaiting ruling across all proposals: +${c.pending.adds} / −${c.pending.cuts}; ` +
            `if all were accepted it would be ${c.pending.projected_count}/100.`,
          context.length
            ? `What the app already knows about these cards. If your reasons did not account for it, tell the owner now:\n${context.join("\n")}`
            : null,
        ]
          .filter(Boolean)
          .join("\n\n"),
      );
    },
  },
  {
    name: "propose_brief_edit",
    title: "Propose an edit to a deck's brief",
    readOnly: false,
    description:
      "Propose a change to a deck's brief: replace the thesis or the constraints, define or replace a " +
      "named engine (with the cards that are its required pieces), or remove one. The brief is the " +
      "owner's document, so on an active deck this waits for their ruling and needs a reason citing what " +
      "prompted it. On a draft deck it applies at once.",
    input: {
      deck: deckRef,
      kind: z.enum(["thesis", "constraints", "engine_set", "engine_remove"]),
      text: z.string().optional().describe("For thesis / constraints: the full replacement text."),
      engine_name: z.string().optional().describe("For engine_set / engine_remove."),
      description: z.string().optional().describe("For engine_set: what the engine does."),
      pieces: z
        .array(z.object({ card: cardRef, note: z.string().optional() }))
        .optional()
        .describe("For engine_set: the cards the engine needs."),
      reason: z.string().optional().describe("What prompted this (rulings, memories). Required on an active deck."),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, a.deck);
      const pieces = (a.pieces ?? []) as Array<{ card: string; note?: string }>;
      const cards = resolveCards(db, pieces.map((p) => p.card));
      const r = proposeDocumentEdit(
        db,
        actor,
        deck,
        a.kind,
        {
          content: a.text,
          engine_name: a.engine_name,
          description: a.description,
          pieces: a.kind === "engine_set" ? pieces.map((p, i) => ({ oracle_id: cards[i].oracle_id, note: p.note })) : undefined,
        },
        a.reason ?? "",
      );
      recordActivity(db, actor, "propose_brief_edit", deck, r.applied ? `Wrote the brief (${a.kind}) of a draft` : `Proposed brief edit #${r.edit_id} (${a.kind})`);
      return done(
        r.applied
          ? `Applied: this deck is a draft, so its brief is yours to write.\n${renderDeck(db, deck, ["brief"])}`
          : `Proposed edit #${r.edit_id} is waiting for the owner's ruling in the app.`,
      );
    },
  },
  {
    name: "propose_profile_edit",
    title: "Propose an edit to the owner's profile",
    readOnly: false,
    description:
      "Propose new text for the owner's profile: who they are as a builder — tastes, playgroup and power " +
      "level, budget stance, standing dislikes. Give the complete replacement text; the owner sees a diff " +
      "against the current profile and rules on it. Use this when memories have settled into something " +
      "durable, and cite them in the reason.",
    input: {
      text: z.string().describe("The complete new profile, in markdown. Keep it under about 600 words."),
      reason: z.string().describe("What prompted this: the rulings and memories it rests on."),
    },
    run: ({ db, actor }, a) => {
      const r = proposeDocumentEdit(db, actor, null, "profile", { content: a.text }, a.reason);
      recordActivity(db, actor, "propose_profile_edit", null, r.applied ? "Edited the profile" : `Proposed profile edit #${r.edit_id}`);
      return done(
        r.applied ? "The profile was updated." : `Proposed edit #${r.edit_id} is waiting for the owner's ruling in the app.`,
      );
    },
  },
];

// ---------- calling a tool without a transport ----------

const SCHEMAS = new Map(TOOLS.map((t) => [t.name, z.object(t.input)]));

/** Validate arguments and run a tool. Errors propagate; the transport turns them into tool errors. */
export function callTool(ctx: ToolContext, name: string, args: unknown = {}): ToolResult {
  const tool = TOOLS.find((t) => t.name === name);
  if (!tool) throw new ServiceError(`Unknown tool '${name}'`, 404);
  const parsed = SCHEMAS.get(name)!.safeParse(args ?? {});
  if (!parsed.success)
    throw new ServiceError(
      `Invalid arguments for ${name}: ${parsed.error.issues.map((i) => `${i.path.join(".") || "(arguments)"}: ${i.message}`).join("; ")}`,
    );
  const result = tool.run(ctx, parsed.data);
  return typeof result === "string" ? { text: result } : result;
}
