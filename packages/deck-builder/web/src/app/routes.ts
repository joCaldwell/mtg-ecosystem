// The route table (spec §11.1). Path routes, so a shared deck has a real URL
// whose link preview the server fills in, and every link in the UI is built
// with href(): a path changes here and nowhere else. The server answers these
// paths with index.html and nothing else (D16).

import { useEffect, useLayoutEffect, useState } from "react";

export type PageName = "decks" | "inbox" | "memories" | "profile" | "activity" | "settings" | "design";
/** `review` opens the deck with its pending proposals already drawn into the
 *  list — the link the inbox hands a proposal off with. `proposal` names the
 *  one the inbox had selected, so the review starts at its first decision; the
 *  deck screen reads it back (domain/reviewStart.ts), which keeps the parse
 *  out of the public bundle. */
export type Route =
  | { page: Exclude<PageName, "inbox"> }
  | { page: "inbox"; entry?: string }
  | { page: "deck"; deckId: number; review?: boolean; proposal?: number; importing?: boolean }
  | { page: "playtest"; deckId: number }
  | { page: "shared"; slug: string }
  | { page: "user"; handle: string }
  | { page: "signin" | "privacy" | "terms" }
  | { page: "missing" };

/** A path segment, or null when its escapes are malformed ("%E0%A4%A"). */
function segment(raw: string): string | null {
  try { return decodeURIComponent(raw); } catch { return null; }
}

export function parseRoute(pathname: string, search = ""): Route {
  const path = pathname.replace(/\/+$/, "") || "/";
  if (path === "/") return { page: "decks" };
  const playtest = path.match(/^\/deck\/(\d+)\/playtest$/);
  if (playtest) return { page: "playtest", deckId: Number(playtest[1]) };
  const deck = path.match(/^\/deck\/(\d+)(\/review)?$/);
  if (deck) return { page: "deck", deckId: Number(deck[1]), review: deck[2] != null, ...(new URLSearchParams(search).get("import") === "1" ? { importing: true } : {}) };
  if (path === "/inbox") {
    const entry = new URLSearchParams(search).get("entry");
    return { page: "inbox", ...(entry && /^(proposal|edit)-[1-9]\d*$/.test(entry) ? { entry } : {}) };
  }
  // Both public paths share decoding and malformed-escape handling.
  const shared = path.match(/^\/([du])\/([^/]+)$/);
  if (shared) {
    const value = segment(shared[2]);
    if (value != null) return shared[1] === "d" ? { page: "shared", slug: value } : { page: "user", handle: value };
  }
  const page = path.match(/^\/(decks|inbox|memories|profile|activity|settings|design|signin|privacy|terms)$/)?.[1];
  return { page: (page ?? "missing") as PageName | "signin" | "privacy" | "terms" | "missing" };
}

export function href(route: Route): string {
  switch (route.page) {
    case "decks": return "/";
    // A review and an import never start together; only a review names a proposal.
    case "deck": return `/deck/${route.deckId}${route.review ? "/review" : ""}${route.importing ? "?import=1" : route.review && route.proposal ? `?proposal=${route.proposal}` : ""}`;
    case "inbox": return `/inbox${route.entry ? `?${new URLSearchParams({ entry: route.entry })}` : ""}`;
    case "playtest": return `/deck/${route.deckId}/playtest`;
    case "shared": return `/d/${encodeURIComponent(route.slug)}`;
    case "user": return `/u/${encodeURIComponent(route.handle)}`;
    case "missing": return "/";
    // Activity and Profile are independent pages (review decision, 2026-10-27).
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
  const local = /^\/(?![/\\])/;
  if (!next || !local.test(next) || /[\u0000-\u001f\\]/.test(next)) return "/";
  try {
    const url = new URL(next, "https://same.invalid");
    const path = `${url.pathname}${url.search}${url.hash}`;
    // Checked again after normalisation: "/.//evil" becomes "//evil".
    return url.origin === "https://same.invalid" && local.test(path) ? path : "/";
  } catch { return "/"; }
}

/** The sign-in page, returning to `here` (a path) afterwards. */
export function signInHref(here?: string): string {
  const next = here && safeNext(here) !== "/" ? safeNext(here) : null;
  return next ? `/signin?${new URLSearchParams({ next })}` : "/signin";
}

const CHANGE = "app:navigate";
/** The app's own title, which a shared page restores when it is left. */
export const APP_TITLE = "MTG Deck Builder";

// Phones and shared pages scroll the window, and going back refetches before
// the list exists, so the browser cannot restore the position itself. Each
// history entry has a key; its position is recorded when it is left — by a
// link, or by Back/Forward, when popstate fires while the window still shows
// the page being left — and kept both in memory and in the entry's state
// (for after a reload). useScrollRestore puts it back once the data renders.
let pendingScroll: number | null = null;
const positions = new Map<string, number>();
let currentKey = "";
const newKey = () => Math.random().toString(36).slice(2, 10);
function rememberScroll(): void {
  if (currentKey) positions.set(currentKey, window.scrollY);
  try { history.replaceState({ ...(history.state ?? {}), key: currentKey, scrollY: window.scrollY }, ""); }
  catch { /* a browser's rate limit on replaceState: the in-memory position stands */ }
}

function go(to: string, replace = false): void {
  rememberScroll();
  pendingScroll = null;
  if (to !== location.pathname + location.search + location.hash) {
    if (!replace) currentKey = newKey();
    history[replace ? "replaceState" : "pushState"]({ key: currentKey }, "", to);
  }
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
  if (url.origin !== location.origin || /^\/(api|mcp|\.well-known|assets)(\/|$)/.test(url.pathname)) return;
  // A link to a spot on this same page is the browser's to scroll to.
  if (url.pathname === location.pathname && url.search === location.search && url.hash) return;
  e.preventDefault();
  go(url.pathname + url.search + url.hash);
}

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(() => {
    adoptHashRoute();
    return parseRoute(location.pathname, location.search);
  });
  useEffect(() => {
    if ("scrollRestoration" in history) history.scrollRestoration = "manual";
    currentKey = typeof history.state?.key === "string" ? history.state.key : newKey();
    try { history.replaceState({ ...(history.state ?? {}), key: currentKey }, ""); } catch { /* keep going without */ }
    // Also kept in the entry's state while scrolling (for a reload), at most
    // every 400 ms: Safari refuses more than 100 replaceState calls in 30 s.
    let savedAt = 0;
    const onScroll = () => {
      const now = Date.now();
      if (now - savedAt >= 400) { savedAt = now; rememberScroll(); }
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    const back = () => {
      // The window still shows the page being left: record it under its own key.
      positions.set(currentKey, window.scrollY);
      currentKey = typeof history.state?.key === "string" ? history.state.key : newKey();
      pendingScroll = positions.get(currentKey) ?? (typeof history.state?.scrollY === "number" ? history.state.scrollY : 0);
      update();
    };
    const update = () => setRoute(parseRoute(location.pathname, location.search));

    window.addEventListener("popstate", back);
    window.addEventListener(CHANGE, update);
    document.addEventListener("click", onLinkClick);
    return () => {
      window.removeEventListener("popstate", back);
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener(CHANGE, update);
      document.removeEventListener("click", onLinkClick);
    };
  }, []);
  return route;
}
