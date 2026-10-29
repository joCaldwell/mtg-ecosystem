import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { DeckListState } from "../../api/client.ts";
interface DeckListValue {
  state: DeckListState;
  /** Nothing in the list may be selected or edited: shared pages and phones. */
  readOnly: boolean;
  /** Someone other than the owner is reading a shared deck. The owner's own
   *  workflow (memory and decision markers, unfiled counts, empty slot
   *  targets) is left out for them. */
  viewer: boolean;
}

const DeckListContext = /* @__PURE__ */ createContext<DeckListValue | null>(null);

export function DeckListProvider({ state, readOnly = false, viewer = false, children }: { state: DeckListState; readOnly?: boolean; viewer?: boolean; children: ReactNode }) {
  const value = useMemo(() => ({ state, readOnly, viewer }), [state, readOnly, viewer]);
  return <DeckListContext.Provider value={value}>{children}</DeckListContext.Provider>;
}

export function useDeckList(): DeckListValue {
  const value = useContext(DeckListContext);
  if (!value) throw new Error("useDeckList outside a DeckListProvider");
  return value;
}

