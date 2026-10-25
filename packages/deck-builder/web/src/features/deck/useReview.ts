// Ruling on proposals (spec §8.2). One place for what accept and reject do,
// used by the banner, the keyboard shortcuts and the review panel, so the
// three cannot drift: the ruling goes to the server, the fresh deck and inbox
// are applied, and the owner is told what happened with a way back.

import { useCallback, useEffect } from "react";
import { api } from "../../api/client.ts";
import { useInbox } from "../../app/Inbox.tsx";
import type { ReviewUnit } from "../../domain/review.ts";
import { useToast } from "../../ui/index.ts";
import { useDeck } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";

export interface Review {
  /** The unit under the cursor, or null when nothing is pending. */
  current: ReviewUnit | null;
  accept: (unit: ReviewUnit) => Promise<void>;
  reject: (unit: ReviewUnit, reason: string) => Promise<void>;
  /** Accept every pending item of every open proposal on this deck. */
  acceptAll: () => Promise<void>;
}

export function useReview(units: ReviewUnit[]): Review {
  const { deckId, state, run } = useDeck();
  const ui = useDeckUi();
  const inbox = useInbox();
  const { toast } = useToast();

  const current = units.length ? units[Math.min(ui.cursor, units.length - 1)] : null;

  // The last decision ends the review; a cursor past the end follows the list up.
  const { reviewing, cursor, stopReview, setCursor } = ui;
  useEffect(() => {
    if (reviewing && units.length === 0) stopReview();
    else if (cursor > Math.max(0, units.length - 1)) setCursor(units.length - 1);
  }, [reviewing, cursor, units.length, stopReview, setCursor]);

  const undo = useCallback(
    (logId: number) => {
      void run(() => api.undo(deckId, logId));
    },
    [deckId, run],
  );

  const accept = useCallback(
    async (unit: ReviewUnit) => {
      const proposal = state?.proposals.find(p => p.id === unit.proposalId);
      if (proposal?.stale && !window.confirm(`This proposal was based on r${proposal.base_revision ?? "unknown"}; the deck is now r${state!.deck.revision}. Have you reviewed those changes and want to accept this item/group?`)) return;
      const result = await run(() => api.ruleItems(deckId, unit.itemIds, "accept", "", {
        expected_revision: state?.deck.revision, acknowledge_stale: proposal?.stale === true,
      }));
      if (!result) return;
      inbox.apply(result.inbox);
      const entry = result.state.log.find((e) => e.kind === "accept");
      toast(
        `Accepted. The deck is at r${result.state.deck.revision}.`,
        entry ? { action: { label: "Undo", onAction: () => undo(entry.id) } } : undefined,
      );
    },
    [deckId, state, run, inbox, toast, undo],
  );

  const reject = useCallback(
    async (unit: ReviewUnit, reason: string) => {
      const result = await run(() => api.ruleItems(deckId, unit.itemIds, "reject", reason.trim()));
      if (!result) return;
      inbox.apply(result.inbox);
      ui.setRejecting(null);
      toast(
        reason.trim()
          ? "Rejected. An agent will read your reason at its next session."
          : "Rejected. No reason given.",
      );
    },
    [deckId, run, inbox, toast, ui],
  );

  const acceptAll = useCallback(async () => {
    const open = (state?.proposals ?? []).filter((p) => p.items.some((i) => i.status === "pending"));
    let revision: number | null = null;
    if (open.some(p => p.stale) && !window.confirm("Some proposals were based on an older deck revision. Have you reviewed the changed list and want to accept all current proposals?")) return;
    for (const proposal of open) {
      const expected: number = revision ?? state!.deck.revision;
      const result = await run(() => api.ruleProposal(deckId, proposal.id, "accept", "", {
        expected_revision: expected, acknowledge_stale: proposal.stale || proposal.base_revision !== expected,
      }));
      if (!result) return;
      inbox.apply(result.inbox);
      revision = result.state.deck.revision;
    }
    if (revision != null) toast(`Accepted everything that was open. The deck is at r${revision}.`);
  }, [deckId, state, run, inbox, toast]);

  return { current, accept, reject, acceptAll };
}
