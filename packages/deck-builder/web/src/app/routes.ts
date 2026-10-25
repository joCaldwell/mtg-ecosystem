// The route table (spec §11.1). Path routes, so a shared deck has a real URL
// whose link preview the server fills in, and every link in the UI is built
// with href(): a path changes here and nowhere else. The server answers these
// paths with index.html and nothing else (D16).

import { useEffect, useState } from "react";

export type PageName = "decks" | "inbox" | "memories" | "profile" | "activity" | "settings" | "design";
/** `review` opens the deck with its pending proposals already drawn into the
 *  list — the link the inbox hands a proposal off with. */
export type Route =
  | { page: PageName }
  | { page: "deck"; deckId: number; review?: boolean }
  | { page: "playtest"; deckId: number }
  | { page: "shared"; slug: string }
  | { page: "user"; handle: string }
  | { page: "signin" | "privacy" | "terms" }
  | { page: "missing" };

const PAGES: readonly PageName[] = ["decks", "inbox", "memories", "profile", "activity", "settings", "design"];

export function parseRoute(pathname: string): Route {
  const path = pathname.replace(/\/+$/, "") || "/";
  if (path === "/") return { page: "decks" };
  const playtest = path.match(/^\/deck\/(\d+)\/playtest$/);
  if (playtest) return { page: "playtest", deckId: Number(playtest[1]) };
  const deck = path.match(/^\/deck\/(\d+)(\/review)?$/);
  if (deck) return { page: "deck", deckId: Number(deck[1]), review: deck[2] != null };
  const shared = path.match(/^\/d\/([^/]+)$/);
  if (shared) return { page: "shared", slug: decodeURIComponent(shared[1]) };
  const user = path.match(/^\/u\/([^/]+)$/);
  if (user) return { page: "user", handle: decodeURIComponent(user[1]) };
  const page = path.match(/^\/([a-z]+)$/)?.[1];
  if (page === "signin" || page === "privacy" || page === "terms") return { page };
  return page && (PAGES as readonly string[]).includes(page) ? { page: page as PageName } : { page: "missing" };
}

export function href(route: Route): string {
  switch (route.page) {
    case "decks": return "/";
    case "deck": return `/deck/${route.deckId}${route.review ? "/review" : ""}`;
    case "playtest": return `/deck/${route.deckId}/playtest`;
    case "shared": return `/d/${encodeURIComponent(route.slug)}`;
    case "user": return `/u/${encodeURIComponent(route.handle)}`;
    case "missing": return "/";
    default: return `/${route.page}`;
  }
}

/** Shared pages render without the signed-in app around them (§18.5). */
export function isPublicRoute(route: Route): boolean {
  return route.page === "shared" || route.page === "user";
}

const CHANGE = "app:navigate";

export function navigate(route: Route, opts: { replace?: boolean } = {}): void {
  const to = href(route);
  if (to !== location.pathname) history[opts.replace ? "replaceState" : "pushState"](null, "", to);
  window.dispatchEvent(new Event(CHANGE));
  window.scrollTo?.(0, 0);
}

/** Bookmarks from the hash-route era (`/#/deck/5`) land on their path once. */
function adoptHashRoute(): void {
  const legacy = location.hash.match(/^#(\/.*)$/)?.[1];
  if (legacy) history.replaceState(null, "", legacy === "/" ? "/" : legacy);
}

// Same-origin links navigate in place. A modified click, a new tab, a
// download or a link to another origin is left to the browser.
function onLinkClick(e: MouseEvent): void {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
  if (!a || (a.target && a.target !== "_self") || a.hasAttribute("download")) return;
  const url = new URL(a.href, location.href);
  if (url.origin !== location.origin || url.pathname.startsWith("/api/")) return;
  e.preventDefault();
  if (url.pathname !== location.pathname) history.pushState(null, "", url.pathname + url.search);
  window.dispatchEvent(new Event(CHANGE));
  window.scrollTo?.(0, 0);
}

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(() => {
    adoptHashRoute();
    return parseRoute(location.pathname);
  });
  useEffect(() => {
    const update = () => setRoute(parseRoute(location.pathname));
    window.addEventListener("popstate", update);
    window.addEventListener(CHANGE, update);
    document.addEventListener("click", onLinkClick);
    return () => {
      window.removeEventListener("popstate", update);
      window.removeEventListener(CHANGE, update);
      document.removeEventListener("click", onLinkClick);
    };
  }, []);
  return route;
}
