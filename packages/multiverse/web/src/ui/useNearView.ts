import { useEffect, useRef, useState } from "react";

/** Whether an element has come within `margin` of being seen, measured in
 * the nearest container that actually scrolls (a sidebar, a pane) or else the
 * window. It stays true once seen, so a picture is requested at most once. */
export function useNearView<T extends Element>(margin = "200px") {
  const ref = useRef<T>(null);
  const [near, setNear] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (near || !el) return;
    if (typeof IntersectionObserver !== "function") return setNear(true);
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) { setNear(true); io.disconnect(); }
    }, { root: scroller(el), rootMargin: margin });
    io.observe(el);
    return () => io.disconnect();
  }, [near, margin]);
  return [ref, near] as const;
}

// A rootMargin only extends the root itself, so the root must be the box that
// clips the element; an overflow:auto box that does not overflow clips nothing.
function scroller(el: Element): Element | null {
  for (let p = el.parentElement; p && p !== document.body; p = p.parentElement)
    if (/auto|scroll/.test(getComputedStyle(p).overflowY) && p.scrollHeight > p.clientHeight) return p;
  return null;
}
