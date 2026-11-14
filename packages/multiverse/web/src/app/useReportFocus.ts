import { useEffect, useRef } from "react";
import { api } from "../api/client.ts";
const HEARTBEAT_MS = 60_000;

export function useReportFocus(deckId: number, _deckName: string | null, selected: string[]): void {
  const current = useRef({ deckId, selected });
  current.current = { deckId, selected };
  const reportCurrent = useRef<(claim: boolean) => void>(() => {});
  const selection = selected.join(",");
  useEffect(() => {
    // A new mount (including StrictMode and bfcache restore) gets a fresh token.
    let tab = { id: crypto.randomUUID(), sequence: 0 };
    let timer: ReturnType<typeof setTimeout> | undefined;
    let claiming = false;
    const send = (claim: boolean) => {
      if (document.visibilityState !== "visible") return;
      void api.setFocus(current.current.deckId, current.current.selected,
        { tab_id: tab.id, sequence: ++tab.sequence, claim }).catch(() => {});
    };
    // Pointer/focus/selection bursts share one report and retain its claim.
    const schedule = (claim: boolean) => {
      claiming ||= claim;
      timer ??= setTimeout(() => { timer = undefined; const take = claiming; claiming = false; send(take); }, 75);
    };
    reportCurrent.current = schedule;
    const interact = () => schedule(true);
    const visible = () => schedule(false);
    const clear = () => {
      if (timer !== undefined) { clearTimeout(timer); timer = undefined; claiming = false; }
      api.clearFocusOnExit({ tab_id: tab.id, sequence: ++tab.sequence });
    };
    const restore = (event: PageTransitionEvent) => {
      if (event.persisted) { tab = { id: crypto.randomUUID(), sequence: 0 }; schedule(false); }
    };
    // Mount and visibility alone may renew this tab or claim an empty lease.
    schedule(false);
    window.addEventListener("focus", interact);
    window.addEventListener("pointerdown", interact);
    window.addEventListener("keydown", interact);
    document.addEventListener("visibilitychange", visible);
    window.addEventListener("pagehide", clear);
    window.addEventListener("pageshow", restore);
    const heartbeat = window.setInterval(visible, HEARTBEAT_MS);
    return () => {
      reportCurrent.current = () => {};
      if (timer !== undefined) clearTimeout(timer);
      window.clearInterval(heartbeat);
      window.removeEventListener("focus", interact);
      window.removeEventListener("pointerdown", interact);
      window.removeEventListener("keydown", interact);
      document.removeEventListener("visibilitychange", visible);
      window.removeEventListener("pagehide", clear);
      window.removeEventListener("pageshow", restore);
      void api.setFocus(null, [], { tab_id: tab.id, sequence: ++tab.sequence }).catch(() => {});
    };
  }, []);
  useEffect(() => { reportCurrent.current(false); }, [deckId, selection]);
}
