import { useLayoutEffect, useRef } from "react";
import { PANEL_QUERY } from "../../app/breakpoints.ts";
import { tokenPx } from "../../ui/index.ts";

/** A short native page scroll precedes the independent list scroll. As the
 * hero leaves, the workspace grows down into precisely the space uncovered;
 * its top moves only through native scrolling. No list offset is rewritten,
 * and column breaks depend on width, so neither rows nor their anchors jump.
 * The shrinking end spacer keeps the page's scroll range constant. */
export function useDeckHeaderScroll() {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const page = ref.current!;
    const hero = page.querySelector<HTMLElement>("[data-deck-hero]")!;
    const compact = page.querySelector<HTMLElement>("[data-deck-compact]")!;
    const list = page.querySelector<HTMLElement>("[data-deck-list]")!;
    const tablet = matchMedia(PANEL_QUERY);
    let travel = 0;
    const update = () => {
      if (tablet.matches) {
        delete page.dataset.compact;
        page.style.removeProperty("--deck-workspace-h");
        page.style.removeProperty("--deck-fold-room");
        hero.inert = false;
        compact.inert = true;
        return;
      }
      travel = Math.max(0, hero.offsetHeight - tokenPx("--art-editor-compact-h"));
      const fold = Math.min(travel, page.scrollTop);
      page.style.setProperty("--deck-workspace-h", `${page.clientHeight - hero.offsetHeight + fold}px`);
      page.style.setProperty("--deck-fold-room", `${travel - fold}px`);
      const folded = fold >= travel - 1;
      page.dataset.compact = String(folded);
      hero.inert = folded;
      compact.inert = !folded;
    };
    // Wheel chaining normally gives the deepest scroller all the input. Use
    // its first pixels for the hero, then pass any remainder to that same list.
    // Upward input returns to the hero only after the list reaches its start.
    const wheel = (event: WheelEvent) => {
      if (tablet.matches || event.ctrlKey || event.deltaMode !== 0 || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
      const forward = event.deltaY > 0 && page.scrollTop < travel;
      const backward = event.deltaY < 0 && list.scrollTop <= 0 && page.scrollTop > 0;
      if (!forward && !backward) return;
      event.preventDefault();
      const before = page.scrollTop;
      page.scrollTop = Math.max(0, Math.min(travel, before + event.deltaY));
      update();
      list.scrollTop += event.deltaY - (page.scrollTop - before);
    };
    // Find/review/keyboard reveals can scroll the list without wheel input.
    // Give them the full working viewport before the next paint as well.
    const listScroll = () => {
      if (!tablet.matches && list.scrollTop > 0 && page.scrollTop < travel) {
        page.scrollTop = travel;
        update();
      }
    };
    update();
    const resize = new ResizeObserver(update);
    resize.observe(page);
    resize.observe(hero);
    page.addEventListener("scroll", update, { passive: true });
    list.addEventListener("scroll", listScroll, { passive: true });
    list.addEventListener("wheel", wheel, { passive: false });
    tablet.addEventListener("change", update);
    return () => {
      resize.disconnect();
      page.removeEventListener("scroll", update);
      list.removeEventListener("scroll", listScroll);
      list.removeEventListener("wheel", wheel);
      tablet.removeEventListener("change", update);
    };
  }, []);
  return ref;
}
