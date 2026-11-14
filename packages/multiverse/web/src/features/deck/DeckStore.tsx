import { DeckListProvider } from "./DeckListStore.tsx";
export { DeckListProvider, useDeckList } from "./DeckListStore.tsx";
// One store per open deck. Server state is authoritative — every mutation
// returns the fresh composite under `state` (the server's envelope rule) —
// so the client never merges: apply() overwrites. Components call
// `run(() => api.x(...))` and the response lands here; no setState or mutate
// callbacks are threaded through props.

import { createContext, useCallback, useContext, useEffect, useReducer, useRef, useState, type ReactNode } from "react";
import { api, ApiError, type DeckState, type Envelope } from "../../api/client.ts";
import { acceptsDeckState } from "./stateOrdering.ts";
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
      if (!action.payload.state || !acceptsDeckState(data.state?.deck.id ?? action.payload.state.deck.id, data.state, action.payload.state)) return data;
      return { ...data, state: action.payload.state, missing: false, loadError: null,
        error: data.loadError && data.error === data.loadError.message ? null : data.error };
    case "loadError":
      return { ...data, state: action.status === 403 || action.status === 404 ? null : data.state,
        missing: action.status === 404, loadError: { status: action.status, message: action.message },
        error: data.state && action.status !== 403 && action.status !== 404 ? action.message : data.error };
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
  // Keep a new deck's first render separate even when a consumer forgets key.
  return <DeckSession key={deckId} deckId={deckId}>{children}</DeckSession>;
}

function DeckSession({ deckId, children }: { deckId: number; children: ReactNode }) {
  const [data, dispatch] = useReducer(reducer, { state: null, missing: false, loadError: null, error: null });
  const lifetime = useRef(0);
  const writes = useRef(0);
  useEffect(() => {
    lifetime.current += 1;
    return () => { lifetime.current += 1; };
  }, []);
  const live = useLive({ resources: ["deck"], deckId });
  const [attempt, setAttempt] = useState(0);
  const retry = useCallback(() => setAttempt(n => n + 1), []);

  // A local command invalidates older reads, even when only proposal/log data
  // changed (those do not all increment the membership/edit versions).
  const apply = useCallback((payload: Envelope) => {
    if (!payload.state || payload.state.deck.id !== deckId) return;
    writes.current += 1;
    dispatch({ t: "apply", payload });
    // Reconcile equal-version metadata from concurrent commands with a fresh
    // full server composite. Never merge or fabricate command receipt fields.
    retry();
  }, [deckId, retry]);
  const run = useCallback(async <T extends Envelope>(fn: () => Promise<T>): Promise<T | null> => {
    const generation = lifetime.current;
    dispatch({ t: "error", message: null });
    try {
      const result = await fn();
      if (lifetime.current === generation) apply(result);
      return result;
    } catch (e: unknown) {
      if (lifetime.current === generation) dispatch({ t: "error", message: e instanceof Error ? e.message : String(e) });
      return null;
    }
  }, [apply]);
  const clearError = useCallback(() => dispatch({ t: "error", message: null }), []);

  // Refetched whenever the server reports a change, so what an agent does in
  // another window shows up here without a reload.
  useEffect(() => {
    let current = true;
    const write = writes.current;
    api
      .getDeck(deckId)
      .then((r) => {
        if (current && write === writes.current && r.state.deck.id === deckId) dispatch({ t: "apply", payload: r });
      })
      .catch((error: unknown) => current && write === writes.current && dispatch({ t: "loadError", status: error instanceof ApiError ? error.status : 0, message: error instanceof Error ? error.message : String(error) }));
    return () => {
      current = false;
    };
  }, [deckId, live, attempt]);

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
