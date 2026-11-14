import { useEffect, useRef } from "react";
import { APP_TITLE, isPublicRoute, type Route } from "./routes.ts";

/** Query/selection changes within a screen do not constitute a new page. */
export function routeIdentity(route: Route): string {
  if (route.page === "deck" || route.page === "playtest") return `${route.page}:${route.deckId}`;
  if (route.page === "live-game") return `live-game:${route.gameId}`;
  if (route.page === "shared") return `shared:${route.slug}`;
  if (route.page === "user") return `user:${route.handle}`;
  return route.page;
}

/** SPA navigation needs the title and reading position a document load gives
 * for free. Wait for lazy/data-backed headings, then focus once; subsequent
 * live updates may change the title but must never take focus from editing.
 * WCAG: https://www.w3.org/WAI/WCAG22/Understanding/focus-order.html */
export function useRouteAccessibility(route: Route): void {
  const identity = routeIdentity(route);
  const previous = useRef(identity);
  const publicPage = isPublicRoute(route);
  useEffect(() => {
    let focusPending = previous.current !== identity;
    previous.current = identity;
    const update = () => {
      const main = document.getElementById("main-content");
      const heading = [...main?.querySelectorAll<HTMLElement>("h1") ?? []]
        .find(node => !node.closest('dialog, [role=dialog], [inert], [hidden], [aria-hidden="true"]') && node.getClientRects().length);
      // A hidden retained panel or a modal's heading is not the page title.
      if (!heading) return;
      if (!publicPage) {
        const title = heading.textContent?.replace(/\s+/g, " ").trim();
        if (title) document.title = `${title} · ${APP_TITLE}`;
      }
      if (!focusPending || document.querySelector("dialog:modal, [aria-modal=true]")) return;
      heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
      if (document.activeElement === heading) focusPending = false;
    };
    // Navigation can suspend or wait on a server read; observing actual content
    // avoids guessing with a timer and works when the tab is in the background.
    const observer = new MutationObserver(update);
    observer.observe(document.getElementById("root") ?? document.body, {
      childList: true, subtree: true, characterData: true,
      attributes: true, attributeFilter: ["inert", "hidden", "aria-hidden"],
    });
    update();
    return () => observer.disconnect();
  }, [identity, publicPage]);
}
