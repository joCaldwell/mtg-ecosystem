import { createContext, useContext, useState, useEffect, type ReactNode } from "react";
import { api } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { useToast } from "../ui/index.ts";

const Context = createContext({
  hasAgentActivity: false,
  sidebarCollapsed: false, sidebarBusy: false,
  setSidebarCollapsed: (_collapsed: boolean) => {},
  loaded: false,
});
export function PreferencesProvider({ children }: { children: ReactNode }) {
  const { toast } = useToast();
  const [sidebarSaved, setSidebarSaved] = useState<boolean | null>(null);
  const [sidebarBusy, setSidebarBusy] = useState(false);
  const settings = useQuery(() => api.getPreferences(), [], "preferences");
  useEffect(() => setSidebarSaved(null), [settings.data]);
  async function setSidebarCollapsed(collapsed: boolean) {
    if (sidebarBusy) return;
    setSidebarBusy(true);
    try {
      const result = await api.updateSettings({ sidebar_collapsed: collapsed });
      setSidebarSaved(result.sidebar_collapsed);
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e));
    } finally { setSidebarBusy(false); }
  }
  return <Context.Provider value={{ sidebarCollapsed: sidebarSaved ?? settings.data?.sidebar_collapsed ?? false, sidebarBusy, setSidebarCollapsed, hasAgentActivity: settings.data?.has_agent_activity ?? false, loaded: sidebarSaved !== null || settings.data !== null || settings.error !== null }}>{children}</Context.Provider>;
}
export function usePreferences() { return useContext(Context); }
/** Both voices live together so copy reviews can assess the same state. */
export function useVoice() { const { hasAgentActivity } = usePreferences(); return (first: string, connected: string) => hasAgentActivity ? connected : first; }
