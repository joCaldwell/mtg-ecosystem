import { createContext, useContext, useMemo, useRef, type ReactNode } from "react";
import type { CardInfo } from "./CardInfo.ts";
import { createCardCache } from "./cardCache.ts";

type Lookup = (name: string) => Promise<CardInfo | null>;

interface CardLookupApi {
  lookup: Lookup;
  /** Put already-known cards (the open deck's, say) in the cache so hovering them needs no request. */
  prime: (cards: CardInfo[]) => void;
  clear: () => void;
}

// Outside a provider nothing can be resolved: names stay plain text.
const NONE: CardLookupApi = { lookup: async () => null, prime: () => {}, clear: () => {} };

const Ctx = /* @__PURE__ */ createContext<CardLookupApi>(NONE);

/**
 * Wraps `lookup` in an in-memory cache keyed by lowercased name. The promise
 * is what is cached, so several hovers of one card in a row share a request.
 */
export function CardLookupProvider({ lookup, children }: { lookup: Lookup; children: ReactNode }) {
  const latest = useRef(lookup);
  latest.current = lookup;
  const api = useMemo<CardLookupApi>(() => createCardCache(name => latest.current(name)), []);

  return <Ctx.Provider value={api}>{children}</Ctx.Provider>;
}

export function useCardLookup(): CardLookupApi {
  return useContext(Ctx);
}
