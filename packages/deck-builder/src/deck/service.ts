import type { DatabaseSync } from "node:sqlite";

export class ServiceError extends Error {
  status: number;
  constructor(message: string, status = 400) {
    super(message);
    this.status = status;
  }
}

// Card types and supertypes. Slots and tags say what a card *does* — "ramp",
// "interaction" — and card type says what it *is*; the decklist groups by
// either one, so they have to stay separate axes. A slot named "lands" makes
// "34 lands" ambiguous about which of the two it counts, so both numbers are
// rejected: "land" and "lands", "sorcery" and "sorceries".
const CARD_TYPE_NAMES = new Set([
  "land", "creature", "artifact", "enchantment", "instant", "sorcery",
  "planeswalker", "battle", "kindred", "tribal", "legendary", "basic", "snow", "token",
]);

// Reserved for a different reason: slot:/tag: queries would stop parsing
// (spec §4). Only the exact word collides, so these aren't pluralised.
const RESERVED_NAMES = new Set([
  // search prefixes
  "t", "type", "o", "oracle", "fo", "fulloracle", "cmc", "mv", "manavalue",
  "c", "color", "id", "identity", "pow", "power", "tou", "toughness",
  "loy", "loyalty", "is", "f", "format", "banned", "slot", "tag",
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
export function copyLimit(card: { type_line: string; search_text?: string; oracle_text?: string }): number {
  if (/\bBasic\b/.test(card.type_line)) return Infinity;
  const text = (card.search_text ?? card.oracle_text ?? "").toLowerCase();
  if (text.includes("any number of cards named")) return Infinity;
  const m = text.match(/up to (\w+) cards named/);
  if (m) return WORD_NUMBERS[m[1]] ?? 1;
  return 1;
}

// ---------- deck CRUD ----------

export function createDeck(db: DatabaseSync, name: string): number {
  const trimmed = name.trim();
  if (!trimmed) throw new ServiceError("Deck name cannot be empty");
  try {
    const r = db.prepare("INSERT INTO decks (name) VALUES (?)").run(trimmed);
    return Number(r.lastInsertRowid);
  } catch (e: any) {
    if (String(e.message).includes("UNIQUE"))
      throw new ServiceError(`A deck named '${trimmed}' already exists`, 409);
    throw e;
  }
}

export function renameDeck(db: DatabaseSync, deckId: number, name: string): void {
  requireDeck(db, deckId);
  const trimmed = name.trim();
  if (!trimmed) throw new ServiceError("Deck name cannot be empty");
  db.prepare("UPDATE decks SET name = ? WHERE id = ?").run(trimmed, deckId);
}

export function deleteDeck(db: DatabaseSync, deckId: number): void {
  requireDeck(db, deckId);
  db.prepare("DELETE FROM decks WHERE id = ?").run(deckId);
}

export function listDecks(db: DatabaseSync) {
  return db
    .prepare(
      `SELECT d.id, d.name, d.color_identity, d.created_at,
              COALESCE((SELECT SUM(quantity) FROM deck_cards dc WHERE dc.deck_id = d.id AND dc.role != 'companion' AND dc.maybeboard = 0), 0) AS card_count
       FROM decks d ORDER BY d.name`,
    )
    .all() as unknown as Array<{
    id: number;
    name: string;
    color_identity: string;
    created_at: string;
    card_count: number;
  }>;
}

function requireDeck(db: DatabaseSync, deckId: number) {
  const deck = db.prepare("SELECT * FROM decks WHERE id = ?").get(deckId);
  if (!deck) throw new ServiceError(`Deck ${deckId} not found`, 404);
  return deck as {
    id: number;
    name: string;
    ci_mask: number | null;
    color_identity: string;
    revision: number;
  };
}

// Card-list changes (add/remove/quantity/role) bump the deck revision;
// organizational edits (slot, tags, owned) do not.
function bumpDeckRevision(db: DatabaseSync, deckId: number) {
  db.prepare("UPDATE decks SET revision = revision + 1 WHERE id = ?").run(deckId);
}

function requireDeckCard(db: DatabaseSync, deckId: number, oracleId: string) {
  const row = db
    .prepare("SELECT * FROM deck_cards WHERE deck_id = ? AND oracle_id = ?")
    .get(deckId, oracleId);
  if (!row) throw new ServiceError(`Card ${oracleId} is not in deck ${deckId}`, 404);
  return row as { role: string; quantity: number };
}

// ---------- cards ----------

export function addCard(
  db: DatabaseSync,
  deckId: number,
  oracleId: string,
  opts: { slotId?: number | null; role?: "card" | "commander" | "companion" } = {},
): void {
  requireDeck(db, deckId);
  const card = db.prepare("SELECT oracle_id FROM cards WHERE oracle_id = ?").get(oracleId);
  if (!card) throw new ServiceError(`Unknown oracle_id ${oracleId}`, 404);

  const existing = db
    .prepare("SELECT quantity FROM deck_cards WHERE deck_id = ? AND oracle_id = ?")
    .get(deckId, oracleId) as { quantity: number } | undefined;
  if (existing) {
    // Duplicate add bumps quantity (needed for basics); legality of the
    // count is the computed state's job, not a gate here.
    db.prepare(
      "UPDATE deck_cards SET quantity = quantity + 1 WHERE deck_id = ? AND oracle_id = ?",
    ).run(deckId, oracleId);
    bumpDeckRevision(db, deckId);
    return;
  }

  if (opts.slotId != null) requireSlot(db, deckId, opts.slotId);
  const role = opts.role ?? "card";
  if (role !== "card") assertRoleCapacity(db, deckId, role);
  db.prepare(
    "INSERT INTO deck_cards (deck_id, oracle_id, slot_id, role) VALUES (?, ?, ?, ?)",
  ).run(deckId, oracleId, opts.slotId ?? null, role);
  if (role === "commander") recomputeIdentity(db, deckId);
  bumpDeckRevision(db, deckId);
}

export function removeCard(db: DatabaseSync, deckId: number, oracleId: string): void {
  const row = requireDeckCard(db, deckId, oracleId);
  db.prepare("DELETE FROM deck_cards WHERE deck_id = ? AND oracle_id = ?").run(deckId, oracleId);
  if (row.role === "commander") recomputeIdentity(db, deckId);
  bumpDeckRevision(db, deckId);
}

export function updateCard(
  db: DatabaseSync,
  deckId: number,
  oracleId: string,
  patch: {
    slotId?: number | null;
    role?: "card" | "commander" | "companion";
    owned?: boolean;
    quantity?: number;
    tagIds?: number[];
  },
): void {
  const row = requireDeckCard(db, deckId, oracleId);

  if (patch.slotId !== undefined) {
    if (patch.slotId != null) requireSlot(db, deckId, patch.slotId);
    db.prepare("UPDATE deck_cards SET slot_id = ? WHERE deck_id = ? AND oracle_id = ?").run(
      patch.slotId,
      deckId,
      oracleId,
    );
  }
  if (patch.role !== undefined && patch.role !== row.role) {
    if (patch.role !== "card") assertRoleCapacity(db, deckId, patch.role);
    db.prepare("UPDATE deck_cards SET role = ? WHERE deck_id = ? AND oracle_id = ?").run(
      patch.role,
      deckId,
      oracleId,
    );
    recomputeIdentity(db, deckId);
    bumpDeckRevision(db, deckId);
  }
  if (patch.owned !== undefined) {
    db.prepare("UPDATE deck_cards SET owned = ? WHERE deck_id = ? AND oracle_id = ?").run(
      patch.owned ? 1 : 0,
      deckId,
      oracleId,
    );
  }
  if (patch.quantity !== undefined) {
    if (patch.quantity < 1) throw new ServiceError("Quantity must be at least 1");
    if (patch.quantity !== row.quantity) {
      db.prepare("UPDATE deck_cards SET quantity = ? WHERE deck_id = ? AND oracle_id = ?").run(
        patch.quantity,
        deckId,
        oracleId,
      );
      bumpDeckRevision(db, deckId);
    }
  }
  if (patch.tagIds !== undefined) {
    for (const tagId of patch.tagIds) requireTag(db, deckId, tagId);
    db.prepare("DELETE FROM deck_card_tags WHERE deck_id = ? AND oracle_id = ?").run(
      deckId,
      oracleId,
    );
    const ins = db.prepare(
      "INSERT INTO deck_card_tags (deck_id, oracle_id, tag_id) VALUES (?, ?, ?)",
    );
    for (const tagId of new Set(patch.tagIds)) ins.run(deckId, oracleId, tagId);
  }
}

function assertRoleCapacity(db: DatabaseSync, deckId: number, role: "commander" | "companion") {
  const { n } = db
    .prepare("SELECT COUNT(*) n FROM deck_cards WHERE deck_id = ? AND role = ?")
    .get(deckId, role) as { n: number };
  const cap = role === "commander" ? 2 : 1;
  if (n >= cap)
    throw new ServiceError(
      role === "commander"
        ? "A deck supports at most two command-zone cards (commander + partner or background)"
        : "A deck supports at most one companion",
    );
}

// Deck identity = union of commander identities (spec §3, denormalized).
function recomputeIdentity(db: DatabaseSync, deckId: number): void {
  const commanders = db
    .prepare(
      `SELECT c.ci_mask, c.color_identity FROM deck_cards dc JOIN cards c ON c.oracle_id = dc.oracle_id
       WHERE dc.deck_id = ? AND dc.role = 'commander'`,
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

// ---------- slots ----------

function requireSlot(db: DatabaseSync, deckId: number, slotId: number) {
  const slot = db.prepare("SELECT * FROM slots WHERE id = ? AND deck_id = ?").get(slotId, deckId);
  if (!slot) throw new ServiceError(`Slot ${slotId} not found in deck ${deckId}`, 404);
  return slot;
}

function validateTargets(min: number | null, max: number | null) {
  if (min != null && min < 0) throw new ServiceError("Slot target cannot be negative");
  if (max != null && max < 0) throw new ServiceError("Slot target cannot be negative");
  if (min != null && max != null && min > max)
    throw new ServiceError("Slot target min cannot exceed max");
}

export function createSlot(
  db: DatabaseSync,
  deckId: number,
  name: string,
  targetMin: number | null = null,
  targetMax: number | null = null,
): number {
  requireDeck(db, deckId);
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
    if (String(e.message).includes("UNIQUE"))
      throw new ServiceError(`Slot '${valid}' already exists in this deck`, 409);
    throw e;
  }
}

export function updateSlot(
  db: DatabaseSync,
  deckId: number,
  slotId: number,
  patch: { name?: string; targetMin?: number | null; targetMax?: number | null },
): void {
  const slot = requireSlot(db, deckId, slotId) as {
    name: string;
    target_min: number | null;
    target_max: number | null;
  };
  const name = patch.name !== undefined ? validateVocabName(patch.name) : slot.name;
  const min = patch.targetMin !== undefined ? patch.targetMin : slot.target_min;
  const max = patch.targetMax !== undefined ? patch.targetMax : slot.target_max;
  validateTargets(min, max);
  db.prepare("UPDATE slots SET name = ?, target_min = ?, target_max = ? WHERE id = ?").run(
    name,
    min,
    max,
    slotId,
  );
}

export function deleteSlot(db: DatabaseSync, deckId: number, slotId: number): void {
  requireSlot(db, deckId, slotId);
  db.prepare("DELETE FROM slots WHERE id = ?").run(slotId);
}

// ---------- tags ----------

function requireTag(db: DatabaseSync, deckId: number, tagId: number) {
  const tag = db.prepare("SELECT * FROM tags WHERE id = ? AND deck_id = ?").get(tagId, deckId);
  if (!tag) throw new ServiceError(`Tag ${tagId} not found in deck ${deckId}`, 404);
  return tag;
}

export function createTag(db: DatabaseSync, deckId: number, name: string): number {
  requireDeck(db, deckId);
  const valid = validateVocabName(name);
  try {
    const r = db.prepare("INSERT INTO tags (deck_id, name) VALUES (?, ?)").run(deckId, valid);
    return Number(r.lastInsertRowid);
  } catch (e: any) {
    if (String(e.message).includes("UNIQUE"))
      throw new ServiceError(`Tag '${valid}' already exists in this deck`, 409);
    throw e;
  }
}

export function renameTag(db: DatabaseSync, deckId: number, tagId: number, name: string): void {
  requireTag(db, deckId, tagId);
  db.prepare("UPDATE tags SET name = ? WHERE id = ?").run(validateVocabName(name), tagId);
}

export function deleteTag(db: DatabaseSync, deckId: number, tagId: number): void {
  requireTag(db, deckId, tagId);
  db.prepare("DELETE FROM tags WHERE id = ?").run(tagId);
}

// ---------- full deck state ----------

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
  slot_id: number | null;
  role: "card" | "commander" | "companion";
  /** 1 = on the maybe list: considered for the deck, not part of the 100. */
  maybeboard: number;
  owned: number;
  quantity: number;
  tag_ids: number[];
}

export function getDeck(db: DatabaseSync, deckId: number) {
  const deck = requireDeck(db, deckId);
  const slots = db
    .prepare("SELECT id, name, target_min, target_max, position FROM slots WHERE deck_id = ? ORDER BY position")
    .all(deckId) as unknown as Array<{
    id: number;
    name: string;
    target_min: number | null;
    target_max: number | null;
    position: number;
  }>;
  const tags = db
    .prepare("SELECT id, name FROM tags WHERE deck_id = ? ORDER BY name")
    .all(deckId) as unknown as Array<{ id: number; name: string }>;

  const cards = db
    .prepare(
      `SELECT c.oracle_id, c.name, c.mana_cost, c.cmc, c.type_line, c.oracle_text,
              c.power, c.toughness, c.loyalty, c.color_identity, c.ci_mask,
              c.commander_legality, c.is_commander,
              dc.slot_id, dc.role, dc.maybeboard, dc.owned, dc.quantity
       FROM deck_cards dc JOIN cards c ON c.oracle_id = dc.oracle_id
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

  const pendingItems = db
    .prepare(
      `SELECT pi.action, pi.slot_id, dc.slot_id AS current_slot_id
       FROM proposal_items pi
       JOIN proposals p ON p.id = pi.proposal_id
       LEFT JOIN deck_cards dc ON dc.deck_id = p.deck_id AND dc.oracle_id = pi.oracle_id
       WHERE p.deck_id = ? AND pi.status = 'pending'`,
    )
    .all(deckId) as unknown as Array<{
    action: "add" | "cut";
    slot_id: number | null;
    current_slot_id: number | null;
  }>;

  return {
    deck: {
      id: deck.id,
      name: deck.name,
      ci_mask: deck.ci_mask,
      color_identity: deck.color_identity,
      revision: deck.revision,
    },
    slots,
    tags,
    cards,
    computed: computeState(deck, slots, cards, pendingItems),
  };
}

export type SlotStatus = "ok" | "under" | "over" | "untargeted";

// Computed state is given, never derived by the model (spec §10).
function computeState(
  deck: { ci_mask: number | null },
  slots: Array<{ id: number; name: string; target_min: number | null; target_max: number | null }>,
  cards: DeckCardView[],
  pendingItems: Array<{
    action: "add" | "cut";
    slot_id: number | null;
    current_slot_id: number | null;
  }> = [],
) {
  // Everything below counts the deck. The maybe list (spec §4.1) is explicitly
  // not part of it: parked cards contribute to no count, curve, pip spread,
  // slot delta or violation, which is the whole point of parking them.
  const maybeCards = cards.filter((c) => c.maybeboard);
  // Every role that is actually in the deck — used for the legality checks,
  // which apply to the companion too but must not fire on a parked card.
  const deckCards = cards.filter((c) => !c.maybeboard);
  const mainCards = cards.filter((c) => c.role !== "companion" && !c.maybeboard);
  const cardCount = mainCards.reduce((n, c) => n + c.quantity, 0);

  const slotDeltas = slots.map((s) => {
    const count = mainCards
      .filter((c) => c.slot_id === s.id)
      .reduce((n, c) => n + c.quantity, 0);
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
      target_min: s.target_min,
      target_max: s.target_max,
      status,
      delta,
    };
  });

  const unslotted = mainCards
    .filter((c) => c.slot_id == null)
    .reduce((n, c) => n + c.quantity, 0);

  // Identity violations only exist relative to a chosen commander.
  const identityViolations =
    deck.ci_mask == null
      ? []
      : cards
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

  const notLegal = cards
    .filter((c) => c.commander_legality !== "legal")
    .map((c) => ({ oracle_id: c.oracle_id, name: c.name, legality: c.commander_legality }));

  const lands = mainCards.filter((c) => /\bLand\b/.test(c.type_line));
  const landCount = lands.reduce((n, c) => n + c.quantity, 0);

  const curve: Record<string, number> = { "0": 0, "1": 0, "2": 0, "3": 0, "4": 0, "5": 0, "6": 0, "7+": 0 };
  for (const c of mainCards) {
    if (/\bLand\b/.test(c.type_line) && !c.mana_cost) continue;
    const bucket = c.cmc >= 7 ? "7+" : String(Math.floor(c.cmc));
    curve[bucket] += c.quantity;
  }

  const pips: Record<string, number> = { W: 0, U: 0, B: 0, R: 0, G: 0 };
  for (const c of mainCards) {
    for (const letter of (c.mana_cost ?? "").matchAll(/[WUBRG]/g)) {
      pips[letter[0]] += c.quantity;
    }
  }

  const pendingAdds = pendingItems.filter((i) => i.action === "add").length;
  const pendingCuts = pendingItems.filter((i) => i.action === "cut").length;
  const pendingBySlot: Record<number, number> = {};
  for (const i of pendingItems) {
    const slot = i.action === "add" ? i.slot_id : i.current_slot_id;
    if (slot != null) pendingBySlot[slot] = (pendingBySlot[slot] ?? 0) + (i.action === "add" ? 1 : -1);
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
    maybe_count: maybeCards.reduce((n, c) => n + c.quantity, 0),
    slot_deltas: slotDeltas,
    identity_violations: identityViolations,
    singleton_violations: singletonViolations,
    legality_violations: notLegal,
    land_count: landCount,
    curve,
    pips,
  };
}
