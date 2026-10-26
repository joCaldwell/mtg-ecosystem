import { createContext, useContext, useState, useEffect, useId, type ReactNode } from "react";
import { api } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { Disclosure, ErrorNote } from "../ui/index.ts";

const Context = createContext({
  hasAgentActivity: false, advanced: false,
  /** False until /api/settings answers: Advanced has no known state yet. */
  loaded: false,
  busy: false,
  /** A failed save, and which toggle asked for it, so only that one shows it. */
  error: null as { message: string; from: string } | null,
  setAdvanced: (_open: boolean, _from = "") => {},
});
export function PreferencesProvider({ children }: { children: ReactNode }) {
  const settings = useQuery(() => api.getSettings(), []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ message: string; from: string } | null>(null);
  const [saved, setSaved] = useState<boolean | null>(null);
  useEffect(() => setSaved(null), [settings.data]);
  async function setAdvanced(open: boolean, from = "") {
    if (busy) return;
    setBusy(true); setError(null);
    // The response is the saved settings; the change stream refetches the rest.
    try { const result = await api.updateSettings({ advanced_open: open }); setSaved(result.advanced_open); }
    catch (e) { setError({ message: e instanceof Error ? e.message : String(e), from }); }
    finally { setBusy(false); }
  }
  return <Context.Provider value={{ hasAgentActivity: settings.data?.has_agent_activity ?? false, advanced: saved ?? settings.data?.advanced_open ?? false, loaded: saved !== null || settings.data !== null || settings.error !== null, busy, error, setAdvanced }}>{children}</Context.Provider>;
}
export function usePreferences() { return useContext(Context); }
/** Both voices live together so copy reviews can assess the same state. */
export function useVoice() { const { hasAgentActivity } = usePreferences(); return (first: string, connected: string) => hasAgentActivity ? connected : first; }
/** Renders nothing until the account's preference is known, so an open
 *  section does not flash closed on load. `title` says what the toggle
 *  reveals where the content alone would not. */
export function Advanced({ children, inline = false, title }: { children: ReactNode; inline?: boolean; title?: string }) {
  const { advanced, loaded, setAdvanced, busy, error } = usePreferences();
  const id = useId();
  if (!loaded) return null;
  return <><Disclosure inline={inline} title={title} open={advanced} onChange={(open) => setAdvanced(open, id)} disabled={busy}>{children}</Disclosure>{error?.from === id && <ErrorNote>{error.message}</ErrorNote>}</>;
}
