// One store per open deck. Server state is authoritative — every mutation
// returns fresh composites under named keys (the server's envelope rule) —
// so the client never merges: apply() overwrites whichever slices arrived.
// Components call `run(() => api.x(...))` and the response lands here; no
// setState or mutate callbacks are threaded through props.

import { createContext, useCallback, useContext, useReducer, type ReactNode } from "react";
import type { AuditState, Brief, ContextMeter, DeckState, Envelope } from "./api.ts";

export interface DeckData {
  state: DeckState | null;
  audit: AuditState | null;
  brief: Brief | null;
  meter: ContextMeter | null;
  error: string | null;
  /** Which side-panel tab is up — in the store so any surface (e.g. "ask
   *  agent" on an audit finding) can bring up the chat. */
  sideTab: "search" | "chat";
}

type Action =
  | { t: "apply"; payload: Envelope }
  | { t: "error"; message: string | null }
  | { t: "sideTab"; tab: "search" | "chat" };

function reducer(data: DeckData, action: Action): DeckData {
  switch (action.t) {
    case "apply": {
      const p = action.payload;
      return {
        ...data,
        state: p.state ?? data.state,
        audit: p.audit ?? data.audit,
        brief: p.brief ?? data.brief,
        meter: p.meter ?? data.meter,
      };
    }
    case "error":
      return { ...data, error: action.message };
    case "sideTab":
      return { ...data, sideTab: action.tab };
  }
}

export interface DeckStore extends DeckData {
  deckId: number;
  apply: (payload: Envelope) => void;
  /** Run an API call: clears the error banner, applies the response, and on
   *  failure shows the error and resolves null instead of throwing. */
  run: <T extends Envelope>(fn: () => Promise<T>) => Promise<T | null>;
  setSideTab: (tab: "search" | "chat") => void;
}

const DeckContext = createContext<DeckStore | null>(null);

export function DeckProvider({ deckId, children }: { deckId: number; children: ReactNode }) {
  const [data, dispatch] = useReducer(reducer, {
    state: null,
    audit: null,
    brief: null,
    meter: null,
    error: null,
    sideTab: "search",
  });

  // Stable identities (dispatch is stable), so effects may depend on them.
  const apply = useCallback((payload: Envelope) => dispatch({ t: "apply", payload }), []);
  const run = useCallback(async <T extends Envelope>(fn: () => Promise<T>): Promise<T | null> => {
    dispatch({ t: "error", message: null });
    try {
      const r = await fn();
      dispatch({ t: "apply", payload: r });
      return r;
    } catch (e: any) {
      dispatch({ t: "error", message: e.message });
      return null;
    }
  }, []);
  const setSideTab = useCallback((tab: "search" | "chat") => dispatch({ t: "sideTab", tab }), []);

  return (
    <DeckContext.Provider value={{ ...data, deckId, apply, run, setSideTab }}>
      {children}
    </DeckContext.Provider>
  );
}

export function useDeck(): DeckStore {
  const store = useContext(DeckContext);
  if (!store) throw new Error("useDeck outside DeckProvider");
  return store;
}
