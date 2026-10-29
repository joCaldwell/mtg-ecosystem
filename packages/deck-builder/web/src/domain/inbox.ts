import type { Inbox, Proposal, ProposalItem } from "../api/client.ts";

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

/** What is waiting on the owner: proposals that still have a pending item, plus document edits. */
export function waitingCount(inbox: Inbox): number {
  return (
    inbox.proposals.filter((p) => pendingItems(p).length > 0).length +
    new Set(inbox.document_edits.filter((e) => e.status === "pending").map(e => e.group_id ?? e.id)).size
  );
}
