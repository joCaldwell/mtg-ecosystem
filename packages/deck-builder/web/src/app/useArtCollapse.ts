import { useEffect, useRef, useState, type UIEvent } from "react";
import { nextFold } from "../domain/artFold.ts";
import { tokenPx } from "../ui/index.ts";
import { PANEL_QUERY, useMediaQuery } from "./breakpoints.ts";

/** Fold the deck banner while the desktop list is scrolled. Only the list's
 * own scroll events count; under 1100 px the list scrolls with the window, the
 * banner leaves with it, and a fold left over from a wider window is dropped.
 * The shells remain the sole owners of the window-scroll contract. */
export function useArtCollapse() {
  const desktop = !useMediaQuery(PANEL_QUERY);
  const [folded, setFolded] = useState(false);
  // Tokens do not change while the page is open; read them on first use.
  const space = useRef<{ threshold: number; band: number } | null>(null);
  useEffect(() => { if (!desktop) setFolded(false); }, [desktop]);
  return {
    collapsed: desktop && folded,
    onScrollCapture: (event: UIEvent<HTMLElement>) => {
      const list = event.target as HTMLElement;
      if (!desktop || list.dataset.deckList !== "true") return;
      space.current ??= { threshold: tokenPx("--art-collapse-threshold"), band: tokenPx("--art-banner-space") };
      setFolded((f) => nextFold(f, list, space.current!));
    },
  };
}
