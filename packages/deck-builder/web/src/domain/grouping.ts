// How the deck list is arranged (spec §5). The server hands over every
// number — slot counts, deltas, the unfiled count — and this module only
// decides which cards sit under which heading. Two things are summed here,
// because they follow the grouping and the grouping is chosen on screen: the
// counts of type groups, and what each group costs (spec §4.4).

import type { DeckCard, DeckState, ProposalItem } from "../api/client.ts";
import { CARD_ORDER, TYPE_GROUPS, byName, byPrice, primaryType, quantityOf, type SortBy } from "./cards.ts";
import { pendingItems } from "./inbox.ts";
import { type GroupCost, costOf } from "./price.ts";

export type GroupBy = "slot" | "type";
export type GroupStatus = "ok" | "under" | "over" | "untargeted";

export interface DeckGroup {
  /** "slot-12", "unslotted", "type-Creature" */
  key: string;
  /** Slot name, "Unfiled", or the type. */
  title: string;
  /** The slot id; null for the unfiled group; undefined for type groups. */
  slotId: number | null | undefined;
  /** Quantity-weighted. */
  count: number;
  /** Formatted target, null when there is none. */
  target: string | null;
  status: GroupStatus;
  /** How far outside the target (negative = under). */
  delta: number;
  /** Role "card", not on the maybe list, in the chosen order. */
  cards: DeckCard[];
  /** What the group's cards cost; unchanged when a search hides some of them. */
  cost: GroupCost;
}

export interface DeckLayout {
  commanders: DeckCard[];
  companions: DeckCard[];
  groups: DeckGroup[];
  /** Sorted by name, or by price when that is the chosen order. */
  maybe: DeckCard[];
}

export const UNFILED_KEY = "unslotted";

export function formatTarget(min: number | null, max: number | null): string | null {
  if (min != null && max != null) return min === max ? String(min) : `${min}–${max}`;
  if (min != null) return `${min}+`;
  if (max != null) return `up to ${max}`;
  return null;
}

/** Every item still waiting for a ruling, across the deck's open proposals. */
export function pendingOf(state: DeckState): ProposalItem[] {
  return state.proposals.flatMap(pendingItems);
}

/**
 * The group a pending add would land in. In slot mode an add with no slot —
 * or whose slot no longer exists, which the server also files as unslotted on
 * accept — goes to the unfiled group.
 */
export function groupKeyForAdd(item: ProposalItem, state: DeckState, groupBy: GroupBy): string {
  if (groupBy === "type") return `type-${primaryType(item.type_line)}`;
  return item.slot_id != null && state.slots.some((s) => s.id === item.slot_id)
    ? `slot-${item.slot_id}`
    : UNFILED_KEY;
}

export function layoutDeck(state: DeckState, groupBy: GroupBy, sortBy: SortBy = "curve"): DeckLayout {
  const inDeck = state.cards.filter((c) => !c.maybe);
  const listed = inDeck.filter((c) => c.role === "card").sort(CARD_ORDER[sortBy]);
  const pendingAdds = pendingOf(state).filter((i) => i.action === "add");
  const addKeys = new Set(pendingAdds.map((i) => groupKeyForAdd(i, state, groupBy)));

  const groups: DeckGroup[] = [];
  if (groupBy === "slot") {
    for (const slot of state.slots) {
      // Empty slots stay so an unmet target still shows. The numbers are the
      // server's; nothing is recounted here.
      const d = state.computed.slot_deltas.find((x) => x.slot_id === slot.id);
      const inSlot = listed.filter((c) => c.slot_id === slot.id);
      groups.push({
        key: `slot-${slot.id}`,
        title: slot.name,
        slotId: slot.id,
        count: d?.count ?? 0,
        target: formatTarget(slot.target_min, slot.target_max),
        status: d?.status ?? "untargeted",
        delta: d?.delta ?? 0,
        cards: inSlot,
        cost: costOf(inSlot),
      });
    }
    const unfiled = listed.filter((c) => c.slot_id == null);
    if (unfiled.length > 0 || addKeys.has(UNFILED_KEY))
      groups.push({
        key: UNFILED_KEY,
        title: "Unfiled",
        slotId: null,
        count: state.computed.unslotted_count,
        target: null,
        status: "untargeted",
        delta: 0,
        cards: unfiled,
        cost: costOf(unfiled),
      });
  } else {
    for (const type of [...TYPE_GROUPS, "Other"]) {
      const cards = listed.filter((c) => primaryType(c.type_line) === type);
      if (cards.length === 0 && !addKeys.has(`type-${type}`)) continue;
      groups.push({
        key: `type-${type}`,
        title: type,
        slotId: undefined,
        count: quantityOf(cards),
        target: null,
        status: "untargeted",
        delta: 0,
        cards,
        cost: costOf(cards),
      });
    }
  }

  return {
    commanders: inDeck.filter((c) => c.role === "commander"),
    companions: inDeck.filter((c) => c.role === "companion"),
    groups,
    // The maybe list has no curve to read, so it only leaves name order for price.
    maybe: state.cards.filter((c) => c.maybe).sort(sortBy === "price" ? byPrice : byName),
  };
}
