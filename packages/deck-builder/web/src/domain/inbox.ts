import type { DeckSummary, Inbox, Proposal, ProposalItem } from "../api/client.ts";

/** The items of a proposal the owner has not ruled on yet. */
export function pendingItems(proposal: Proposal): ProposalItem[] {
  return proposal.status !== "open" || proposal.disposition !== "current" ? [] : proposal.items.filter((i) => i.status === "pending");
}

/** The same count per deck, for the sidebar's deck rows; profile edits belong to no deck. */
export function waitingByDeck(inbox: Inbox): Map<number, number> {
  const counts = new Map<number, number>();
  const add = (deckId: number | null) => { if (deckId != null) counts.set(deckId, (counts.get(deckId) ?? 0) + 1); };
  for (const p of inbox.proposals) if (pendingItems(p).length > 0) add(p.deck_id);
  const groups = new Map<number, number | null>();
  for (const e of inbox.document_edits) if (e.status === "pending") groups.set(e.group_id ?? e.id, e.deck_id);
  for (const deckId of groups.values()) add(deckId);
  return counts;
}

/** Before the inbox loads, a summary can indicate waiting but cannot count
 * proposal/edit groups. Once loaded, its counts are authoritative, even zero. */
export function deckWaitingState(deck: Pick<DeckSummary, "id" | "pending_items" | "pending_document_edits">,
  counts: ReadonlyMap<number, number> | null): { count: number | null; pending: boolean } {
  if (counts === null) return { count: null, pending: deck.pending_items + deck.pending_document_edits > 0 };
  const count = counts.get(deck.id) ?? 0;
  return { count, pending: count > 0 };
}

/** What is waiting on the owner: proposals that still have a pending item, plus document edits. */
export function waitingCount(inbox: Inbox): number {
  return (
    inbox.proposals.filter((p) => pendingItems(p).length > 0).length +
    new Set(inbox.document_edits.filter((e) => e.status === "pending").map(e => e.group_id ?? e.id)).size
  );
}

/**
 * A summary as plain text, for a place that is one button and so cannot hold
 * the card references and emphasis a summary renders elsewhere.
 */
export function plainText(markdown: string): string {
  return markdown.replace(/\[\[([^\]]+)\]\]/g, "$1").replace(/\*\*|`/g, "");
}
