import type { SVGAttributes } from "react";

// Simple 16x16 stroke icons. The first six are the prototype's; the rest follow
// the same weight.
const PATHS = {
  inbox: "M2 9l2-6h8l2 6v4H2z M2 9h3l1 2h4l1-2h3",
  decks: "M3 5h10v8H3z M5 2.5h6",
  memories: "M4 2h8v12l-4-3-4 3z",
  activity: "M1 8h3l2-5 3 10 2-5h4",
  profile: "M8 8a3 3 0 100-6 3 3 0 000 6z M2.5 14c.6-3 2.8-4 5.5-4s4.9 1 5.5 4",
  settings: "M2 5h7 M12 5h2 M2 11h2 M7 11h7 M10.5 3.5v3 M5.5 9.5v3",
  back: "M13 8H3 M7 4L3 8l4 4",
  // The left rail itself: a panel with its divider, the chevron says which way it moves.
  sidebarCollapse: "M3 2.5h10a1 1 0 011 1v9a1 1 0 01-1 1H3a1 1 0 01-1-1v-9a1 1 0 011-1z M6 2.5v11 M11 6L9 8l2 2",
  sidebarExpand: "M3 2.5h10a1 1 0 011 1v9a1 1 0 01-1 1H3a1 1 0 01-1-1v-9a1 1 0 011-1z M6 2.5v11 M9 6l2 2-2 2",
  close: "M4 4l8 8 M12 4l-8 8",
  check: "M3 8.5l3.2 3.2L13 4.8",
  plus: "M8 3v10 M3 8h10",
  chevronDown: "M4 6l4 4 4-4",
  // A step, not a dropdown: the shaft keeps these from reading as a chevron.
  arrowUp: "M8 13V3 M4 7l4-4 4 4",
  arrowDown: "M8 3v10 M4 9l4 4 4-4",
  undo: "M5.5 3.5l-3 3 3 3 M2.5 6.5H9a4 4 0 010 8H6",
  redo: "M10.5 3.5l3 3-3 3 M13.5 6.5H7a4 4 0 000 8h3",
  search: "M7 12a5 5 0 100-10 5 5 0 000 10z M11 11l3 3",
  // Three dots: zero-length segments with round caps, drawn thicker so they read.
  more: "M3.5 8h.01 M8 8h.01 M12.5 8h.01",
  // Sharing (spec §18): public is the globe, unlisted the link.
  globe: "M8 14.5a6.5 6.5 0 100-13 6.5 6.5 0 000 13z M1.5 8h13 M8 1.5c1.8 1.8 2.6 4 2.6 6.5S9.8 12.7 8 14.5 M8 1.5C6.2 3.3 5.4 5.5 5.4 8s.8 4.7 2.6 6.5",
  link: "M6.6 9.4a2.6 2.6 0 003.7 0l2.3-2.3a2.6 2.6 0 10-3.7-3.7l-.9.9 M9.4 6.6a2.6 2.6 0 00-3.7 0L3.4 8.9a2.6 2.6 0 103.7 3.7l.9-.9",
  // Delete: a lid with its handle, and a tapered bin with two ribs.
  trash: "M2.5 4.5h11 M6.5 4.5V2.8h3v1.7 M4 4.5l.7 8.7h6.6l.7-8.7 M6.7 7v3.8 M9.3 7v3.8",
  // A limit or gap the reader should know about; drawn beside amber words, never alone.
  warning: "M8 2L14.5 13.5h-13z M8 6.5v3 M8 11.6h.01",
} as const;

export type IconName = keyof typeof PATHS;

export interface IconProps extends Omit<SVGAttributes<SVGSVGElement>, "name"> {
  name: IconName;
  size?: number;
}

export function Icon({ name, size = 14, ...rest }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={name === "more" ? 2.4 : 1.4}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
