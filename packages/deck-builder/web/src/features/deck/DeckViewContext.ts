import { createContext, useContext } from "react";
import type { DeckViewModel } from "./useDeckView.ts";

const DeckViewContext = /* @__PURE__ */ createContext<DeckViewModel | null>(null);
export const DeckViewProvider = DeckViewContext.Provider;
export function useSharedDeckView(): DeckViewModel {
  const view = useContext(DeckViewContext);
  if (!view) throw new Error("useSharedDeckView outside DeckViewProvider");
  return view;
}
