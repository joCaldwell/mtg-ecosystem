// What is on screen in the deck view that the server does not know about:
// the grouping, the open tab, the open card, the selection, and whether a
// review is in progress. One reducer, so the rules between them live in one
// place — opening a card clears nothing, starting a review switches the panel
// to Review, and so on.

import { createContext, useContext, useEffect, useMemo, useReducer, type ReactNode } from "react";
import type { GroupBy } from "../../domain/grouping.ts";

export type PanelTab = "review" | "brief" | "checks" | "memories" | "history" | "paper";
export type DeckDialog = "fork" | "compare" | "interop" | "slots" | "delete";

export interface OpenCard {
  oracleId: string;
  name: string;
}

export interface DeckUiState {
  groupBy: GroupBy;
  tab: PanelTab;
  /** The card shown in the side panel, over whichever tab is open. It need
   *  not be in the deck: a proposed add or a card named in a memory opens too. */
  openCard: OpenCard | null;
  /** oracle_ids the owner has selected for a bulk action. Also reported as focus. */
  selected: string[];
  /** Review mode: pending proposal items are drawn into the list. */
  reviewing: boolean;
  /** Index into the pending review units (domain/review.ts). */
  cursor: number;
  /** Key of the unit whose rejection reason is being typed. */
  rejecting: string | null;
  dialog: DeckDialog | null;
}

type Action =
  | { t: "groupBy"; value: GroupBy }
  | { t: "tab"; value: PanelTab }
  | { t: "openCard"; card: OpenCard | null }
  | { t: "toggleSelected"; oracleId: string }
  | { t: "clearSelection" }
  | { t: "startReview" }
  | { t: "stopReview" }
  | { t: "cursor"; index: number }
  | { t: "rejecting"; key: string | null }
  | { t: "dialog"; value: DeckDialog | null };

const GROUP_BY_KEY = "deck.groupBy";

function reducer(ui: DeckUiState, action: Action): DeckUiState {
  switch (action.t) {
    case "groupBy":
      return { ...ui, groupBy: action.value };
    case "tab":
      // Choosing a tab is asking to see it, so the card panel gets out of the way.
      return { ...ui, tab: action.value, openCard: null };
    case "openCard":
      return { ...ui, openCard: action.card };
    case "toggleSelected": {
      const rest = ui.selected.filter((id) => id !== action.oracleId);
      return { ...ui, selected: rest.length === ui.selected.length ? [...rest, action.oracleId] : rest };
    }
    case "clearSelection":
      return { ...ui, selected: [] };
    case "startReview":
      return { ...ui, reviewing: true, tab: "review", openCard: null, cursor: 0, rejecting: null };
    case "stopReview":
      return { ...ui, reviewing: false, rejecting: null };
    case "cursor":
      return { ...ui, cursor: Math.max(0, action.index), rejecting: null };
    case "rejecting":
      return { ...ui, rejecting: action.key };
    case "dialog":
      return { ...ui, dialog: action.value };
  }
}

export interface DeckUi extends DeckUiState {
  setGroupBy: (value: GroupBy) => void;
  setTab: (value: PanelTab) => void;
  openCardPanel: (card: OpenCard | null) => void;
  toggleSelected: (oracleId: string) => void;
  clearSelection: () => void;
  startReview: () => void;
  stopReview: () => void;
  setCursor: (index: number) => void;
  setRejecting: (key: string | null) => void;
  openDialog: (value: DeckDialog) => void;
  closeDialog: () => void;
}

const DeckUiContext = createContext<DeckUi | null>(null);

export function DeckUiProvider({ children }: { children: ReactNode }) {
  const [ui, dispatch] = useReducer(reducer, null, (): DeckUiState => ({
    groupBy: localStorage.getItem(GROUP_BY_KEY) === "type" ? "type" : "slot",
    tab: "brief",
    openCard: null,
    selected: [],
    reviewing: false,
    cursor: 0,
    rejecting: null,
    dialog: null,
  }));

  useEffect(() => {
    localStorage.setItem(GROUP_BY_KEY, ui.groupBy);
  }, [ui.groupBy]);

  const actions = useMemo(
    () => ({
      setGroupBy: (value: GroupBy) => dispatch({ t: "groupBy", value }),
      setTab: (value: PanelTab) => dispatch({ t: "tab", value }),
      openCardPanel: (card: OpenCard | null) => dispatch({ t: "openCard", card }),
      toggleSelected: (oracleId: string) => dispatch({ t: "toggleSelected", oracleId }),
      clearSelection: () => dispatch({ t: "clearSelection" }),
      startReview: () => dispatch({ t: "startReview" }),
      stopReview: () => dispatch({ t: "stopReview" }),
      setCursor: (index: number) => dispatch({ t: "cursor", index }),
      setRejecting: (key: string | null) => dispatch({ t: "rejecting", key }),
      openDialog: (value: DeckDialog) => dispatch({ t: "dialog", value }),
      closeDialog: () => dispatch({ t: "dialog", value: null }),
    }),
    [],
  );

  return <DeckUiContext.Provider value={{ ...ui, ...actions }}>{children}</DeckUiContext.Provider>;
}

export function useDeckUi(): DeckUi {
  const ui = useContext(DeckUiContext);
  if (!ui) throw new Error("useDeckUi outside DeckUiProvider");
  return ui;
}
