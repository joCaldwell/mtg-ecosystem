// One store per open deck. Server state is authoritative — every mutation
// returns the fresh composite under `state` (the server's envelope rule) —
// so the client never merges: apply() overwrites. Components call
// `run(() => api.x(...))` and the response lands here; no setState or mutate
// callbacks are threaded through props.

import { createContext, useCallback, useContext, useReducer, type ReactNode } from "react";
import type { DeckState, Envelope } from "./api.ts";

export interface DeckData {
  state: DeckState | null;
  error: string | null;
  /** Cards whose rows are open — reported to the server as the owner's focus. */
  selected: string[];
}

type Action =
  | { t: "apply"; payload: Envelope }
  | { t: "error"; message: string | null }
  | { t: "select"; oracleId: string; on: boolean };

function reducer(data: DeckData, action: Action): DeckData {
  switch (action.t) {
    case "apply":
      return { ...data, state: action.payload.state ?? data.state };
    case "error":
      return { ...data, error: action.message };
    case "select": {
      const rest = data.selected.filter((id) => id !== action.oracleId);
      return { ...data, selected: action.on ? [...rest, action.oracleId] : rest };
    }
  }
}

export interface DeckStore extends DeckData {
  deckId: number;
  apply: (payload: Envelope) => void;
  /** Run an API call: clears the error banner, applies the response, and on
   *  failure shows the error and resolves null instead of throwing. */
  run: <T extends Envelope>(fn: () => Promise<T>) => Promise<T | null>;
  setSelected: (oracleId: string, on: boolean) => void;
}

const DeckContext = createContext<DeckStore | null>(null);

export function DeckProvider({ deckId, children }: { deckId: number; children: ReactNode }) {
  const [data, dispatch] = useReducer(reducer, { state: null, error: null, selected: [] });

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
  const setSelected = useCallback(
    (oracleId: string, on: boolean) => dispatch({ t: "select", oracleId, on }),
    [],
  );

  return (
    <DeckContext.Provider value={{ ...data, deckId, apply, run, setSelected }}>
      {children}
    </DeckContext.Provider>
  );
}

export function useDeck(): DeckStore {
  const store = useContext(DeckContext);
  if (!store) throw new Error("useDeck outside DeckProvider");
  return store;
}
