import { createContext, useContext, useState, useEffect, type ReactNode } from "react";
import { api } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { Disclosure, ErrorNote } from "../ui/index.ts";

const Context = createContext({ hasAgentActivity: false, advanced: false, busy: false, error: null as string | null, setAdvanced: (_open: boolean) => {} });
export function PreferencesProvider({ children }: { children: ReactNode }) {
  const settings = useQuery(() => api.getSettings(), []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<boolean | null>(null);
  useEffect(() => setSaved(null), [settings.data]);
  async function setAdvanced(open: boolean) {
    if (busy) return;
    setBusy(true); setError(null);
    try { const result = await api.updateSettings({ advanced_open: open }); setSaved(result.advanced_open); settings.reload(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  return <Context.Provider value={{ hasAgentActivity: settings.data?.has_agent_activity ?? false, advanced: saved ?? settings.data?.advanced_open ?? false, busy, error, setAdvanced }}>{children}</Context.Provider>;
}
export function usePreferences() { return useContext(Context); }
/** Both voices live together so copy reviews can assess the same state. */
export function useVoice() { const { hasAgentActivity } = usePreferences(); return (first: string, connected: string) => hasAgentActivity ? connected : first; }
export function Advanced({ children, inline = false }: { children: ReactNode; inline?: boolean }) {
  const { advanced, setAdvanced, busy, error } = usePreferences();
  return <><Disclosure inline={inline} open={advanced} onChange={setAdvanced} disabled={busy}>{children}</Disclosure>{error && <ErrorNote>{error}</ErrorNote>}</>;
}
