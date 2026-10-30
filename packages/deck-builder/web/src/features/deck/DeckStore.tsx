import { DeckListProvider } from "./DeckListStore.tsx";
export { DeckListProvider, useDeckList } from "./DeckListStore.tsx";
// One store per open deck. Server state is authoritative — every mutation
// returns the fresh composite under `state` (the server's envelope rule) —
// so the client never merges: apply() overwrites. Components call
// `run(() => api.x(...))` and the response lands here; no setState or mutate
// callbacks are threaded through props.

import { createContext, useCallback, useContext, useEffect, useMemo, useReducer, useState, type ReactNode } from "react";
import { api, ApiError, type DeckListState, type DeckState, type Envelope } from "../../api/client.ts";
import { useLive } from "../../api/live.tsx";

interface DeckData {
  state: DeckState | null;
  /** The deck does not exist (deleted, or a stale link). */
  missing: boolean;
  loadError: { status: number; message: string } | null;
  /** The last failed action, shown as a banner until the next action. */
  error: string | null;
}

type Action =
  | { t: "apply"; payload: Envelope }
  | { t: "loadError"; status: number; message: string }
  | { t: "error"; message: string | null };

function reducer(data: DeckData, action: Action): DeckData {
  switch (action.t) {
    case "apply":
      return { ...data, state: action.payload.state ?? data.state, missing: false, loadError: null };
    case "loadError":
      return { ...data, state: null, missing: action.status === 404, loadError: { status: action.status, message: action.message } };
    case "error":
      return { ...data, error: action.message };
  }
}

export interface DeckStore extends DeckData {
  deckId: number;
  apply: (payload: Envelope) => void;
  /** Run an API call: clears the error banner, applies the response, and on
   *  failure shows the error and resolves null instead of throwing. */
  run: <T extends Envelope>(fn: () => Promise<T>) => Promise<T | null>;
  clearError: () => void;
  retry: () => void;
}

const DeckContext = /* @__PURE__ */ createContext<DeckStore | null>(null);

export function DeckProvider({ deckId, children }: { deckId: number; children: ReactNode }) {
  const [data, dispatch] = useReducer(reducer, { state: null, missing: false, loadError: null, error: null });
  const live = useLive({ resources: ["deck"], deckId });
  const [attempt, setAttempt] = useState(0);
  const retry = useCallback(() => setAttempt(n => n + 1), []);

  // Stable identities (dispatch is stable), so effects may depend on them.
  const apply = useCallback((payload: Envelope) => dispatch({ t: "apply", payload }), []);
  const run = useCallback(async <T extends Envelope>(fn: () => Promise<T>): Promise<T | null> => {
    dispatch({ t: "error", message: null });
    try {
      const result = await fn();
      dispatch({ t: "apply", payload: result });
      return result;
    } catch (e: unknown) {
      dispatch({ t: "error", message: e instanceof Error ? e.message : String(e) });
      return null;
    }
  }, []);
  const clearError = useCallback(() => dispatch({ t: "error", message: null }), []);

  // Refetched whenever the server reports a change, so what an agent does in
  // another window shows up here without a reload.
  useEffect(() => {
    let current = true;
    api
      .getDeck(deckId)
      .then((r) => current && apply(r))
      .catch((error: unknown) => current && dispatch({ t: "loadError", status: error instanceof ApiError ? error.status : 0, message: error instanceof Error ? error.message : String(error) }));
    return () => {
      current = false;
    };
  }, [deckId, apply, live, attempt]);

  return (
    <DeckContext.Provider value={{ ...data, deckId, apply, run, clearError, retry }}>
      {data.state ? <DeckListProvider state={data.state}>{children}</DeckListProvider> : children}
    </DeckContext.Provider>
  );
}

// ---------- the list's own context ----------
// The list, its groups and rows read only this, so the same components draw a
// shared deck (spec §18.5) and the phone view (§11.2) as well as the editor.

export function useDeck(): DeckStore {
  const store = useContext(DeckContext);
  if (!store) throw new Error("useDeck outside DeckProvider");
  return store;
}

/** The loaded deck. For components rendered only once the deck has arrived. */
export function useDeckState(): DeckState {
  const { state } = useDeck();
  if (!state) throw new Error("useDeckState before the deck loaded");
  return state;
}
