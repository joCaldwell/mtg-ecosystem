import { useState, type UIEvent } from "react";
import { tokenPx } from "../ui/index.ts";
/** Observe only desktop list scrolling; narrower banners leave with the window.
 * The existing shells remain the sole owners of the window-scroll contract. */
export function useArtCollapse() {
  const [collapsed, setCollapsed] = useState(false);
  return { collapsed, onScrollCapture: (event: UIEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    if (target.dataset.deckList !== "true") return;
    const threshold = tokenPx("--art-collapse-threshold");
    if (target.scrollTop <= threshold) setCollapsed(false);
    // Folding gives the pane more room. A short list could then clamp to
    // zero and immediately unfold; keep its short banner stable instead.
    else if (target.scrollHeight - target.clientHeight > threshold + tokenPx("--art-banner-space")) setCollapsed(true);
  } };
}
