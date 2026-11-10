import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { DeckListState } from "../../api/client.ts";
import { deckTokens, type DeckTokens } from "../../domain/tokens.ts";
interface DeckListValue {
  state: DeckListState;
  /** Nothing in the list may be selected or edited: shared pages and phones. */
  readOnly: boolean;
  /** Someone other than the owner is reading a shared deck. The owner's own
   *  workflow (memory and decision markers, unfiled counts, empty slot
   *  targets) is left out for them. */
  viewer: boolean;
  /** The tokens and emblems the deck makes, and whether that list exists yet. */
  tokens: DeckTokens;
}

const DeckListContext = /* @__PURE__ */ createContext<DeckListValue | null>(null);

export function DeckListProvider({ state, tokens, readOnly = false, viewer = false, children }: {
  state: DeckListState;
  /** Defaults to the tokens carried by `state` (a full DeckState). */
  tokens?: DeckTokens;
  readOnly?: boolean;
  viewer?: boolean;
  children: ReactNode;
}) {
  const value = useMemo(() => ({ state, readOnly, viewer, tokens: tokens ?? deckTokens(state) }), [state, tokens, readOnly, viewer]);
  return <DeckListContext.Provider value={value}>{children}</DeckListContext.Provider>;
}

export function useDeckList(): DeckListValue {
  const value = useContext(DeckListContext);
  if (!value) throw new Error("useDeckList outside a DeckListProvider");
  return value;
}
