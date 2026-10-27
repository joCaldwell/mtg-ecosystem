import { createContext, useContext, type ReactNode } from "react";
import { api } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";

const Context = createContext({ hasAgentActivity: false });
export function PreferencesProvider({ children }: { children: ReactNode }) {
  const settings = useQuery(() => api.getSettings(), []);
  return <Context.Provider value={{ hasAgentActivity: settings.data?.has_agent_activity ?? false }}>{children}</Context.Provider>;
}
export function usePreferences() { return useContext(Context); }
/** Both voices live together so copy reviews can assess the same state. */
export function useVoice() { const { hasAgentActivity } = usePreferences(); return (first: string, connected: string) => hasAgentActivity ? connected : first; }
