// Find-in-deck keeps every row in place and mutes the nonmatches; this steps
// through the matches instead, in the order the list draws them. The toolbar's
// find box (Enter, Shift+Enter) and the filter summary's previous/next share it.

import { useSharedDeckView } from "./DeckViewContext.ts";
import { useDeckUi } from "./DeckUi.tsx";

export interface FindInDeck {
  active: boolean;
  total: number;
  /** Zero-based position of the current match; 0 when there are none. */
  index: number;
  /** Move to the next (1) or previous (-1) match, wrapping at either end. */
  step: (delta: 1 | -1) => void;
}

export function useFindInDeck(): FindInDeck {
  const { findIndex, setFindIndex } = useDeckUi();
  const { matches } = useSharedDeckView();
  const total = matches.ordered?.length ?? 0;
  const index = Math.min(findIndex, Math.max(0, total - 1));
  return {
    active: matches.active,
    total,
    index,
    step: (delta) => { if (total) setFindIndex((index + delta + total) % total); },
  };
}
