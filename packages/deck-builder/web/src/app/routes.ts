// The route table. Hash routes, because the app is one static page served
// from localhost and a deck link has to survive a reload with no server
// routing. Every link in the UI is built with href(), so a path changes here
// and nowhere else.

import { useEffect, useState } from "react";

export type PageName = "decks" | "inbox" | "memories" | "profile" | "activity" | "settings" | "design";
/** `review` opens the deck with its pending proposals already drawn into the
 *  list — the link the inbox hands a proposal off with. */
export type Route = { page: "playtest"; deckId: number } | { page: PageName } | { page: "deck"; deckId: number; review?: boolean };

const PAGES: readonly PageName[] = ["inbox", "memories", "profile", "activity", "settings", "design"];

export function parseRoute(hash: string): Route {
  const playtest = hash.match(/^#\/deck\/(\d+)\/playtest$/);
  if (playtest) return { page: "playtest", deckId: Number(playtest[1]) };
  const deck = hash.match(/^#\/deck\/(\d+)(\/review)?$/);
  if (deck) return { page: "deck", deckId: Number(deck[1]), review: deck[2] != null };
  const page = hash.match(/^#\/([a-z]+)$/)?.[1] as PageName | undefined;
  return { page: page && PAGES.includes(page) ? page : "decks" };
}

export function href(route: Route): string {
  if (route.page === "playtest") return `#/deck/${route.deckId}/playtest`;
  if (route.page === "deck") return `#/deck/${route.deckId}${route.review ? "/review" : ""}`;
  return route.page === "decks" ? "#/" : `#/${route.page}`;
}

export function navigate(route: Route): void {
  window.location.hash = href(route);
}

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.hash));
  useEffect(() => {
    const onHash = () => setRoute(parseRoute(window.location.hash));
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  return route;
}
