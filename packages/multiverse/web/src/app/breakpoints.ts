// The one breakpoint the phone view and the public pages share (spec §11.2,
// plan D17): below 720 px the signed-in app is the read-and-rule phone shell
// and shared pages go single-column. CSS media queries cannot read a custom
// property, so module CSS repeats this number as `@media (max-width: 719px)`;
// test/web-architecture.test.ts holds every such query to it.

import { useEffect, useState } from "react";

/** The widest a viewport can be and still be a phone. */
export const PHONE_MAX_WIDTH = 719;
/** Full navigation becomes a drawer below this width. */
export const DRAWER_BREAKPOINT = 960;
/** The deck panel becomes a sheet below this width. */
export const PANEL_BREAKPOINT = 1100;
export const DRAWER_QUERY = `(max-width: ${DRAWER_BREAKPOINT - 1}px)`;
export const PANEL_QUERY = `(max-width: ${PANEL_BREAKPOINT - 1}px)`;
export const PHONE_QUERY = `(max-width: ${PHONE_MAX_WIDTH}px)`;

function matches(query: string): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia(query).matches;
}

export function useMediaQuery(query: string): boolean {
  const [value, setValue] = useState(() => matches(query));
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const list = window.matchMedia(query);
    const update = () => setValue(list.matches);
    update();
    list.addEventListener("change", update);
    return () => list.removeEventListener("change", update);
  }, [query]);
  return value;
}

/** True below 720 px. */
export function useIsPhone(): boolean {
  return useMediaQuery(PHONE_QUERY);
}
