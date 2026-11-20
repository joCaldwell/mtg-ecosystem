import { useEffect, useId, useLayoutEffect, useRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { placeNear } from "./floating.ts";
import { cx, dismissOnEscape } from "./internal.ts";
import styles from "./Popover.module.css";

export interface PopoverProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  anchor: HTMLElement | null;
  open: boolean;
  onClose: (reason?: "outside" | "escape") => void;
  placement?: "below" | "right" | "above" | "left";
  /** Which edge of the anchor the panel lines up with. */
  align?: "start" | "end";
  /** Never narrower than the anchor, as a dropdown under its trigger. */
  fitAnchor?: boolean;
  /** Use the app's outside-press and Escape handling even in the native top layer. */
  manualDismiss?: boolean;
  children: ReactNode;
}

/** A floating panel next to an anchor element, rendered into document.body. */
export function Popover({
  anchor,
  open,
  onClose,
  placement = "below",
  align = "start",
  fitAnchor = false,
  manualDismiss = false,
  children,
  className,
  onKeyDown,
  ...rest
}: PopoverProps) {
  const panel = useRef<HTMLDivElement>(null);
  const anchorName = `--popover-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const nativePopover = typeof HTMLElement !== "undefined" && Object.hasOwn(HTMLElement.prototype, "popover");
  // Early implementations lack source association. Native light dismiss also
  // treats a button's <label> as outside its source, dismissing before the
  // label forwards its click and reopening the controlled dropdown. Retain
  // manual dismissal for labelled controls while still using the top layer.
  const labelledAnchor = typeof HTMLButtonElement !== "undefined" && anchor instanceof HTMLButtonElement && !!anchor.labels?.length;
  const nativeDismiss = nativePopover && !manualDismiss && !labelledAnchor && typeof ToggleEvent !== "undefined" && "source" in ToggleEvent.prototype;
  const anchored = nativePopover && typeof CSS !== "undefined" && CSS.supports("position-area: block-end");
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const closeRef = useRef(onClose);
  useLayoutEffect(() => {
    closeRef.current = onClose;
  });

  useLayoutEffect(() => {
    if (!open || !anchor || !panel.current) { setPos(null); return; }
    const el = panel.current;
    const previousAnchor = anchor.style.getPropertyValue("anchor-name");
    if (anchored) anchor.style.setProperty("anchor-name", [previousAnchor, anchorName].filter(Boolean).join(", "));
    // beforetoggle keeps controlled state synchronized before another input
    // event can reopen a natively dismissed panel; toggle is queued too late.
    const onToggle = (event: Event) => {
      if ((event as ToggleEvent).newState !== "closed") return;
      const focused = document.activeElement;
      closeRef.current(focused === anchor || (focused instanceof Node && el.contains(focused)) ? "escape" : "outside");
    };
    el.addEventListener("beforetoggle", onToggle);
    if (nativePopover) (el.showPopover as (options: { source: HTMLElement }) => void)({ source: anchor });
    const place = () => {
      if (anchored) return;
      const { width, height } = el.getBoundingClientRect();
      setPos(placeNear(anchor.getBoundingClientRect(), { width, height }, { prefer: placement, align }));
    };
    place();
    // ResizeObserver also covers async content, zoom and anchor text changes.
    const observer = new ResizeObserver(place);
    observer.observe(el);
    observer.observe(anchor);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      el.removeEventListener("beforetoggle", onToggle);
      observer.disconnect();
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      if (nativePopover && el.matches(":popover-open")) el.hidePopover();
      if (anchored) {
        if (previousAnchor) anchor.style.setProperty("anchor-name", previousAnchor);
        else anchor.style.removeProperty("anchor-name");
      }
    };
  }, [open, anchor, placement, align, anchored, nativePopover, anchorName]);

  useEffect(() => {
    if (!open || nativeDismiss) return;
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      // The anchor toggles the popover itself; closing here would reopen it on click.
      if (panel.current?.contains(t) || anchor?.contains(t)) return;
      // So does its label, which clicks it; pressing one would otherwise blur the anchor first.
      if (anchor instanceof HTMLButtonElement && Array.from(anchor.labels).some((label) => label.contains(t))) {
        e.preventDefault();
        return;
      }
      closeRef.current("outside");
    };
    const releaseEscape = dismissOnEscape(() => closeRef.current("escape"));
    document.addEventListener("pointerdown", onDown);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      releaseEscape();
    };
  }, [open, anchor, nativeDismiss]);

  if (!open || !anchor) return null;
  return createPortal(
    <div
      ref={panel}
      popover={nativePopover ? (nativeDismiss ? "auto" : "manual") : undefined}
      data-placement={placement}
      data-align={align}
      data-anchored={anchored || undefined}
      className={cx(styles.popover, className)}
      onKeyDown={event => {
        onKeyDown?.(event);
        // Contain keyboard propagation without cancelling the browser's native
        // Escape dismissal; React portals otherwise reach a parent inspector.
        if (event.key === "Escape" && !event.nativeEvent.isComposing && event.nativeEvent.keyCode !== 229) event.stopPropagation();
      }}
      style={{
        ...(anchored ? { positionAnchor: anchorName } : pos ? { left: pos.left, top: pos.top } : { left: 0, top: 0, opacity: 0 }),
        ...(fitAnchor ? { minWidth: anchor.getBoundingClientRect().width } : {}),
      }}
      {...rest}
    >
      {children}
    </div>,
    anchor.closest('dialog, [role="dialog"]') ?? document.body,
  );
}
