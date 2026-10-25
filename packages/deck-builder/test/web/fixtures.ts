// Minimal valid wire data for the domain tests. Typed against the real wire
// types, so a server change that adds or renames a field fails this file's
// typecheck instead of silently leaving the tests describing a shape the
// server no longer sends. Names are synthetic: no real card appears here.

import type {
  ActivityEntry,
  CardData,
  DeckCard,
  DeckState,
  DocumentEdit,
  LogEntry,
  Memory,
  Proposal,
  ProposalItem,
  Slot,
} from "../../web/src/api/client.ts";

let next = 1;
const id = () => next++;

export function card(over: Partial<DeckCard> & { name?: string } = {}): DeckCard {
  const name = over.name ?? "Test Card A";
  return {
    oracle_id: `oid-${name}`,
    name,
    mana_cost: null,
    cmc: 0,
    type_line: "Creature — Test",
    oracle_text: "",
    power: null,
    toughness: null,
    loyalty: null,
    color_identity: "",
    ci_mask: 0,
    commander_legality: "legal",
    is_commander: 0,
    cents: null, currency: "USD", store: "tcgplayer", as_of: null, source: "Scryfall",
    image_small: null, image_normal: null, image_art_crop: null, artist: null, retired: 0,
    slot_id: null,
    secondary_slot_ids: [],
    role: "card",
    maybe: 0,
    maybe_note: "",
    quantity: 1,
    tag_ids: [],
    ...over,
  };
}

export function searchResult(over: Partial<CardData> & { name?: string } = {}): CardData {
  const name = over.name ?? "Test Card A";
  return {
    oracle_id: `oid-${name}`,
    name,
    mana_cost: null,
    cmc: 0,
    type_line: "Creature — Test",
    oracle_text: "",
    power: null,
    toughness: null,
    loyalty: null,
    color_identity: "",
    commander_legality: "legal",
    is_commander: 0,
    cents: null, currency: "USD", store: "tcgplayer", as_of: null, source: "Scryfall",
    image_small: null, image_normal: null, image_art_crop: null, retired: 0,
    ...over,
  };
}

export function slot(over: Partial<Slot> = {}): Slot {
  const slotId = over.id ?? id();
  return { id: slotId, name: `Test Slot ${slotId}`, target_min: null, target_max: null, position: 0, ...over };
}

export function item(over: Partial<ProposalItem> & { name?: string } = {}): ProposalItem {
  const name = over.name ?? "Test Card A";
  const { name: _name, ...rest } = over;
  return {
    id: id(),
    proposal_id: 1,
    action: "add",
    oracle_id: `oid-${name}`,
    card_name: name,
    mana_cost: null,
    type_line: "Creature — Test",
    oracle_text: "",
    power: null,
    toughness: null,
    loyalty: null,
    cents: null, currency: "USD", store: "tcgplayer", as_of: null, source: "Scryfall",
    image_small: null, image_normal: null, image_art_crop: null,
    slot_id: null,
    slot_name: null,
    secondary_slot_ids: [],
    secondary_slot_names: [],
    to_maybe: 0,
    reason: "Test reason",
    group_id: null,
    status: "pending",
    ...rest,
  };
}

export function proposal(items: ProposalItem[], over: Partial<Proposal> = {}): Proposal {
  const proposalId = over.id ?? id();
  return {
    id: proposalId,
    deck_id: 1,
    deck_name: "Test Deck",
    summary: "",
    author_token_id: null, author_oauth_client_id: null,
    author_user_id: 1, author_via: "agent", author_label: "test-agent",
    author: "test-agent",
    status: "open",
    created_at: "2026-01-01 12:00:00",
    base_revision: 0, current_revision: 0, stale: false, disposition: "current", superseded_by: null, supersedes: [], is_current: true,
    items: items.map((i) => ({ ...i, proposal_id: proposalId })),
    ...over,
  };
}

export function logEntry(over: Partial<LogEntry> = {}): LogEntry {
  return {
    read_only: false,
    hidden: 0,
    id: id(),
    deck_id: 1,
    deck_name: "Test Deck",
    revision: 1,
    kind: "accept",
    summary: "",
    ruling_reason: "",
    proposal_id: null,
    document_edit_id: null,
    author_user_id: 1, author_via: "agent", author_label: "test-agent",
    author: "owner",
    undo_of: null,
    undone_by: null,
    reviewed_at: null,
    ts: "2026-01-01 12:00:00",
    cards: [],
    ...over,
  };
}

export function memory(over: Partial<Omit<Memory, "cards">> & { cards?: string[] } = {}): Memory {
  const { cards = [], ...rest } = over;
  return {
    read_only: false,
    id: id(),
    text: "Test memory",
    user_id: 1,
    scope: "global",
    deck_id: null,
    deck_name: null,
    basis: "stated",
    author_user_id: 1, author_via: "agent", author_label: "test-agent",
    author: "owner",
    owner_touched: false,
    source_log_id: null,
    deck_revision: null,
    status: "active",
    superseded_by: null,
    created_at: "2026-01-01 12:00:00",
    revised_at: "2026-01-01 12:00:00",
    cards: cards.map((oracle_id) => ({ oracle_id, name: oracle_id })),
    ...rest,
  };
}

export function documentEdit(over: Partial<DocumentEdit> = {}): DocumentEdit {
  return {
    read_only: false,
    id: id(),
    deck_id: 1,
    deck_name: "Test Deck",
    kind: "thesis",
    payload: {},
    reason: "Test reason",
    author_token_id: null, author_oauth_client_id: null,
    author_user_id: 1, author_via: "agent", author_label: "test-agent",
    author: "test-agent",
    status: "pending",
    created_at: "2026-01-01 12:00:00",
    stale: false,
    current: "",
    piece_names: [],
    base_revision: 0, current_revision: 0, disposition: "current", superseded_by: null, supersedes: [], group_id: null, group_edit_ids: [], group_edits: [], current_engine: null, is_current: true,
    ...over,
  };
}

export function activity(over: Partial<ActivityEntry> = {}): ActivityEntry {
  return {
    id: id(),
    ts: "2026-01-01 12:00:00",
    actor: "test-agent", user_id: 1, actor_user_id: 1, actor_via: "agent", actor_label: "test-agent",
    action: "note",
    deck_id: 1,
    deck_name: "Test Deck",
    summary: "",
    ...over,
  };
}

/**
 * A server-shaped `computed` for the given cards, slots and pending items.
 * It mirrors the parts of the server's computeState that the domain layer
 * reads; tests that must prove the client does not recount override fields on
 * the result.
 */
function computedFor(
  slots: Slot[],
  cards: DeckCard[],
  proposals: Proposal[],
): DeckState["computed"] {
  const main = cards.filter((c) => !c.maybe && c.role !== "companion");
  const cardCount = main.reduce((n, c) => n + c.quantity, 0);
  const pending = proposals.flatMap((p) => p.items).filter((i) => i.status === "pending");
  const adds = pending.filter((i) => i.action === "add").length;
  const cuts = pending.length - adds;
  const bySlot: Record<number, number> = {};
  for (const i of pending) {
    const slotId = i.action === "add" ? i.slot_id : (cards.find((c) => c.oracle_id === i.oracle_id)?.slot_id ?? null);
    if (slotId != null) bySlot[slotId] = (bySlot[slotId] ?? 0) + (i.action === "add" ? 1 : -1);
  }
  return {
    card_count: cardCount,
    delta_to_100: cardCount - 100,
    pending: { adds, cuts, projected_count: cardCount + adds - cuts, by_slot: bySlot, projected_cost: { cents: 0, unpriced: 0, as_of: null, currency: "USD", store: "tcgplayer", source: "Scryfall" }, cost_delta: { cents: 0, unpriced: 0, as_of: null, currency: "USD", store: "tcgplayer", source: "Scryfall" } },
    unslotted_count: main.filter((c) => c.slot_id == null && c.role === "card").reduce((n, c) => n + c.quantity, 0),
    maybe_count: cards.filter((c) => c.maybe).length,
    slot_deltas: slots.map((s) => {
      const count = main.filter((c) => c.slot_id === s.id).reduce((n, c) => n + c.quantity, 0);
      const secondaryCount = main.filter(c => c.slot_id !== s.id && c.secondary_slot_ids.includes(s.id)).reduce((n, c) => n + c.quantity, 0);
      const { target_min: min, target_max: max } = s;
      const under = min != null && count < min;
      const over = max != null && count > max;
      return {
        slot_id: s.id,
        name: s.name,
        count,
        primary_count: count,
        secondary_count: secondaryCount,
        coverage_count: count + secondaryCount,
        target_min: min,
        target_max: max,
        status: under ? "under" : over ? "over" : min != null || max != null ? "ok" : "untargeted",
        delta: under ? count - min : over ? count - max : 0,
      };
    }),
    identity_violations: [],
    singleton_violations: [],
    legality_violations: [],
    land_count: 0,
    average_mana_value: null,
    curve: { "0": 0, "1": 0, "2": 0, "3": 0, "4": 0, "5": 0, "6": 0, "7+": 0 },
    pips: { W: 0, U: 0, B: 0, R: 0, G: 0 },
    cost: { cents: 0, unpriced: 0, as_of: null, currency: "USD", store: "tcgplayer", source: "Scryfall" },
  };
}

/** A minimal valid DeckState. `computed` is derived from the rest unless given; pass a partial to patch it. */
export function deckState(
  over: Partial<Omit<DeckState, "computed">> & { computed?: Partial<DeckState["computed"]> } = {},
): DeckState {
  const slots = over.slots ?? [];
  const cards = over.cards ?? [];
  const proposals = over.proposals ?? [];
  const { computed, ...rest } = over;
  return {
    deck: {
      id: 1,
      name: "Test Deck",
      category: "theorycrafted",
      parent: null,
      upstream: null,
      visibility: "private",
      slug: null,
      publish_decisions: false,
      ci_mask: null,
      color_identity: "",
      revision: 1,
    },
    slots,
    tags: [],
    cards,
    power: { target: null, assessment: null, history: [], game_changers: { count: 0, unknown_cards: 0, cards: [] }, differences: [], fields: [], guidance: { version: "2026-02-09", url: "", expectations_url: "" } },
    brief: { thesis: "", constraints_md: "", engines: [], updated_at: null },
    checks: [],
    proposals,
    document_edits: [],
    log: [],
    memory_budget: { budget: 1000, used: 0, omitted: 0 },
    ...rest,
    computed: { ...computedFor(slots, cards, proposals), ...computed },
  };
}
