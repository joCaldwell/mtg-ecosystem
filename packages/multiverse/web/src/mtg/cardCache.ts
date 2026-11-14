import type { CardInfo } from "./CardInfo.ts";

export type CardLoader = (name: string) => Promise<CardInfo | null>;
const key = (name: string) => name.trim().toLowerCase();

/** Share pending lookups without allowing a failed, invalidated request to
 * evict newer facts primed by a deck read or a subsequent lookup. */
export function createCardCache(load: CardLoader) {
  const cache = new Map<string, Promise<CardInfo | null>>();
  return {
    clear: () => cache.clear(),
    lookup(name: string): Promise<CardInfo | null> {
      const k = key(name);
      const hit = cache.get(k);
      if (hit) return hit;
      const pending = Promise.resolve().then(() => load(name)).catch(() => {
        if (cache.get(k) === pending) cache.delete(k);
        return null;
      });
      cache.set(k, pending);
      return pending;
    },
    prime(cards: CardInfo[]): void {
      for (const card of cards) {
        const done = Promise.resolve(card);
        cache.set(key(card.name), done);
        if (card.name.includes(" // ")) {
          for (const face of card.name.split(" // ")) {
            if (!cache.has(key(face))) cache.set(key(face), done);
          }
        }
      }
    },
  };
}
