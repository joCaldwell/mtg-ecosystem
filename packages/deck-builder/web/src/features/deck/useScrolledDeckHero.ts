import { useEffect, useRef, useState } from "react";
import { PANEL_QUERY, useIsPhone, useMediaQuery } from "../../app/breakpoints.ts";

/** Window-scrolling decks keep their identity after the art leaves the
 * usable viewport. Desktop's native list fold remains independent. */
export function useScrolledDeckHero(reviewing = false) {
  const hero = useRef<HTMLDivElement>(null);
  const [passed, setPassed] = useState(false);
  const narrow = useMediaQuery(PANEL_QUERY);
  const phone = useIsPhone();
  useEffect(() => {
    const node = hero.current;
    if (!narrow || !node) { setPassed(false); return; }
    const navigation = document.querySelector<HTMLElement>("[data-window-navigation]");
    const review = !phone && reviewing ? document.querySelector<HTMLElement>("[data-tablet-review]") : null;
    let inset = 0;
    let observer: IntersectionObserver;
    const update = () => setPassed(node.getBoundingClientRect().bottom <= inset);
    // Measure real controls, including wrapping/zoom, rather than parsing
    // unresolved calc()/max() token expressions as pixel numbers.
    const measure = () => {
      inset = (navigation?.getBoundingClientRect().height ?? 0) + (review?.getBoundingClientRect().height ?? 0);
      observer?.disconnect();
      observer = new IntersectionObserver(update, { rootMargin: `-${inset}px 0px 0px 0px` });
      observer.observe(node);
      update();
    };
    const resize = new ResizeObserver(measure);
    if (navigation) resize.observe(navigation);
    if (review) resize.observe(review);
    measure();
    return () => { observer.disconnect(); resize.disconnect(); };
  }, [narrow, phone, reviewing]);
  return { hero, passed };
}
