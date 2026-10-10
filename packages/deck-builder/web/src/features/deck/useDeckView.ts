// The deck as it is drawn: grouped, with the review overlay and the per-card
// markers. Everything here is derived from the server composite by the pure
// functions in domain/, memoised so the list does not regroup on every
// keystroke elsewhere on the page.

import { createContext, useContext, useMemo } from "react";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { layoutDeck, type DeckLayout } from "../../domain/grouping.ts";
import { cardMarks, type CardMark } from "../../domain/marks.ts";
import { reviewOverlay, reviewUnits, type ReviewOverlay, type ReviewUnit } from "../../domain/review.ts";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";

export interface DeckViewModel {
  layout: DeckLayout;
  /** The decisions still open, in the order the owner walks them. */
  units: ReviewUnit[];
  /** What the list would be if everything pending were accepted. */
  overlay: ReviewOverlay;
  marks: Map<string, CardMark>;
}

const DeckViewContext = createContext<DeckViewModel | null>(null);

/** Provided once by the deck page, so the list is grouped a single time. */
export const DeckViewProvider = DeckViewContext.Provider;

/** The view the deck page computed. For components under it that take no props. */
export function useSharedDeckView(): DeckViewModel {
  const view = useContext(DeckViewContext);
  if (!view) throw new Error("useSharedDeckView outside DeckViewProvider");
  return view;
}

export function useDeckView(): DeckViewModel {
  const state = useDeckState();
  const { groupBy } = useDeckUi();
  // Active memories of every scope; cardMarks keeps the global ones and this deck's.
  const memories = useQuery(() => api.listMemories(), [state.deck.id]);

  const layout = useMemo(() => layoutDeck(state, groupBy), [state, groupBy]);
  const units = useMemo(() => reviewUnits(state.proposals), [state.proposals]);
  const overlay = useMemo(() => reviewOverlay(state, layout, groupBy), [state, layout, groupBy]);
  const marks = useMemo(() => cardMarks(state, memories.data?.memories ?? []), [state, memories.data]);

  return { layout, units, overlay, marks };
}
