// The sidebar shows the context the server would give an agent, including
// when another tab owns it. A failed refresh preserves the last confirmed view.
import { createContext, useContext, useEffect, useMemo, type ReactNode } from "react";
import { api } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
export interface Focus { deckId: number; deckName: string; selectedCount: number; stale: boolean; ageMinutes: number }
const FocusContext = createContext<{ focus: Focus | null }>({ focus: null });
export function FocusProvider({ children }: { children: ReactNode }) {
  const query = useQuery(api.getFocus, [], "focus");
  useEffect(() => {
    const timer = window.setInterval(query.reload, 30_000);
    return () => window.clearInterval(timer);
  }, [query.reload]);
  const state = query.data?.focus;
  const value = useMemo(() => ({ focus: state ? { deckId: state.deck_id, deckName: state.deck_name, selectedCount: state.selected.length, stale: state.stale, ageMinutes: state.age_minutes } : null }), [state]);
  return <FocusContext.Provider value={value}>{children}</FocusContext.Provider>;
}
export function useFocus() { return useContext(FocusContext); }
