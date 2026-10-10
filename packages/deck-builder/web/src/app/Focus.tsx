// Focus (spec §10.3): the deck and cards the owner has open. The deck page
// reports it to the server so that "this deck" in a conversation with an
// agent means the one on screen; the sidebar shows the same thing back to the
// owner, so what an agent will be told is never a surprise.

import { createContext, useContext, useMemo, useState, type ReactNode } from "react";

export interface Focus {
  deckId: number;
  deckName: string;
  selectedCount: number;
}

interface FocusStore {
  focus: Focus | null;
  setFocus: (focus: Focus | null) => void;
}

const FocusContext = createContext<FocusStore>({ focus: null, setFocus: () => {} });

export function FocusProvider({ children }: { children: ReactNode }) {
  const [focus, setFocus] = useState<Focus | null>(null);
  const store = useMemo(() => ({ focus, setFocus }), [focus]);
  return <FocusContext.Provider value={store}>{children}</FocusContext.Provider>;
}

export function useFocus(): FocusStore {
  return useContext(FocusContext);
}
