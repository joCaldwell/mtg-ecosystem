import { createContext, useContext, useMemo, useRef, type ReactNode } from "react";
import type { CardInfo } from "./CardInfo.ts";

type Lookup = (name: string) => Promise<CardInfo | null>;

interface CardLookupApi {
  lookup: Lookup;
  /** Put already-known cards (the open deck's, say) in the cache so hovering them needs no request. */
  prime: (cards: CardInfo[]) => void;
  clear: () => void;
}

const key = (name: string) => name.trim().toLowerCase();

// Outside a provider nothing can be resolved: names stay plain text.
const NONE: CardLookupApi = { lookup: async () => null, prime: () => {}, clear: () => {} };

const Ctx = createContext<CardLookupApi>(NONE);

/**
 * Wraps `lookup` in an in-memory cache keyed by lowercased name. The promise
 * is what is cached, so several hovers of one card in a row share a request.
 */
export function CardLookupProvider({ lookup, children }: { lookup: Lookup; children: ReactNode }) {
  const cache = useRef(new Map<string, Promise<CardInfo | null>>());
  const latest = useRef(lookup);
  latest.current = lookup;

  const api = useMemo<CardLookupApi>(
    () => ({
      clear: () => cache.current.clear(),
      lookup: (name) => {
        const k = key(name);
        let hit = cache.current.get(k);
        if (!hit) {
          hit = latest.current(name).catch(() => {
            // A failed request must not poison the cache: forget it so the next hover retries.
            cache.current.delete(k);
            return null;
          });
          cache.current.set(k, hit);
        }
        return hit;
      },
      prime: (cards) => {
        for (const card of cards) {
          const done = Promise.resolve(card);
          cache.current.set(key(card.name), done);
          // Prose usually names a two-faced card by one face, so index those too.
          if (card.name.includes(" // ")) {
            for (const face of card.name.split(" // ")) {
              if (!cache.current.has(key(face))) cache.current.set(key(face), done);
            }
          }
        }
      },
    }),
    [],
  );

  return <Ctx.Provider value={api}>{children}</Ctx.Provider>;
}

export function useCardLookup(): CardLookupApi {
  return useContext(Ctx);
}
