// The desktop app scrolls inside its panes, so base.css pins the body and
// hides its overflow. Phones and shared pages scroll the window instead
// (routes.ts keeps their positions), and a pinned body cannot be scrolled by
// touch or wheel at all. Their shells mark the document while mounted, and
// base.css releases the body for as long as the mark is there.

import { useLayoutEffect } from "react";

let holders = 0;

export function useWindowScroll(): void {
  useLayoutEffect(() => {
    holders++;
    document.documentElement.dataset.scroll = "window";
    return () => {
      if (--holders === 0) delete document.documentElement.dataset.scroll;
    };
  }, []);
}
