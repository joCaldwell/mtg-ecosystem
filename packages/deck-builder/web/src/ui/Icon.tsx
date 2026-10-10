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
  close: "M4 4l8 8 M12 4l-8 8",
  check: "M3 8.5l3.2 3.2L13 4.8",
  plus: "M8 3v10 M3 8h10",
  chevronDown: "M4 6l4 4 4-4",
  undo: "M5.5 3.5l-3 3 3 3 M2.5 6.5H9a4 4 0 010 8H6",
  search: "M7 12a5 5 0 100-10 5 5 0 000 10z M11 11l3 3",
  // Three dots: zero-length segments with round caps, drawn thicker so they read.
  more: "M3.5 8h.01 M8 8h.01 M12.5 8h.01",
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
