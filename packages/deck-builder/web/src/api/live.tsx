// Resource-scoped subscriptions. Reconnect always invalidates all reads;
// small bursts share one notification without losing any affected resource.
import type { Change, Resource } from "../../../src/changes.ts";
import { createContext, useCallback, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from "react";
export type LiveScope = Resource | "data" | { resources: Resource[]; deckId?: number };
function createLiveStore() {
  const listeners = new Set<() => void>();
  const versions = new Map<string, number>();
  let reset = 0, data = 0;
  const bump = (key: string) => versions.set(key, (versions.get(key) ?? 0) + 1);
  return {
    subscribe: (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; },
    snapshot(scope: LiveScope) {
      if (scope === "data") return reset + data;
      const { resources, deckId } = typeof scope === "string" ? { resources: [scope], deckId: undefined } : scope;
      return reset + resources.reduce((n, resource) => n + (versions.get(`${resource}:${deckId === undefined ? "*" : "all"}`) ?? 0) +
        (deckId === undefined ? 0 : versions.get(`${resource}:${deckId}`) ?? 0), 0);
    },
    apply(changes: Change[]) {
      const touched = new Set<string>();
      let changedData = false;
      for (const change of changes) {
        if (change.kind === "connected" || (!change.resources && !["agent_contact", "focus"].includes(change.kind))) { reset++; continue; }
        const resources = change.resources ?? [change.kind === "agent_contact" ? "agents" : "focus"];
        const ids = change.deck_ids?.length ? change.deck_ids : change.deck_id === undefined ? [] : [change.deck_id];
        if (resources.some(resource => resource !== "agents" && resource !== "focus")) changedData = true;
        for (const resource of resources) {
          touched.add(`${resource}:*`);
          // Memories can move scopes or inherit global context. Their resource
          // invalidation is global; deck composites and games are addressable.
          if (ids.length && !change.global_resources?.includes(resource) && ["deck", "playtest"].includes(resource)) for (const id of ids) touched.add(`${resource}:${id}`);
          else touched.add(`${resource}:all`);
        }
      }
      if (changedData) data++;
      for (const key of touched) bump(key);
      for (const listener of listeners) listener();
    },
  };
}
const fallback = createLiveStore();
const LiveContext = createContext(fallback);
export function LiveProvider({ children }: { children: ReactNode }) {
  const [store] = useState(createLiveStore);
  useEffect(() => {
    const source = new EventSource("/api/events");
    let pending: Change[] = [], timer: ReturnType<typeof setTimeout> | undefined;
    source.onmessage = event => {
      try { pending.push(JSON.parse(event.data) as Change); } catch { return; }
      timer ??= setTimeout(() => { const batch = pending; pending = []; timer = undefined; store.apply(batch); }, 20);
    };
    return () => { source.close(); if (timer !== undefined) clearTimeout(timer); };
  }, [store]);
  return <LiveContext.Provider value={store}>{children}</LiveContext.Provider>;
}
export function useLive(scope: LiveScope = "data"): number {
  const store = useContext(LiveContext);
  const key = JSON.stringify(scope);
  const snapshot = useCallback(() => store.snapshot(JSON.parse(key) as LiveScope), [store, key]);
  return useSyncExternalStore(store.subscribe, snapshot, snapshot);
}
