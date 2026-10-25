// Decks, their organization, and the composite deck view. What is *in* a deck
// changes only through deck/membership.ts; everything here is either deck
// lifecycle, organization (spec §5.4 — slots, tags, the maybe list, none of
// which touches the 100), or reading.

import type { Database } from "../sqlite.ts";
import { isUniqueViolation, ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import { type Actor, type DeckRole, isDeckOwner, requireDeckOwner, authorOf, PermissionError, requireUi } from "../actor.ts";
import { recordActivity } from "../activity.ts";
import { requireCardById } from "../cards.ts";
import { estimateCost, pricesAsOf } from "../prices.ts";

export type Role = "card" | "commander" | "companion";
export type DeckCategory = "paper" | "theorycrafted" | "legacy";

// Card types and supertypes. Slots and tags say what a card *does* — "ramp",
// "interaction" — and card type says what it *is*; the decklist groups by
// either one, so they have to stay separate axes. A slot named "lands" makes
// "34 lands" ambiguous about which of the two it counts, so both numbers are
// rejected: "land" and "lands", "sorcery" and "sorceries".
const CARD_TYPE_NAMES = new Set([
  "land", "creature", "artifact", "enchantment", "instant", "sorcery",
  "planeswalker", "battle", "kindred", "tribal", "legendary", "basic", "snow", "token",
]);

// Reserved for a different reason: slot:/tag: queries would stop parsing.
// Only the exact word collides, so these aren't pluralised.
const RESERVED_NAMES = new Set([
  // search prefixes
  "t", "type", "o", "oracle", "fo", "fulloracle", "cmc", "mv", "manavalue",
  "c", "color", "id", "identity", "pow", "power", "tou", "toughness",
  "loy", "loyalty", "usd", "is", "f", "format", "banned", "slot", "tag",
  // grammar words and sentinels
  "or", "and", "not", "none",
]);

// Enough to fold the plural of every word in CARD_TYPE_NAMES onto its singular
// ("sorceries" is the only irregular one); not a general-purpose stemmer.
function singular(word: string): string {
  if (word.endsWith("ies")) return `${word.slice(0, -3)}y`;
  if (word.endsWith("s") && !word.endsWith("ss")) return word.slice(0, -1);
  return word;
}

export function validateVocabName(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) throw new ServiceError("Name cannot be empty");
  const lower = trimmed.toLowerCase();
  if (CARD_TYPE_NAMES.has(singular(lower)))
    throw new ServiceError(
      `'${trimmed}' is a card type, and card types are reserved: slots and tags describe ` +
        `what a card does, and the decklist already groups by card type on its own.`,
    );
  if (RESERVED_NAMES.has(lower))
    throw new ServiceError(
      `'${trimmed}' is reserved (search prefixes cannot be slot or tag names)`,
    );
  return trimmed;
}

const WORD_NUMBERS: Record<string, number> = {
  one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7,
  eight: 8, nine: 9, ten: 10, eleven: 11, twelve: 12,
};

// How many copies a card allows: 1 (singleton), a printed cap ("up to seven
// cards named..."), or Infinity (basics, "any number of cards named...").
export function copyLimit(card: { type_line: string; oracle_text?: string }): number {
  if (/\bBasic\b/.test(card.type_line)) return Infinity;
  const text = (card.oracle_text ?? "").toLowerCase();
  if (text.includes("any number of cards named")) return Infinity;
  const m = text.match(/up to (\w+) cards named/);
  if (m) return WORD_NUMBERS[m[1]] ?? 1;
  return 1;
}

// ---------- deck lifecycle ----------

export interface DeckRow {
  user_id: number;
  role: DeckRole;
  id: number;
  name: string;
  category: DeckCategory;
  parent_id: number | null;
  ci_mask: number | null;
  color_identity: string;
  revision: number;
  created_at: string;
}

export function createDeck(
  db: Database,
  actor: Actor,
  name: string,
  opts: { category?: DeckCategory; parentId?: number | null } = {},
): number {
  const trimmed = name.trim();
  if (!trimmed) throw new ServiceError("Deck name cannot be empty");
  if (opts.category !== undefined && !["paper", "theorycrafted", "legacy"].includes(opts.category))
    throw new ServiceError("Deck category must be 'paper', 'theorycrafted' or 'legacy'");
  if (opts.category !== undefined && opts.category !== "theorycrafted") requireUi(actor, "classify a new deck");
  if (opts.parentId != null) requireDeckOwner(db, actor, opts.parentId, "fork this deck");
  try {
    return withTransaction(db, () => {
      const r = db.prepare("INSERT INTO decks (user_id, name, category, parent_id) VALUES (?, ?, ?, ?)")
        .run(actor.user_id, trimmed, opts.category ?? "theorycrafted", opts.parentId ?? null);
      const id = Number(r.lastInsertRowid);
      recordActivity(db, actor, "create_deck", id, `Created "${trimmed}"`);
      return id;
    });
  } catch (e: any) {
    if (isUniqueViolation(e))
      throw new ServiceError(`A deck named '${trimmed}' already exists. Choose another name.`, 409);
    throw e;
  }
}

export function renameDeck(db: Database, actor: Actor, deckId: number, name: string): void {
  requireDeckOwner(db, actor, deckId);
  const deck = requireEditableDeck(db, actor, deckId);
  const trimmed = name.trim();
  if (!trimmed) throw new ServiceError("Deck name cannot be empty");
  try {
    withTransaction(db, () => {
      db.prepare("UPDATE decks SET name = ? WHERE id = ?").run(trimmed, deckId);
      recordActivity(db, actor, "rename_deck", deckId, `Renamed "${deck.name}" to "${trimmed}"`);
    });
  } catch (e: any) {
    if (isUniqueViolation(e))
      throw new ServiceError(`A deck named '${trimmed}' already exists`, 409);
    throw e;
  }
}

export function deleteDeck(db: Database, actor: Actor, deckId: number): void {
  requireDeckOwner(db, actor, deckId);
  const deck = requireEditableDeck(db, actor, deckId);
  withTransaction(db, () => {
    db.prepare("DELETE FROM decks WHERE id = ?").run(deckId);
    // The deck no longer exists; its name stays in the audit summary.
    recordActivity(db, actor, "delete_deck", null, `Deleted deck ${deckId} "${deck.name}" and its associated records`);
  });
}

/** Only the owner changes lifecycle: it controls agent permissions. */
export function setDeckCategory(
  db: Database,
  actor: Actor,
  deckId: number,
  category: DeckCategory,
): void {
  requireDeckOwner(db, actor, deckId);
  requireDeckOwner(db, actor, deckId, "change a deck’s lifecycle");
  requireUi(actor, "change a deck’s lifecycle");
  const deck = requireDeck(db, actor, deckId);
  if (category !== "paper" && category !== "theorycrafted" && category !== "legacy")
    throw new ServiceError("Deck category must be 'paper', 'theorycrafted' or 'legacy'");
  withTransaction(db, () => {
    db.prepare("UPDATE decks SET category = ? WHERE id = ?").run(category, deckId);
    recordActivity(db, actor, "set_deck_category", deckId, `Changed "${deck.name}" from ${deck.category} to ${category}`);
  });
}

// Record, or clear, which deck this one was forked from (spec §5.3). For a
// deck that was copied by hand before forks existed, this is how it gets the
// link that lets the two be compared and read as one family.
export function setDeckParent(
  db: Database,
  actor: Actor,
  deckId: number,
  parentId: number | null,
): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  requireUi(actor, "change which deck a deck was forked from");
  const deck = requireDeck(db, actor, deckId);
  if (parentId != null) {
    requireDeckOwner(db, actor, parentId, "use this parent deck");
    // Walking up from the proposed parent must not arrive back here.
    const seen = new Set<number>();
    for (let cur: number | null = parentId; cur != null; cur = requireDeckOwner(db, actor, cur).parent_id) {
      if (cur === deckId)
        throw new ServiceError(
          `"${requireDeck(db, actor, parentId).name}" is itself descended from "${deck.name}", so it cannot be its parent`,
        );
      if (seen.has(cur)) break;
      seen.add(cur);
    }
  }
  db.prepare("UPDATE decks SET parent_id = ? WHERE id = ?").run(parentId, deckId);
}

export interface DeckSummary {
  id: number;
  name: string;
  category: DeckCategory;
  parent_id: number | null;
  parent_name: string | null;
  color_identity: string;
  revision: number;
  created_at: string;
  card_count: number;
  commanders: string[];
  pending_items: number;
  pending_document_edits: number;
}

/** The roster: one line per deck, enough to choose which one to open. */
export function listDecks(db: Database, actor: Actor): DeckSummary[] {
  const rows = db
    .prepare(
      `SELECT d.id, d.name, d.category, d.parent_id, p.name AS parent_name, d.color_identity,
              d.revision, d.created_at,
              COALESCE((SELECT SUM(quantity) FROM deck_cards dc
                        WHERE dc.deck_id = d.id AND dc.role != 'companion' AND dc.maybe = 0), 0) AS card_count,
              (SELECT COUNT(*) FROM proposal_items pi JOIN proposals pr ON pr.id = pi.proposal_id
               WHERE pr.deck_id = d.id AND pr.status = 'open' AND pi.status = 'pending') AS pending_items,
              (SELECT COUNT(*) FROM document_edits de
               WHERE de.deck_id = d.id AND de.status = 'pending') AS pending_document_edits
       FROM decks d LEFT JOIN decks p ON p.id = d.parent_id AND p.user_id = d.user_id WHERE d.user_id = ? ORDER BY d.name`,
    )
    .all(actor.user_id) as unknown as Array<Omit<DeckSummary, "commanders">>;
  const commanders = db.prepare(
    `SELECT c.name FROM deck_cards dc JOIN cards c ON c.oracle_id = dc.oracle_id
     WHERE dc.deck_id = ? AND dc.role = 'commander' ORDER BY c.name`,
  );
  return rows.map((r) => ({
    ...r,
    commanders: (commanders.all(r.id) as unknown as { name: string }[]).map((c) => c.name),
  }));
}

export function requireDeck(db: Database, actor: Actor, deckId: number): DeckRow {
  const deck = db.prepare("SELECT * FROM decks WHERE id = ?").get(deckId) as DeckRow | undefined;
  if (!deck || deck.user_id !== actor.user_id)
    throw new ServiceError(
      `There is no deck ${deckId}. List the decks to see which ids exist.`,
      404,
    );
  return { ...deck, role: "owner" };
}

/** Guard a membership revision before a dependent mutation. */
export function requireRevision(db: Database, actor: Actor, deckId: number, expected?: number): void {
  const deck = requireDeck(db, actor, deckId);
  if (expected !== undefined && expected !== deck.revision)
    throw new ServiceError(`Deck revision changed: expected ${expected}, current ${deck.revision}. Read the deck again.`, 409);
}

/** All writes to an archived deck pass this guard, including organization. */
export function requireEditableDeck(db: Database, actor: Actor, deckId: number): DeckRow {
  const deck = requireDeckOwner(db, actor, deckId);
  if (deck.category === "legacy")
    throw new PermissionError(`"${deck.name}" is a Legacy deck and is read-only. Fork it to explore changes, or have the owner restore it to Paper or Theorycrafted.`);
  return deck;
}

/** The shared permission gate for direct membership changes. */
export function requireDirectCardEdit(db: Database, actor: Actor, deckId: number): DeckRow {
  const deck = requireEditableDeck(db, actor, deckId);
  if (actor.via === "agent" && deck.category === "paper")
    throw new PermissionError(`"${deck.name}" is a Paper deck, so its cards change only through a proposal the owner accepts. Use propose_changes. Agents edit Theorycrafted cards directly with edit_cards; only the owner can change lifecycle.`);
  return deck;
}

// Deck identity = union of commander identities (spec §5.1, denormalized).
export function recomputeIdentity(db: Database, actor: Actor, deckId: number): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  const commanders = db
    .prepare(
      `SELECT c.ci_mask, c.color_identity FROM deck_cards dc JOIN cards c ON c.oracle_id = dc.oracle_id
       WHERE dc.deck_id = ? AND dc.role = 'commander' AND dc.maybe = 0`,
    )
    .all(deckId) as unknown as Array<{ ci_mask: number; color_identity: string }>;
  if (!commanders.length) {
    db.prepare("UPDATE decks SET ci_mask = NULL, color_identity = '' WHERE id = ?").run(deckId);
    return;
  }
  let mask = 0;
  const letters = new Set<string>();
  for (const c of commanders) {
    mask |= c.ci_mask;
    for (const ch of c.color_identity) letters.add(ch);
  }
  const identity = ["W", "U", "B", "R", "G"].filter((c) => letters.has(c)).join("");
  db.prepare("UPDATE decks SET ci_mask = ?, color_identity = ? WHERE id = ?").run(
    mask,
    identity,
    deckId,
  );
}

export interface DeckCardRow {
  oracle_id: string;
  /** The card's primary purpose. */
  slot_id: number | null;
  secondary_slot_ids: number[];
  role: Role;
  maybe: number;
  maybe_note: string;
  quantity: number;
}

export function findDeckCard(
  db: Database, actor: Actor,
  deckId: number,
  oracleId: string,
): DeckCardRow | undefined {
  requireDeck(db, actor, deckId);
  const row = db
    .prepare(
      `SELECT oracle_id, slot_id, role, maybe, maybe_note, quantity
       FROM deck_cards WHERE deck_id = ? AND oracle_id = ?`,
    )
    .get(deckId, oracleId) as DeckCardRow | undefined;
  if (row) row.secondary_slot_ids = secondarySlotsOf(db, actor, deckId, oracleId);
  return row;
}

export function secondarySlotsOf(db: Database, actor: Actor, deckId: number, oracleId: string): number[] {
  requireDeck(db, actor, deckId);
  return (db.prepare("SELECT slot_id FROM deck_card_secondary_slots WHERE deck_id = ? AND oracle_id = ? ORDER BY slot_id")
    .all(deckId, oracleId) as unknown as { slot_id: number }[]).map(r => r.slot_id);
}

function requireDeckCard(db: Database, actor: Actor, deckId: number, oracleId: string): DeckCardRow {
  const row = findDeckCard(db, actor, deckId, oracleId);
  if (!row) {
    const name = requireCardById(db, oracleId).name;
    throw new ServiceError(`${name} is not in this deck or on its maybe list`, 404);
  }
  return row;
}

// ---------- slots ----------

export interface SlotRow {
  id: number;
  name: string;
  target_min: number | null;
  target_max: number | null;
  position: number;
}

export function listSlots(db: Database, actor: Actor, deckId: number): SlotRow[] {
  requireDeck(db, actor, deckId);
  return db
    .prepare(
      "SELECT id, name, target_min, target_max, position FROM slots WHERE deck_id = ? ORDER BY position, id",
    )
    .all(deckId) as unknown as SlotRow[];
}

function requireSlot(db: Database, actor: Actor, deckId: number, slotId: number): SlotRow {
  const slot = db
    .prepare("SELECT id, name, target_min, target_max, position FROM slots WHERE id = ? AND deck_id = ?")
    .get(slotId, deckId) as SlotRow | undefined;
  if (!slot) throw new ServiceError(`Slot ${slotId} not found in deck ${deckId}`, 404);
  return slot;
}

export function findSlotByName(db: Database, actor: Actor, deckId: number, name: string): SlotRow | null {
  requireDeck(db, actor, deckId);
  return (
    (db
      .prepare(
        `SELECT id, name, target_min, target_max, position FROM slots
         WHERE deck_id = ? AND name = ? COLLATE NOCASE`,
      )
      .get(deckId, name.trim()) as SlotRow | undefined) ?? null
  );
}

/** A slot by name, or an error that lists the slots that do exist. */
export function requireSlotByName(db: Database, actor: Actor, deckId: number, name: string): SlotRow {
  requireDeck(db, actor, deckId);
  const slot = findSlotByName(db, actor, deckId, name);
  if (slot) return slot;
  const existing = listSlots(db, actor, deckId).map((s) => s.name);
  throw new ServiceError(
    `This deck has no slot named '${name}'. ` +
      (existing.length
        ? `Its slots are: ${existing.join(", ")}.`
        : `It defines no slots; create one first, or leave the card unslotted.`),
    404,
  );
}

function validateTargets(min: number | null, max: number | null) {
  for (const n of [min, max])
    if (n != null && (!Number.isInteger(n) || n < 0))
      throw new ServiceError("A slot target must be a whole number, zero or more");
  if (min != null && max != null && min > max)
    throw new ServiceError("Slot target min cannot exceed max");
}

export function createSlot(
  db: Database, actor: Actor,
  deckId: number,
  name: string,
  targetMin: number | null = null,
  targetMax: number | null = null,
): number {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  const valid = validateVocabName(name);
  validateTargets(targetMin, targetMax);
  const { p } = db
    .prepare("SELECT COALESCE(MAX(position), -1) + 1 p FROM slots WHERE deck_id = ?")
    .get(deckId) as { p: number };
  try {
    const r = db
      .prepare(
        "INSERT INTO slots (deck_id, name, target_min, target_max, position) VALUES (?, ?, ?, ?, ?)",
      )
      .run(deckId, valid, targetMin, targetMax, p);
    return Number(r.lastInsertRowid);
  } catch (e: any) {
    if (isUniqueViolation(e))
      throw new ServiceError(`Slot '${valid}' already exists in this deck`, 409);
    throw e;
  }
}

export function updateSlot(
  db: Database, actor: Actor,
  deckId: number,
  slotId: number,
  patch: { name?: string; targetMin?: number | null; targetMax?: number | null },
): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  const slot = requireSlot(db, actor, deckId, slotId);
  const name = patch.name !== undefined ? validateVocabName(patch.name) : slot.name;
  const min = patch.targetMin !== undefined ? patch.targetMin : slot.target_min;
  const max = patch.targetMax !== undefined ? patch.targetMax : slot.target_max;
  validateTargets(min, max);
  try {
    db.prepare("UPDATE slots SET name = ?, target_min = ?, target_max = ? WHERE id = ?").run(
      name,
      min,
      max,
      slotId,
    );
  } catch (e: any) {
    if (isUniqueViolation(e))
      throw new ServiceError(`Slot '${name}' already exists in this deck`, 409);
    throw e;
  }
}

export function deleteSlot(db: Database, actor: Actor, deckId: number, slotId: number): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  requireSlot(db, actor, deckId, slotId);
  db.prepare("DELETE FROM slots WHERE id = ?").run(slotId);
}

// ---------- tags ----------

export interface TagRow {
  id: number;
  name: string;
}

export function listTags(db: Database, actor: Actor, deckId: number): TagRow[] {
  requireDeck(db, actor, deckId);
  return db
    .prepare("SELECT id, name FROM tags WHERE deck_id = ? ORDER BY name")
    .all(deckId) as unknown as TagRow[];
}

function requireTag(db: Database, actor: Actor, deckId: number, tagId: number) {
  const tag = db.prepare("SELECT * FROM tags WHERE id = ? AND deck_id = ?").get(tagId, deckId);
  if (!tag) throw new ServiceError(`Tag ${tagId} not found in deck ${deckId}`, 404);
  return tag;
}

// Folds a tag name down to what makes two tags "the same idea spelled
// differently": case, punctuation, word order and plurals.
function tagKey(name: string): string {
  return name
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean)
    .map(singular)
    .sort()
    .join(" ");
}

// An existing tag the candidate would shadow, if any. Without this the
// vocabulary grows `ramp`, `Ramp` and `ramps` in a month and tag: filters
// quietly stop finding things (spec §5.4). Deliberately narrow: `mana ramp`
// beside `ramp` is a judgement call and is allowed.
export function similarTag(existing: string[], candidate: string): string | null {
  const key = tagKey(candidate);
  return existing.find((e) => tagKey(e) === key) ?? null;
}

export function createTag(db: Database, actor: Actor, deckId: number, name: string): number {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  const valid = validateVocabName(name);
  const clash = similarTag(
    listTags(db, actor, deckId).map((t) => t.name),
    valid,
  );
  if (clash)
    throw new ServiceError(
      `This deck already has the tag '${clash}', which '${valid}' would duplicate. Use '${clash}'.`,
      409,
    );
  const r = db.prepare("INSERT INTO tags (deck_id, name) VALUES (?, ?)").run(deckId, valid);
  return Number(r.lastInsertRowid);
}

// Have adds evidence of ownership only. Reapplying it must not erase a more
// precise current physical/available count or create extra physical copies.
function recordHaveEvidence(db: Database, actor: Actor, deckId: number, oracleId: string): void {
  const latest = db.prepare("SELECT status FROM ownership_evidence WHERE oracle_id = ? AND user_id = ? ORDER BY id DESC LIMIT 1").get(oracleId, actor.user_id) as { status: string } | undefined;
  if (latest?.status === "owned") return;
  db.prepare(`INSERT INTO ownership_evidence
    (oracle_id, status, basis, author, source_deck_id, source_deck_name, user_id, author_user_id, author_via, author_label)
    VALUES (?, 'owned', 'Explicit Have tag; physical quantity and availability unknown', ?, ?, ?, ?, ?, ?, ?)`)
    .run(oracleId, authorOf(actor), deckId, requireDeck(db, actor, deckId).name, actor.user_id, actor.user_id, actor.via, actor.label);
}

export function renameTag(db: Database, actor: Actor, deckId: number, tagId: number, name: string): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  requireTag(db, actor, deckId, tagId);
  const valid = validateVocabName(name);
  const clash = similarTag(listTags(db, actor, deckId).filter((tag) => tag.id !== tagId).map((tag) => tag.name), valid);
  if (clash)
    throw new ServiceError(
      `This deck already has the tag '${clash}', which '${valid}' would duplicate. Use '${clash}'.`,
      409,
    );
  try {
    withTransaction(db, () => {
      db.prepare("UPDATE tags SET name = ? WHERE id = ?").run(valid, tagId);
      if (valid.trim().toLowerCase() === "have") {
        for (const row of db.prepare("SELECT oracle_id FROM deck_card_tags WHERE deck_id = ? AND tag_id = ?").all(deckId, tagId) as unknown as { oracle_id: string }[])
          recordHaveEvidence(db, actor, deckId, row.oracle_id);
      }
    });
  } catch (e: any) {
    if (isUniqueViolation(e))
      throw new ServiceError(`Tag '${valid}' already exists in this deck`, 409);
    throw e;
  }
}

export function deleteTag(db: Database, actor: Actor, deckId: number, tagId: number): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  requireTag(db, actor, deckId, tagId);
  db.prepare("DELETE FROM tags WHERE id = ?").run(tagId);
}

function findOrCreateTag(db: Database, actor: Actor, deckId: number, name: string): number {
  const existing = listTags(db, actor, deckId);
  const exact = existing.find((t) => t.name.toLowerCase() === name.trim().toLowerCase());
  return exact ? exact.id : createTag(db, actor, deckId, name);
}

// ---------- organization of cards (spec §5.4) ----------

export interface OrganizePatch {
  /** Primary slot id, or null to clear it. Omit to leave it alone. */
  slotId?: number | null;
  /** Replace secondary purposes; [] clears them. Omit to keep them. */
  secondarySlotIds?: number[];
  /** Replace the card's tags with exactly these tag ids. */
  tagIds?: number[];
}

export function validateSlotAssignments(db: Database, actor: Actor, deckId: number, primary: number | null, secondary: number[]): void {
  requireDeck(db, actor, deckId);
  if (primary != null && (!Number.isSafeInteger(primary) || primary < 1))
    throw new ServiceError("Primary slot ID must be a positive integer or null");
  if (!Array.isArray(secondary) || secondary.some(id => !Number.isSafeInteger(id) || id < 1))
    throw new ServiceError("Secondary slot IDs must be an array of positive integers");
  if (primary != null) requireSlot(db, actor, deckId, primary);
  for (const id of secondary) requireSlot(db, actor, deckId, id);
  if (primary != null && secondary.includes(primary))
    throw new ServiceError("A primary slot cannot also be secondary");
}

/** File one card's purposes and management tags. Changes no membership. */
export function organizeCard(
  db: Database, actor: Actor,
  deckId: number,
  oracleId: string,
  patch: OrganizePatch,
): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  withTransaction(db, () => {
    const row = requireDeckCard(db, actor, deckId, oracleId);
    if (patch.tagIds !== undefined && (!Array.isArray(patch.tagIds) || patch.tagIds.some((id) => !Number.isSafeInteger(id) || id < 1)))
      throw new ServiceError("Tag IDs must be an array of positive integers");
    if (patch.slotId !== undefined || patch.secondarySlotIds !== undefined) {
      const primary = patch.slotId !== undefined ? patch.slotId : row.slot_id;
      // Promoting a secondary purpose removes its secondary assignment. An
      // explicit duplicate is an error, so ambiguous writes never land.
      const secondary = patch.secondarySlotIds !== undefined ? patch.secondarySlotIds : row.secondary_slot_ids.filter(id => id !== primary);
      validateSlotAssignments(db, actor, deckId, primary, secondary);
      db.prepare("DELETE FROM deck_card_secondary_slots WHERE deck_id = ? AND oracle_id = ?").run(deckId, oracleId);
      db.prepare("UPDATE deck_cards SET slot_id = ? WHERE deck_id = ? AND oracle_id = ?").run(
        primary,
        deckId,
        oracleId,
      );
      const ins = db.prepare("INSERT INTO deck_card_secondary_slots (deck_id, oracle_id, slot_id) VALUES (?, ?, ?)");
      for (const id of new Set(secondary)) ins.run(deckId, oracleId, id);
    }
    if (patch.tagIds !== undefined) {
      for (const tagId of patch.tagIds) requireTag(db, actor, deckId, tagId);
      const hadHave = db.prepare(`SELECT 1 FROM deck_card_tags ct JOIN tags t ON t.id = ct.tag_id
        WHERE ct.deck_id = ? AND ct.oracle_id = ? AND lower(trim(t.name)) = 'have'`).get(deckId, oracleId);
      const addsHave = patch.tagIds.some(id => listTags(db, actor, deckId).some(t => t.id === id && t.name.trim().toLowerCase() === "have"));
      if (addsHave && !hadHave) recordHaveEvidence(db, actor, deckId, oracleId);
      db.prepare("DELETE FROM deck_card_tags WHERE deck_id = ? AND oracle_id = ?").run(
        deckId,
        oracleId,
      );
      const ins = db.prepare(
        "INSERT INTO deck_card_tags (deck_id, oracle_id, tag_id) VALUES (?, ?, ?)",
      );
      for (const tagId of new Set(patch.tagIds)) ins.run(deckId, oracleId, tagId);
    }
  });
}

export interface FilingInput {
  oracle_id: string;
  /** Primary slot name; null clears it; undefined leaves it alone. */
  slot?: string | null;
  /** Replace secondary purposes by name; [] clears them. */
  secondary_slots?: string[];
  add_tags?: string[];
  remove_tags?: string[];
}

// Bulk filing by name, for agents: every entry lands or none does. Tags named
// here are created if the deck lacks them, subject to the near-duplicate rule.
export function fileCards(db: Database, actor: Actor, deckId: number, filings: FilingInput[], removeUnusedTags: string[] = []): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  if (!Array.isArray(removeUnusedTags) || removeUnusedTags.some(name => typeof name !== "string"))
    throw new ServiceError("Unused tags to remove must be an array of tag names");
  withTransaction(db, () => {
    for (const f of filings) {
      requireDeckCard(db, actor, deckId, f.oracle_id);
      const patch: OrganizePatch = {};
      if (f.slot !== undefined)
        patch.slotId = f.slot === null ? null : requireSlotByName(db, actor, deckId, f.slot).id;
      if (f.secondary_slots !== undefined) {
        if (!Array.isArray(f.secondary_slots) || f.secondary_slots.some(name => typeof name !== "string"))
          throw new ServiceError("Secondary slots must be an array of slot names");
        patch.secondarySlotIds = f.secondary_slots.map(name => requireSlotByName(db, actor, deckId, name).id);
      }
      if (f.add_tags?.length || f.remove_tags?.length) {
        const current = new Set(
          (
            db
              .prepare("SELECT tag_id FROM deck_card_tags WHERE deck_id = ? AND oracle_id = ?")
              .all(deckId, f.oracle_id) as unknown as { tag_id: number }[]
          ).map((t) => t.tag_id),
        );
        for (const name of f.add_tags ?? []) current.add(findOrCreateTag(db, actor, deckId, name));
        for (const name of f.remove_tags ?? []) {
          const tag = listTags(db, actor, deckId).find(
            (t) => t.name.toLowerCase() === name.trim().toLowerCase(),
          );
          if (tag) current.delete(tag.id);
        }
        patch.tagIds = [...current];
      }
      organizeCard(db, actor, deckId, f.oracle_id, patch);
    }
    // Explicit vocabulary cleanup, never inferred from an empty assignment.
    // Reserved status tags can stay even while no card carries them.
    for (const name of removeUnusedTags) {
      const tag = listTags(db, actor, deckId).find(t => t.name.toLowerCase() === name.trim().toLowerCase());
      if (!tag) continue;
      if (db.prepare("SELECT 1 FROM deck_card_tags WHERE deck_id = ? AND tag_id = ? LIMIT 1").get(deckId, tag.id))
        throw new ServiceError(`Tag '${tag.name}' is still assigned; remove its assignments before removing it from the vocabulary`);
      deleteTag(db, actor, deckId, tag.id);
    }
  });
}

export interface SlotSpec {
  name: string;
  /** Rename the slot to this. */
  rename_to?: string;
  target_min?: number | null;
  target_max?: number | null;
  /** Remove the slot; its cards become unslotted. */
  remove?: boolean;
}

// Create, rename, retarget and remove slots in one transactional call. A slot
// named here that does not exist is created; one that exists is updated.
export function defineSlots(db: Database, actor: Actor, deckId: number, specs: SlotSpec[]): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  withTransaction(db, () => {
    for (const s of specs) {
      const existing = findSlotByName(db, actor, deckId, s.name);
      if (s.remove) {
        if (!existing) requireSlotByName(db, actor, deckId, s.name);
        deleteSlot(db, actor, deckId, existing!.id);
        continue;
      }
      if (!existing) {
        if (s.rename_to !== undefined) requireSlotByName(db, actor, deckId, s.name);
        createSlot(db, actor, deckId, s.name, s.target_min ?? null, s.target_max ?? null);
        continue;
      }
      updateSlot(db, actor, deckId, existing.id, {
        name: s.rename_to,
        targetMin: s.target_min,
        targetMax: s.target_max,
      });
    }
  });
}

// ---------- the maybe list (spec §5.4) ----------

// Shortlisting a card that is not in the deck is organization: it sits
// outside the 100 and bumps no revision. Moving a card between the 100 and
// the maybe list is membership and goes through deck/membership.ts.
export function shortlistCard(
  db: Database, actor: Actor,
  deckId: number,
  oracleId: string,
  note = "",
): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  const card = requireCardById(db, oracleId);
  const existing = findDeckCard(db, actor, deckId, oracleId);
  if (existing && !existing.maybe)
    throw new ServiceError(`${card.name} is already in the deck, so it cannot be shortlisted`);
  if (existing) {
    db.prepare("UPDATE deck_cards SET maybe_note = ? WHERE deck_id = ? AND oracle_id = ?").run(
      note.trim() || existing.maybe_note,
      deckId,
      oracleId,
    );
    return;
  }
  db.prepare(
    "INSERT INTO deck_cards (deck_id, oracle_id, maybe, maybe_note) VALUES (?, ?, 1, ?)",
  ).run(deckId, oracleId, note.trim());
}

export function unshortlistCard(db: Database, actor: Actor, deckId: number, oracleId: string): void {
  requireDeckOwner(db, actor, deckId);
  requireEditableDeck(db, actor, deckId);
  const row = requireDeckCard(db, actor, deckId, oracleId);
  if (!row.maybe)
    throw new ServiceError(
      `${requireCardById(db, oracleId).name} is in the deck, not on the maybe list. ` +
        `Removing it from the deck is a membership change.`,
    );
  db.prepare("DELETE FROM deck_cards WHERE deck_id = ? AND oracle_id = ?").run(deckId, oracleId);
}

// ---------- the deck view ----------

export interface DeckCardView {
  oracle_id: string;
  name: string;
  mana_cost: string | null;
  cmc: number;
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  color_identity: string;
  ci_mask: number;
  commander_legality: string;
  is_commander: number;
  /** Spec §4.4: the cheapest printing's price; null when no printing has one. */
  usd_cents: number | null;
  image_small: string | null;
  image_normal: string | null;
  image_art_crop: string | null;
  retired: number;
  slot_id: number | null;
  secondary_slot_ids: number[];
  role: Role;
  /** 1 = on the maybe list: considered for the deck, not part of the 100. */
  maybe: number;
  maybe_note: string;
  quantity: number;
  tag_ids: number[];
}

export function getDeck(db: Database, actor: Actor, deckId: number) {
  const deck = requireDeck(db, actor, deckId);
  const slots = listSlots(db, actor, deckId);
  const tags = listTags(db, actor, deckId);

  const cards = db
    .prepare(
      `SELECT c.oracle_id, c.name, c.mana_cost, c.cmc, c.type_line, c.oracle_text,
              c.power, c.toughness, c.loyalty, c.color_identity, c.ci_mask,
              c.commander_legality, c.is_commander, p.usd_cents, c.image_small, c.image_normal, c.image_art_crop, c.retired,
              dc.slot_id, dc.role, dc.maybe, dc.maybe_note, dc.quantity
       FROM deck_cards dc JOIN cards c ON c.oracle_id = dc.oracle_id
       LEFT JOIN card_prices p ON p.oracle_id = dc.oracle_id
       WHERE dc.deck_id = ? ORDER BY c.name`,
    )
    .all(deckId) as unknown as DeckCardView[];

  const tagRows = db
    .prepare("SELECT oracle_id, tag_id FROM deck_card_tags WHERE deck_id = ?")
    .all(deckId) as unknown as Array<{ oracle_id: string; tag_id: number }>;
  const tagsByCard = new Map<string, number[]>();
  for (const r of tagRows) {
    const list = tagsByCard.get(r.oracle_id) ?? [];
    list.push(r.tag_id);
    tagsByCard.set(r.oracle_id, list);
  }
  for (const card of cards) card.tag_ids = tagsByCard.get(card.oracle_id) ?? [];
  const secondaryRows = db.prepare("SELECT oracle_id, slot_id FROM deck_card_secondary_slots WHERE deck_id = ? ORDER BY slot_id")
    .all(deckId) as unknown as { oracle_id: string; slot_id: number }[];
  const secondaryByCard = new Map<string, number[]>();
  for (const r of secondaryRows) {
    const ids = secondaryByCard.get(r.oracle_id) ?? [];
    ids.push(r.slot_id);
    secondaryByCard.set(r.oracle_id, ids);
  }
  for (const card of cards) card.secondary_slot_ids = secondaryByCard.get(card.oracle_id) ?? [];

  const pendingItems = db
    .prepare(
      `SELECT pi.action, pi.slot_id, dc.slot_id AS current_slot_id,
              dc.maybe AS current_maybe, dc.role AS current_role
       FROM proposal_items pi
       JOIN proposals p ON p.id = pi.proposal_id
       LEFT JOIN deck_cards dc ON dc.deck_id = p.deck_id AND dc.oracle_id = pi.oracle_id
       WHERE p.deck_id = ? AND p.status = 'open' AND pi.status = 'pending'`,
    )
    .all(deckId) as unknown as PendingItem[];

  const parentRow = deck.parent_id
    ? (db.prepare("SELECT id, name FROM decks WHERE id = ? AND user_id = ?").get(deck.parent_id, actor.user_id) as
        | { id: number; name: string }
        | undefined)
    : undefined;
  const parent = parentRow ? { id: parentRow.id, name: parentRow.name } : null;

  return {
    deck: {
      id: deck.id,
      name: deck.name,
      category: deck.category,
      parent,
      ci_mask: deck.ci_mask,
      color_identity: deck.color_identity,
      revision: deck.revision,
    },
    slots,
    tags,
    cards,
    computed: computeState(deck, slots, cards, pendingItems, pricesAsOf(db)),
  };
}

export type DeckView = ReturnType<typeof getDeck>;

interface PendingItem {
  action: "add" | "cut";
  slot_id: number | null;
  current_slot_id: number | null;
  current_maybe: number | null;
  current_role: Role | null;
}

export type SlotStatus = "ok" | "under" | "over" | "untargeted";

const isLand = (c: { type_line: string }) => /\bLand\b/.test(c.type_line);

// Computed state is handed over, never derived by whoever reads it (spec §1,
// rule 2). If a reader would need to count something, the count belongs here.
function computeState(
  deck: { ci_mask: number | null },
  slots: SlotRow[],
  cards: DeckCardView[],
  pendingItems: PendingItem[],
  pricesAsOf: string | null,
) {
  // Everything below counts the deck. The maybe list is explicitly not part
  // of it: parked cards contribute to no count, curve, pip spread, slot delta
  // or violation, which is the whole point of parking them.
  const maybeCards = cards.filter((c) => c.maybe);
  // Every role that is actually in the deck — used for the legality checks,
  // which apply to the companion too but must not fire on a parked card.
  const deckCards = cards.filter((c) => !c.maybe);
  const mainCards = deckCards.filter((c) => c.role !== "companion");
  const cardCount = mainCards.reduce((n, c) => n + c.quantity, 0);

  const slotDeltas = slots.map((s) => {
    const primaryCount = mainCards
      .filter((c) => c.slot_id === s.id)
      .reduce((n, c) => n + c.quantity, 0);
    const secondaryCount = mainCards
      .filter(c => c.slot_id !== s.id && c.secondary_slot_ids.includes(s.id))
      .reduce((n, c) => n + c.quantity, 0);
    const count = primaryCount;
    let status: SlotStatus = "untargeted";
    let delta = 0;
    if (s.target_min != null || s.target_max != null) {
      if (s.target_min != null && count < s.target_min) {
        status = "under";
        delta = count - s.target_min;
      } else if (s.target_max != null && count > s.target_max) {
        status = "over";
        delta = count - s.target_max;
      } else {
        status = "ok";
      }
    }
    return {
      slot_id: s.id,
      name: s.name,
      count,
      primary_count: primaryCount,
      secondary_count: secondaryCount,
      coverage_count: primaryCount + secondaryCount,
      target_min: s.target_min,
      target_max: s.target_max,
      status,
      delta,
    };
  });

  const unslotted = mainCards
    .filter((c) => c.slot_id == null && c.role === "card")
    .reduce((n, c) => n + c.quantity, 0);

  // Identity violations only exist relative to a chosen commander.
  const identityViolations =
    deck.ci_mask == null
      ? []
      : deckCards
          .filter((c) => (c.ci_mask & ~deck.ci_mask!) !== 0)
          .map((c) => ({ oracle_id: c.oracle_id, name: c.name, color_identity: c.color_identity }));

  const singletonViolations = mainCards
    .filter((c) => c.quantity > 1)
    .map((c) => ({ card: c, limit: copyLimit(c) }))
    .filter(({ card, limit }) => card.quantity > limit)
    .map(({ card, limit }) => ({
      oracle_id: card.oracle_id,
      name: card.name,
      quantity: card.quantity,
      limit: Number.isFinite(limit) ? limit : null,
    }));

  const notLegal = deckCards
    .filter((c) => c.commander_legality !== "legal")
    .map((c) => ({ oracle_id: c.oracle_id, name: c.name, legality: c.commander_legality }));

  const landCount = mainCards.filter(isLand).reduce((n, c) => n + c.quantity, 0);
  // Same land classification as land_count; commanders count, the companion
  // and parked cards do not. Copies are weighted, not distinct rows.
  const nonlandCount = cardCount - landCount;
  const averageManaValue = nonlandCount
    ? mainCards.filter((c) => !isLand(c)).reduce((n, c) => n + c.cmc * c.quantity, 0) /
      nonlandCount
    : null;

  const curve: Record<string, number> = { "0": 0, "1": 0, "2": 0, "3": 0, "4": 0, "5": 0, "6": 0, "7+": 0 };
  for (const c of mainCards) {
    if (isLand(c) && !c.mana_cost) continue;
    const bucket = c.cmc >= 7 ? "7+" : String(Math.floor(c.cmc));
    curve[bucket] += c.quantity;
  }

  const pips: Record<string, number> = { W: 0, U: 0, B: 0, R: 0, G: 0 };
  for (const c of mainCards)
    for (const letter of (c.mana_cost ?? "").matchAll(/[WUBRG]/g)) pips[letter[0]] += c.quantity;

  // Spec §4.4: what the deck would cost to buy, every card at its cheapest
  // printing. The companion is a card to buy too, so it counts here though it
  // sits outside the 100.
  const cost = { ...estimateCost(deckCards), as_of: pricesAsOf };

  const pendingAdds = pendingItems.filter((i) => i.action === "add").length;
  const pendingCuts = pendingItems.filter((i) => i.action === "cut").length;
  const pendingBySlot: Record<number, number> = {};
  for (const i of pendingItems) {
    // Targets count primary assignments only. A promoted maybe card retains
    // its primary purpose if no surviving replacement slot is named.
    if (i.current_role === "companion") continue;
    const proposed = slots.some(s => s.id === i.slot_id) ? i.slot_id : null;
    const primary = i.action === "add"
      ? (i.current_maybe === 0 ? i.current_slot_id : proposed ?? (i.current_maybe ? i.current_slot_id : null))
      : i.current_slot_id;
    if (primary != null && slots.some(s => s.id === primary))
      pendingBySlot[primary] = (pendingBySlot[primary] ?? 0) + (i.action === "add" ? 1 : -1);
  }

  return {
    card_count: cardCount,
    delta_to_100: cardCount - 100,
    pending: {
      adds: pendingAdds,
      cuts: pendingCuts,
      projected_count: cardCount + pendingAdds - pendingCuts,
      by_slot: pendingBySlot,
    },
    unslotted_count: unslotted,
    maybe_count: maybeCards.length,
    slot_deltas: slotDeltas,
    identity_violations: identityViolations,
    singleton_violations: singletonViolations,
    legality_violations: notLegal,
    land_count: landCount,
    average_mana_value: averageManaValue,
    curve,
    pips,
    cost,
  };
}

export type ComputedState = ReturnType<typeof computeState>;
