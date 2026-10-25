// The composite deck view: one payload with everything the deck page needs,
// returned by every mutation so the client never reconciles partial updates.

import type { Database } from "./sqlite.ts";
import { getPower } from "./deck/power.ts";
import { getBrief } from "./deck/brief.ts";
import { checkDeck } from "./deck/checks.ts";
import { listLog } from "./deck/log.ts";
import { listProposals } from "./deck/proposals.ts";
import { getDeck } from "./deck/service.ts";
import { listDocumentEdits } from "./documents.ts";
import { loadScope } from "./memory.ts";

export function deckState(db: Database, deckId: number) {
  const memories = loadScope(db, "deck", deckId);
  return {
    ...getDeck(db, deckId),
    brief: getBrief(db, deckId),
    power: getPower(db, deckId),
    checks: checkDeck(db, deckId),
    proposals: listProposals(db, { deckId, status: "open" }),
    document_edits: listDocumentEdits(db, { deckId, status: "pending" }),
    log: listLog(db, { deckId, limit: 40 }),
    // What an agent opening this deck is shown, and whether it all fit.
    memory_budget: { budget: memories.budget, used: memories.used, omitted: memories.omitted },
  };
}

export type DeckState = ReturnType<typeof deckState>;
