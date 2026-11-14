import { useLayoutEffect, useRef } from "react";
import { PANEL_QUERY } from "../../app/breakpoints.ts";
import { tokenPx } from "../../ui/index.ts";

/** The list is the native scroller for the entire gesture. Its first pixels
 * move the hero; a matching content translation cancels those pixels inside
 * the list, whose viewport grows by the same amount. Past the fold only the
 * list moves. No wheel interception, device guessing or animation clock. */
export function useDeckHeaderScroll() {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const page = ref.current!;
    const hero = page.querySelector<HTMLElement>("[data-deck-hero]")!;
    const compact = page.querySelector<HTMLElement>("[data-deck-compact]")!;
    const list = page.querySelector<HTMLElement>("[data-deck-list]")!;
    const tablet = matchMedia(PANEL_QUERY);
    let travel = 0;
    let height = 0;
    let heroHeight = 0;
    let compactHeight = 0;
    let fold = 0;
    let painted = false;
    const paint = (force = false) => {
      const next = Math.min(travel, list.scrollTop);
      if (!force && painted && next === fold) return;
      painted = true;
      fold = next;
      page.style.setProperty("--deck-workspace-h", `${height - heroHeight + fold}px`);
      page.style.setProperty("--deck-fold-room", `${travel - fold}px`);
      page.style.setProperty("--deck-fold", `${fold}px`);
      // Crossfade in the last compact-bar height of travel. Scroll-linked
      // opacity has no independent easing or reduced-motion animation.
      const mix = Math.max(0, Math.min(1, (fold - travel + compactHeight) / compactHeight));
      page.style.setProperty("--deck-compact-opacity", String(mix));
      const folded = fold >= travel - 1;
      page.dataset.compact = String(folded);
      hero.inert = folded;
      compact.inert = !folded;
      hero.setAttribute("aria-hidden", String(folded));
      compact.setAttribute("aria-hidden", String(!folded));
      page.scrollTop = fold;
    };
    const measure = () => {
      if (tablet.matches) {
        painted = false;
        delete page.dataset.compact;
        for (const property of ["--deck-workspace-h", "--deck-fold-room", "--deck-fold", "--deck-compact-opacity"]) page.style.removeProperty(property);
        hero.inert = false;
        compact.inert = true;
        hero.removeAttribute("aria-hidden");
        compact.setAttribute("aria-hidden", "true");
        return;
      }
      heroHeight = hero.offsetHeight;
      height = page.clientHeight;
      compactHeight = tokenPx("--art-editor-compact-h");
      travel = Math.max(0, heroHeight - compactHeight);
      // Leave the native list offset alone on resize. Rewriting it during
      // a wheel animation can make the browser finish its target in one frame.
      paint(true);
    };
    const reveal = () => {
      if (tablet.matches) return;
      list.scrollTop = Math.max(travel, list.scrollTop);
      paint();
    };
    const listScroll = () => { if (!tablet.matches) paint(); };
    // Wheel/focus on the hero or toolbar can still move the short outer
    // scroller. Reconcile that input into the same list coordinate system.
    const pageScroll = () => {
      if (tablet.matches || Math.abs(page.scrollTop - fold) < 1) return;
      // Filter restore and native focus can set both offsets in one turn.
      // If they already agree, do not apply that outer delta a second time.
      if (Math.abs(page.scrollTop - Math.min(travel, list.scrollTop)) >= 1) {
        list.scrollTop += page.scrollTop - fold;
      }
      paint(true);
    };
    measure();
    const resize = new ResizeObserver(measure);
    resize.observe(page);
    resize.observe(hero);
    page.addEventListener("scroll", pageScroll, { passive: true });
    list.addEventListener("scroll", listScroll, { passive: true });
    list.addEventListener("deck-workspace-reveal", reveal);
    tablet.addEventListener("change", measure);
    return () => {
      resize.disconnect();
      page.removeEventListener("scroll", pageScroll);
      list.removeEventListener("scroll", listScroll);
      list.removeEventListener("deck-workspace-reveal", reveal);
      tablet.removeEventListener("change", measure);
    };
  }, []);
  return ref;
}
