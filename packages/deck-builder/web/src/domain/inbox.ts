import type { Inbox, Proposal, ProposalItem } from "../api/client.ts";

/** The items of a proposal the owner has not ruled on yet. */
export function pendingItems(proposal: Proposal): ProposalItem[] {
  return proposal.status !== "open" || proposal.disposition !== "current" ? [] : proposal.items.filter((i) => i.status === "pending");
}

/** What is waiting on the owner: proposals that still have a pending item, plus document edits. */
export function waitingCount(inbox: Inbox): number {
  return (
    inbox.proposals.filter((p) => pendingItems(p).length > 0).length +
    new Set(inbox.document_edits.filter((e) => e.status === "pending").map(e => e.group_id ?? e.id)).size
  );
}
