// The queue: one entry per open proposal that still has something pending, and
// one per pending document edit. Pure, so the page only arranges it.

import type { DocumentEdit, Inbox, Proposal } from "../../api/client.ts";
import { editDiff, isTextEdit } from "../../domain/documents.ts";
import { pendingItems } from "../../domain/inbox.ts";
import { parseServerTime } from "../../domain/time.ts";

export type Entry =
  | { key: string; type: "proposal"; created_at: string; proposal: Proposal }
  | { key: string; type: "edit"; created_at: string; edit: DocumentEdit };

/** Oldest first: it is a queue, and the stalest request is the one waiting longest. */
export function inboxEntries(inbox: Inbox): Entry[] {
  const entries: Entry[] = [
    ...inbox.proposals
      .filter((p) => p.status === "open" && pendingItems(p).length > 0)
      .map((proposal): Entry => ({ key: `proposal-${proposal.id}`, type: "proposal", created_at: proposal.created_at, proposal })),
    ...inbox.document_edits
      .filter((e) => e.status === "pending")
      .map((edit): Entry => ({ key: `edit-${edit.id}`, type: "edit", created_at: edit.created_at, edit })),
  ];
  return entries.sort((a, b) => parseServerTime(a.created_at) - parseServerTime(b.created_at));
}

/** One phrase for what a document edit does, for the list. */
export function describeEdit(edit: DocumentEdit): string {
  if (isTextEdit(edit)) {
    const lines = editDiff(edit);
    const added = lines.filter((l) => l.kind === "add").length;
    const removed = lines.filter((l) => l.kind === "del").length;
    const parts = [added && `${added} ${added === 1 ? "line" : "lines"} added`, removed && `${removed} removed`].filter(Boolean);
    return parts.length ? parts.join(", ") : "no change to the text";
  }
  return edit.kind === "engine_remove" ? "removes an engine" : "defines an engine";
}

/**
 * A summary as plain text, for the list. A row there is one button, which
 * cannot hold the card references and emphasis the detail view renders.
 */
export function plainText(markdown: string): string {
  return markdown.replace(/\[\[([^\]]+)\]\]/g, "$1").replace(/\*\*|`/g, "");
}
