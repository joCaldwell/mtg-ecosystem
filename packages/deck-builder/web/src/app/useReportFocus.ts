// Focus (spec §10.3): tell the server which deck is open and which cards are
// selected, so "this deck" in a conversation with an agent means this one,
// and mirror it into the sidebar so the owner can see what was reported. The
// server treats old focus as possibly a closed tab, hence the heartbeat.

import { useEffect } from "react";
import { api } from "../api/client.ts";
import { useFocus } from "./Focus.tsx";

const HEARTBEAT_MS = 60_000;

export function useReportFocus(deckId: number, deckName: string | null, selected: string[]): void {
  const { setFocus } = useFocus();
  const key = selected.join(",");

  useEffect(() => {
    const ids = key ? key.split(",") : [];
    const report = () => void api.setFocus(deckId, ids).catch(() => {});
    report();
    const beat = window.setInterval(report, HEARTBEAT_MS);
    return () => window.clearInterval(beat);
  }, [deckId, key]);

  useEffect(() => {
    if (deckName != null) setFocus({ deckId, deckName, selectedCount: key ? key.split(",").length : 0 });
  }, [deckId, deckName, key, setFocus]);

  useEffect(
    () => () => {
      setFocus(null);
      void api.setFocus(null).catch(() => {});
    },
    [setFocus],
  );
}
