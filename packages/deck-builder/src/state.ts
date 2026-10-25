import type { Actor } from "./actor.ts";
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

export function deckState(db: Database, actor: Actor, deckId: number) {
  const memories = loadScope(db, actor, "deck", deckId);
  return {
    ...getDeck(db, actor, deckId),
    brief: getBrief(db, actor, deckId),
    power: getPower(db, actor, deckId),
    checks: checkDeck(db, actor, deckId),
    proposals: listProposals(db, actor, { deckId, status: "open" }),
    document_edits: listDocumentEdits(db, actor, { deckId, status: "pending" }),
    log: listLog(db, actor, { deckId, limit: 40 }),
    // What an agent opening this deck is shown, and whether it all fit.
    memory_budget: { budget: memories.budget, used: memories.used, omitted: memories.omitted },
  };
}

export type DeckState = ReturnType<typeof deckState>;
