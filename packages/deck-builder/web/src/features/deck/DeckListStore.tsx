import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { DeckListState } from "../../api/client.ts";
interface DeckListValue {
  state: DeckListState;
  /** Nothing in the list may be selected or edited: shared pages and phones. */
  readOnly: boolean;
}

const DeckListContext = /* @__PURE__ */ createContext<DeckListValue | null>(null);

export function DeckListProvider({ state, readOnly = false, children }: { state: DeckListState; readOnly?: boolean; children: ReactNode }) {
  const value = useMemo(() => ({ state, readOnly }), [state, readOnly]);
  return <DeckListContext.Provider value={value}>{children}</DeckListContext.Provider>;
}

export function useDeckList(): DeckListValue {
  const value = useContext(DeckListContext);
  if (!value) throw new Error("useDeckList outside a DeckListProvider");
  return value;
}

