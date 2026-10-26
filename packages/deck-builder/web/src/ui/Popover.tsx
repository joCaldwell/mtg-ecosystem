import { useEffect, useLayoutEffect, useRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { placeNear } from "./floating.ts";
import { cx, dismissOnEscape } from "./internal.ts";
import styles from "./Popover.module.css";

export interface PopoverProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  anchor: HTMLElement | null;
  open: boolean;
  onClose: () => void;
  placement?: "below" | "right" | "above" | "left";
  /** Which edge of the anchor the panel lines up with. */
  align?: "start" | "end";
  children: ReactNode;
}

/** A floating panel next to an anchor element, rendered into document.body. */
export function Popover({
  anchor,
  open,
  onClose,
  placement = "below",
  align = "start",
  children,
  className,
  ...rest
}: PopoverProps) {
  const panel = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const closeRef = useRef(onClose);
  useLayoutEffect(() => {
    closeRef.current = onClose;
  });

  // Measure the panel, place it, and re-place it when the window changes.
  useLayoutEffect(() => {
    if (!open || !anchor) {
      setPos(null);
      return;
    }
    const place = () => {
      const el = panel.current;
      if (!el) return;
      const { width, height } = el.getBoundingClientRect();
      setPos(placeNear(anchor.getBoundingClientRect(), { width, height }, { prefer: placement, align }));
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open, anchor, placement, align]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      // The anchor toggles the popover itself; closing here would reopen it on click.
      if (panel.current?.contains(t) || anchor?.contains(t)) return;
      closeRef.current();
    };
    const releaseEscape = dismissOnEscape(() => closeRef.current());
    document.addEventListener("mousedown", onDown);
    return () => {
      document.removeEventListener("mousedown", onDown);
      releaseEscape();
    };
  }, [open, anchor]);

  if (!open || !anchor) return null;
  return createPortal(
    <div
      ref={panel}
      className={cx(styles.popover, className)}
      style={pos ? { left: pos.left, top: pos.top } : { left: 0, top: 0, opacity: 0 }}
      {...rest}
    >
      {children}
    </div>,
    anchor.closest('[role="dialog"]') ?? document.body,
  );
}
