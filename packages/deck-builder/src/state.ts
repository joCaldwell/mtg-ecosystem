// The composite deck view: one payload with everything the deck page needs,
// returned by every mutation so the client never reconciles partial updates.
// Lives above deck/ and agent/ because it composes both (consolidations are
// agent-side); deck/ itself never imports agent code.

import type { DatabaseSync } from "node:sqlite";
import { getDeck } from "./deck/service.ts";
import { listProposals } from "./deck/proposals.ts";
import { listBriefEdits } from "./deck/brief.ts";
import { getLog, listCardNotes, listHardFilters } from "./deck/log.ts";
import { listPlaytestNotes } from "./deck/interop.ts";
import { listConsolidations } from "./agent/consolidate.ts";

export function deckState(db: DatabaseSync, deckId: number) {
  return {
    ...getDeck(db, deckId),
    proposals: listProposals(db, deckId, "open"),
    brief_edits: listBriefEdits(db, deckId, "pending"),
    log: getLog(db, deckId, 30),
    hard_filters: listHardFilters(db, deckId),
    card_notes: listCardNotes(db, deckId),
    playtest_notes: listPlaytestNotes(db, deckId),
    consolidations: listConsolidations(db, deckId, "pending"),
  };
}

export type DeckState = ReturnType<typeof deckState>;
