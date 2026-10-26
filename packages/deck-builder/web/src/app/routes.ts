// The route table (spec §11.1). Path routes, so a shared deck has a real URL
// whose link preview the server fills in, and every link in the UI is built
// with href(): a path changes here and nowhere else. The server answers these
// paths with index.html and nothing else (D16).

import { useEffect, useLayoutEffect, useState } from "react";

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

/** A path segment, or null when its escapes are malformed ("%E0%A4%A"). */
function segment(raw: string): string | null {
  try { return decodeURIComponent(raw); } catch { return null; }
}

export function parseRoute(pathname: string): Route {
  const path = pathname.replace(/\/+$/, "") || "/";
  if (path === "/") return { page: "decks" };
  const playtest = path.match(/^\/deck\/(\d+)\/playtest$/);
  if (playtest) return { page: "playtest", deckId: Number(playtest[1]) };
  const deck = path.match(/^\/deck\/(\d+)(\/review)?$/);
  if (deck) return { page: "deck", deckId: Number(deck[1]), review: deck[2] != null };
  const shared = path.match(/^\/d\/([^/]+)$/);
  if (shared) { const slug = segment(shared[1]); return slug == null ? { page: "missing" } : { page: "shared", slug }; }
  const user = path.match(/^\/u\/([^/]+)$/);
  if (user) { const handle = segment(user[1]); return handle == null ? { page: "missing" } : { page: "user", handle }; }
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

/** Where sign-in returns to: a same-origin path only (one leading "/", no
 *  scheme, no "//" or "/\\" host), else the deck list. Never an open redirect. */
export function safeNext(next: string | null | undefined): string {
  if (!next || !/^\/(?![/\\])/.test(next) || /[\u0000-\u001f]/.test(next)) return "/";
  try {
    const url = new URL(next, "https://same.invalid");
    return url.origin === "https://same.invalid" ? `${url.pathname}${url.search}${url.hash}` : "/";
  } catch { return "/"; }
}

/** The sign-in page, returning to `here` (a path) afterwards. */
export function signInHref(here?: string): string {
  const next = here && safeNext(here) !== "/" ? safeNext(here) : null;
  return next ? `/signin?${new URLSearchParams({ next })}` : "/signin";
}

const CHANGE = "app:navigate";

// Phones and shared pages scroll the window, and going back refetches before
// the list exists, so the browser cannot restore the position itself. Each
// history entry keeps its own scrollY; useScrollRestore puts it back once
// the screen's data has rendered.
let pendingScroll: number | null = null;
function rememberScroll(): void {
  history.replaceState({ ...(history.state ?? {}), scrollY: window.scrollY }, "");
}

function go(to: string, replace = false): void {
  rememberScroll();
  pendingScroll = null;
  if (to !== location.pathname + location.search + location.hash) history[replace ? "replaceState" : "pushState"](null, "", to);
  window.dispatchEvent(new Event(CHANGE));
  window.scrollTo?.(0, 0);
}

export function navigate(route: Route, opts: { replace?: boolean } = {}): void {
  go(href(route), opts.replace);
}

/** Call with true once the screen's data is on the page (review S13). */
export function useScrollRestore(ready: boolean): void {
  // A layout effect: the rows are in the DOM and nothing has painted at the
  // top yet. (No requestAnimationFrame: it never fires in a background tab.)
  useLayoutEffect(() => {
    if (!ready || pendingScroll == null) return;
    const y = pendingScroll;
    pendingScroll = null;
    window.scrollTo(0, y);
  }, [ready]);
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
  // Only the app's own pages; the API, MCP, metadata and built files belong to the server.
  if (url.origin !== location.origin || /^\/(api|mcp|\.well-known|assets)(\/|$)/.test(url.pathname) || /^\/mcp/.test(url.pathname)) return;
  // A link to a spot on this same page is the browser's to scroll to.
  if (url.pathname === location.pathname && url.search === location.search && url.hash) return;
  e.preventDefault();
  go(url.pathname + url.search + url.hash);
}

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(() => {
    adoptHashRoute();
    return parseRoute(location.pathname);
  });
  useEffect(() => {
    if ("scrollRestoration" in history) history.scrollRestoration = "manual";
    const update = () => setRoute(parseRoute(location.pathname));
    const back = () => { pendingScroll = typeof history.state?.scrollY === "number" ? history.state.scrollY : 0; update(); };
    window.addEventListener("popstate", back);
    window.addEventListener(CHANGE, update);
    document.addEventListener("click", onLinkClick);
    return () => {
      window.removeEventListener("popstate", back);
      window.removeEventListener(CHANGE, update);
      document.removeEventListener("click", onLinkClick);
    };
  }, []);
  return route;
}
