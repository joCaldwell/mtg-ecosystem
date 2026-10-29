// Decision on proposals (spec §8). The owner decides in units, never in loose
// items, and sees the list as it would look if everything pending were accepted.

import type { DeckListState, Proposal, ProposalItem } from "../api/client.ts";
import { groupKeyForAdd, pendingOf } from "./grouping.ts";
import type { DeckLayout, GroupBy } from "./grouping.ts";
import { pendingItems } from "./inbox.ts";

export interface ReviewUnit {
  /** Stable: `${proposalId}:${group_id}` or `${proposalId}:item-${itemId}` */
  key: string;
  proposalId: number;
  /** swap = exactly one add and one cut ruled together; group = any other multi-item group. */
  kind: "swap" | "add" | "cut" | "group";
  /** Pending items only, adds before cuts. */
  items: ProposalItem[];
  itemIds: number[];
}

function kindOf(items: ProposalItem[]): ReviewUnit["kind"] {
  if (items.length === 1) return items[0].action;
  const adds = items.filter((i) => i.action === "add").length;
  return items.length === 2 && adds === 1 ? "swap" : "group";
}

/**
 * The decisions the owner has left, in order: proposals oldest first, units
 * in first-appearance order. Items sharing a group_id within one proposal are
 * one unit (spec §8.1: a group is ruled on as a unit).
 */
export function reviewUnits(proposals: Proposal[]): ReviewUnit[] {
  const out: ReviewUnit[] = [];
  for (const proposal of [...proposals].sort((a, b) => a.id - b.id)) {
    const byGroup = new Map<string, ProposalItem[]>();
    const ordered: Array<{ key: string; items: ProposalItem[] }> = [];
    for (const item of pendingItems(proposal)) {
      if (!item.group_id) {
        ordered.push({ key: `${proposal.id}:item-${item.id}`, items: [item] });
        continue;
      }
      const existing = byGroup.get(item.group_id);
      if (existing) existing.push(item);
      else {
        const items = [item];
        byGroup.set(item.group_id, items);
        ordered.push({ key: `${proposal.id}:${item.group_id}`, items });
      }
    }
    for (const { key, items } of ordered) {
      // Adds first; Array.sort is stable, so the proposer's order holds within each.
      const sorted = [...items].sort((a, b) => Number(b.action === "add") - Number(a.action === "add"));
      out.push({
        key,
        proposalId: proposal.id,
        kind: kindOf(sorted),
        items: sorted,
        itemIds: sorted.map((i) => i.id),
      });
    }
  }
  return out;
}

/** An open proposal's place in the review queue: its position among the
 * proposals left ("1 of 3"), the first of its decisions in `units` (where the
 * cursor goes to focus it) and how many changes it still asks for. */
export interface QueuedProposal {
  proposalId: number;
  position: number;
  of: number;
  firstUnit: number;
  changes: number;
}

/** The proposals left to decide, oldest first, as `reviewUnits` orders them. */
export function proposalQueue(units: ReviewUnit[]): QueuedProposal[] {
  const out: QueuedProposal[] = [];
  units.forEach((unit, index) => {
    const last = out.at(-1);
    if (last?.proposalId === unit.proposalId) last.changes += unit.items.length;
    else out.push({ proposalId: unit.proposalId, position: out.length + 1, of: 0, firstUnit: index, changes: unit.items.length });
  });
  for (const entry of out) entry.of = out.length;
  return out;
}

export interface ReviewOverlay {
  /** oracle_ids with a pending cut. */
  cuts: Set<string>;
  /** DeckGroup.key -> pending adds to draw in that group. */
  addsByGroup: Map<string, ProposalItem[]>;
  /** DeckGroup.key -> count if everything pending were accepted (only groups whose count would change). */
  projected: Map<string, number>;
  projectedTotal: number;
}

/** What the list looks like if every pending item were accepted. */
export function reviewOverlay(state: DeckListState, layout: DeckLayout, groupBy: GroupBy): ReviewOverlay {
  const pending = pendingOf(state);
  const cuts = new Set(pending.filter((i) => i.action === "cut").map((i) => i.oracle_id));

  const addsByGroup = new Map<string, ProposalItem[]>();
  for (const item of pending) {
    if (item.action !== "add") continue;
    const key = groupKeyForAdd(item, state, groupBy);
    const list = addsByGroup.get(key);
    if (list) list.push(item);
    else addsByGroup.set(key, [item]);
  }

  const projected = new Map<string, number>();
  for (const group of layout.groups) {
    let change: number;
    if (groupBy === "slot" && group.slotId != null) {
      // The server's number, not a recount.
      change = state.computed.pending.by_slot[group.slotId] ?? 0;
    } else {
      // The server has no number for the unfiled group or for types: each
      // item is one copy.
      const gained = addsByGroup.get(group.key)?.length ?? 0;
      const lost = group.cards.filter((c) => cuts.has(c.oracle_id)).length;
      change = gained - lost;
    }
    if (change !== 0) projected.set(group.key, group.count + change);
  }

  return { cuts, addsByGroup, projected, projectedTotal: state.computed.pending.projected_count };
}
