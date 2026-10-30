// Server reads. A query refetches when its deps change, when `reload` is
// called, and whenever the server's event stream reports a change — which is
// what makes an agent's write appear here without a reload (spec §11).

import { useCallback, useEffect, useState } from "react";
import { useLive, type LiveScope } from "./live.tsx";

export interface Query<T> {
  /** The last successful result. Kept while a refetch is in flight, so a live
   *  update never blanks the screen. */
  data: T | null;
  error: string | null;
  /** True only until the first result or error arrives. */
  loading: boolean;
  reload: () => void;
}

export function useQuery<T>(load: () => Promise<T>, deps: unknown[], scope: LiveScope = "data"): Query<T> {
  const live = useLive(scope);
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let current = true;
    load()
      .then((value) => {
        if (!current) return;
        setData(value);
        setError(null);
      })
      .catch((e: unknown) => current && setError(e instanceof Error ? e.message : String(e)))
      .finally(() => current && setLoading(false));
    return () => {
      current = false;
    };
    // The caller's deps are the contract; `load` is an inline arrow.
  }, [...deps, live, tick]);

  const reload = useCallback(() => setTick((t) => t + 1), []);
  return { data, error, loading, reload };
}
