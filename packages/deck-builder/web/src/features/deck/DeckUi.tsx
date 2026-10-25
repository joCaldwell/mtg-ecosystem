// What is on screen in the deck view that the server does not know about:
// the grouping, the open tab, the open card, the selection, and whether a
// review is in progress. One reducer, so the rules between them live in one
// place — opening a card clears nothing, starting a review switches the panel
// to Review, and so on.

import { createContext, useContext, useEffect, useMemo, useReducer, type ReactNode } from "react";
import type { SortBy } from "../../domain/cards.ts";
import type { GroupBy } from "../../domain/grouping.ts";
import type { TagMatch } from "../../domain/deckSearch.ts";

export type PanelTab = "review" | "brief" | "power" | "checks" | "memories" | "history";
export type DeckDialog = "fork" | "compare" | "interop" | "slots" | "delete" | "share";

export interface OpenCard {
  oracleId: string;
  name: string;
}

export interface DeckUiState {
  groupBy: GroupBy;
  /** The order of the cards inside each group. */
  sortBy: SortBy;
  searchTerm: string;
  tagIds: number[];
  tagMatch: TagMatch;
  tagInverted: boolean;
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
  | { t: "sortBy"; value: SortBy }
  | { t: "searchTerm"; value: string }
  | { t: "tagIds"; value: number[] }
  | { t: "tagMatch"; value: TagMatch }
  | { t: "tagInverted"; value: boolean }
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
const SORT_BY_KEY = "deck.sortBy";

function storedSortBy(): SortBy {
  const raw = localStorage.getItem(SORT_BY_KEY);
  return raw === "name" || raw === "price" ? raw : "curve";
}

function reducer(ui: DeckUiState, action: Action): DeckUiState {
  switch (action.t) {
    case "groupBy":
      return { ...ui, groupBy: action.value };
    case "sortBy":
      return { ...ui, sortBy: action.value };
    case "searchTerm":
      return { ...ui, searchTerm: action.value };
    case "tagIds":
      return { ...ui, tagIds: action.value, tagInverted: action.value.length > 0 && ui.tagInverted };
    case "tagMatch":
      return { ...ui, tagMatch: action.value };
    case "tagInverted":
      return { ...ui, tagInverted: action.value };
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
  setSortBy: (value: SortBy) => void;
  setSearchTerm: (value: string) => void;
  setTagIds: (value: number[]) => void;
  setTagMatch: (value: TagMatch) => void;
  setTagInverted: (value: boolean) => void;
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
    sortBy: storedSortBy(),
    searchTerm: "",
    tagIds: [],
    tagMatch: "any",
    tagInverted: false,
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

  useEffect(() => {
    localStorage.setItem(SORT_BY_KEY, ui.sortBy);
  }, [ui.sortBy]);

  const actions = useMemo(
    () => ({
      setGroupBy: (value: GroupBy) => dispatch({ t: "groupBy", value }),
      setSortBy: (value: SortBy) => dispatch({ t: "sortBy", value }),
      setSearchTerm: (value: string) => dispatch({ t: "searchTerm", value }),
      setTagIds: (value: number[]) => dispatch({ t: "tagIds", value }),
      setTagMatch: (value: TagMatch) => dispatch({ t: "tagMatch", value }),
      setTagInverted: (value: boolean) => dispatch({ t: "tagInverted", value }),
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
