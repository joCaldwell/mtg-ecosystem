import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { cx } from "./internal.ts";
import styles from "./ResizeHandle.module.css";

export interface ResizeHandleProps {
  /** What dragging does, for assistive tech: "Resize the deck panel". */
  label: string;
  /** Which edge of the element it resizes the handle sits on. Dragging outward widens. */
  edge: "left" | "right";
  /** The chosen width and its limits, in pixels. */
  value: number;
  min: number;
  max: number;
  onChange: (width: number) => void;
  /** Double-click: go back to the default width. */
  onReset?: () => void;
  className?: string;
}

// One arrow-key press.
const STEP = 16;

/**
 * A draggable edge. Render it inside the element it resizes, which must be
 * positioned: the handle pins itself to that element's edge and measures it,
 * so a drag starts from the width on screen even where CSS has overridden
 * `value` (a narrow window, say). Arrow keys move it too; Home and End go to
 * the limits.
 */
export function ResizeHandle({ label, edge, value, min, max, onChange, onReset, className }: ResizeHandleProps) {
  const drag = useRef<{ x: number; width: number } | null>(null);
  const [dragging, setDragging] = useState(false);

  const clamp = (width: number) => Math.round(Math.max(min, Math.min(width, max)));
  const shown = (el: HTMLElement) => el.parentElement?.getBoundingClientRect().width ?? value;
  // Toward the handle's own edge is outward, and outward is wider.
  const outward = edge === "left" ? -1 : 1;

  function onPointerDown(e: PointerEvent<HTMLDivElement>) {
    if (e.button !== 0) return;
    e.preventDefault(); // or the drag selects the text it passes over
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, width: shown(e.currentTarget) };
    setDragging(true);
  }

  function onPointerMove(e: PointerEvent<HTMLDivElement>) {
    if (!drag.current) return;
    onChange(clamp(drag.current.width + (e.clientX - drag.current.x) * outward));
  }

  function stop() {
    drag.current = null;
    setDragging(false);
  }

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    let next: number;
    if (e.key === "ArrowLeft") next = shown(e.currentTarget) - STEP * outward;
    else if (e.key === "ArrowRight") next = shown(e.currentTarget) + STEP * outward;
    else if (e.key === "Home") next = min;
    else if (e.key === "End") next = max;
    else return;
    e.preventDefault();
    onChange(clamp(next));
  }

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={clamp(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      title={onReset ? "Drag to resize. Double-click to reset." : "Drag to resize"}
      className={cx(styles.handle, styles[edge], dragging && styles.dragging, className)}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={stop}
      onPointerCancel={stop}
      onLostPointerCapture={stop}
      onDoubleClick={onReset}
      onKeyDown={onKeyDown}
    />
  );
}
