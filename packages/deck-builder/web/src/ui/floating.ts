// Pure positioning for floating boxes (popovers, menus). No React, no DOM reads
// beyond window.innerWidth/innerHeight.

export type Side = "right" | "below" | "above" | "left";

export interface PlaceOptions {
  prefer?: Side;
  /** Distance between the anchor and the box. */
  gap?: number;
  /** Minimum distance kept from the viewport edge. */
  margin?: number;
  /** Which edge of the anchor the box lines up with along the other axis. */
  align?: "start" | "end";
}

const OPPOSITE: Record<Side, Side> = { right: "left", left: "right", below: "above", above: "below" };

/**
 * Returns the top-left corner for a box of `size` placed beside `anchor`.
 * Uses the preferred side if it fits, otherwise the opposite side if that
 * fits, otherwise whichever has more room; then clamps inside the viewport.
 */
export function placeNear(
  anchor: DOMRect,
  size: { width: number; height: number },
  opts: PlaceOptions = {},
): { left: number; top: number } {
  const { prefer = "below", gap = 4, margin = 8, align = "start" } = opts;
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  const room: Record<Side, number> = {
    right: vw - margin - (anchor.right + gap),
    left: anchor.left - gap - margin,
    below: vh - margin - (anchor.bottom + gap),
    above: anchor.top - gap - margin,
  };
  const need = (s: Side) => (s === "left" || s === "right" ? size.width : size.height);

  let side = prefer;
  if (room[side] < need(side)) {
    const opp = OPPOSITE[side];
    if (room[opp] >= need(opp) || room[opp] > room[side]) side = opp;
  }

  let left: number;
  let top: number;
  if (side === "right" || side === "left") {
    left = side === "right" ? anchor.right + gap : anchor.left - gap - size.width;
    top = align === "end" ? anchor.bottom - size.height : anchor.top;
  } else {
    top = side === "below" ? anchor.bottom + gap : anchor.top - gap - size.height;
    left = align === "end" ? anchor.right - size.width : anchor.left;
  }

  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(v, Math.max(lo, hi)));
  return {
    left: clamp(left, margin, vw - size.width - margin),
    top: clamp(top, margin, vh - size.height - margin),
  };
}
