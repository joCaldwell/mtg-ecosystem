// Every dialog here shows a failed request next to the control that caused
// it. The deck store's run() reports errors in the page banner behind the
// dialog, where they would go unseen, so the dialogs use this instead.

import { useCallback, useState } from "react";
import type { Envelope } from "../../../api/client.ts";
import { useDeck } from "../DeckStore.tsx";

export function useAttempt() {
  const { apply } = useDeck();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  /** Run a request; resolves null (and sets `error`) when it fails. */
  const attempt = useCallback(async <T>(fn: () => Promise<T>): Promise<T | null> => {
    setError(null);
    setBusy(true);
    try {
      return await fn();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
      return null;
    } finally {
      setBusy(false);
    }
  }, []);

  /** Like attempt, and lands the returned deck state in the store. */
  const mutate = useCallback(
    async <T extends Envelope>(fn: () => Promise<T>): Promise<T | null> => {
      const result = await attempt(fn);
      if (result) apply(result);
      return result;
    },
    [attempt, apply],
  );

  return { error, setError, busy, attempt, mutate };
}
