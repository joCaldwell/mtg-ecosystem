import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Badge, Button } from "../../ui/index.ts";
import styles from "./TableZone.module.css";
export function TableZone({
  label,
  count,
  actions,
  children,
  onDrop,
  emptyText = "Move cards here",
  variant = "row",
  showHeader = true,
}: {
  label: string;
  count: number;
  actions?: ReactNode;
  children?: ReactNode;
  onDrop?: (id: string) => void;
  emptyText?: string;
  variant?: "row" | "hand";
  showHeader?: boolean;
}) {
  const cards = useRef<HTMLDivElement>(null);
  const cardsId = useId();
  const dragDepth = useRef(0);
  const [dropActive, setDropActive] = useState(false);
  const accepts = (transfer: DataTransfer) => !!onDrop && Array.from(transfer.types).includes("application/x-mtg-object");
  const resetDrag = () => { dragDepth.current = 0; setDropActive(false); };
  useEffect(() => {
    // A child card owns insertion/reordering drops and stops propagation.
    // Capture drop and listen for dragend so cancellation also clears the lane.
    const reset = () => { dragDepth.current = 0; setDropActive(false); };
    window.addEventListener("drop", reset, true);
    window.addEventListener("dragend", reset);
    return () => { window.removeEventListener("drop", reset, true); window.removeEventListener("dragend", reset); };
  }, []);
  const [overflow, setOverflow] = useState({ axis: null as "x" | "y" | null, before: false, after: false });
  useLayoutEffect(() => {
    const el = cards.current;
    if (variant !== "hand" || !el) return;
    const measure = () => {
      const axis: "x" | "y" | null = el.scrollWidth > el.clientWidth + 1 ? "x" : el.scrollHeight > el.clientHeight + 1 ? "y" : null;
      const offset = axis === "x" ? el.scrollLeft : el.scrollTop;
      const end = axis === "x" ? el.scrollWidth - el.clientWidth : el.scrollHeight - el.clientHeight;
      const next = { axis, before: offset > 1, after: axis != null && offset < end - 1 };
      setOverflow(previous => previous.axis === next.axis && previous.before === next.before && previous.after === next.after ? previous : next);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    for (const card of el.children) observer.observe(card);
    el.addEventListener("scroll", measure, { passive: true });
    return () => { observer.disconnect(); el.removeEventListener("scroll", measure); };
  }, [variant, count, children]);

  function traverse(direction: number) {
    const el = cards.current;
    if (!el || !overflow.axis) return;
    // Align the next partially hidden card to the leading edge, instead of
    // skipping it or leaving a fresh card cut midway through its rules text.
    const bounds = el.getBoundingClientRect();
    const horizontal = overflow.axis === "x";
    const leading = horizontal ? bounds.left : bounds.top;
    const trailing = horizontal ? bounds.right : bounds.bottom;
    const nodes = [...el.querySelectorAll<HTMLElement>("[data-text-card]")];
    const node = direction > 0
      ? nodes.find(node => (horizontal ? node.getBoundingClientRect().right : node.getBoundingClientRect().bottom) > trailing + 1)
      : nodes.slice().reverse().find(node => (horizontal ? node.getBoundingClientRect().left : node.getBoundingClientRect().top) < leading - 1);
    if (!node) {
      // Account for the scroller's small edge padding after boundary alignment.
      el.scrollTo(horizontal ? { left: direction > 0 ? el.scrollWidth : 0 } : { top: direction > 0 ? el.scrollHeight : 0 });
      return;
    }
    const rect = node.getBoundingClientRect();
    const delta = (horizontal ? rect.left : rect.top) - leading;
    // A tall first card can itself overflow vertically. In that case advance
    // within it before moving on to the next row.
    const page = horizontal ? el.clientWidth : el.clientHeight;
    const distance = direction > 0 ? (delta > 1 ? delta : page) : Math.max(delta, -page);
    el.scrollBy(horizontal ? { left: distance } : { top: distance });
  }
  return (
    <section
      className={`${styles.zone} ${variant === "hand" ? styles.hand : ""}`}
      aria-label={label}
      data-drop-active={dropActive || undefined}
      onDragEnter={(e) => {
        if (accepts(e.dataTransfer)) { dragDepth.current++; setDropActive(true); }
      }}
      onDragLeave={() => {
        dragDepth.current = Math.max(0, dragDepth.current - 1);
        if (dragDepth.current === 0) setDropActive(false);
      }}
      onDragOver={(e) => {
        if (accepts(e.dataTransfer)) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
        }
      }}
      onDrop={(e) => {
        resetDrag();
        const id = e.dataTransfer.getData("application/x-mtg-object");
        if (id && onDrop) {
          e.preventDefault();
          e.stopPropagation();
          onDrop(id);
        }
      }}
    >
      {showHeader && (
        <header className={styles.head}>
          <h2>{label}</h2>
          <Badge>{count}</Badge>
          <div className={styles.actions}>
            {overflow.axis && <div className={styles.traversal} role="group" aria-label={`${label} scrolling`}>
              <Button variant="ghost" aria-controls={cardsId} aria-label={`Previous ${label.toLowerCase()} cards`} disabled={!overflow.before} onClick={() => traverse(-1)}>Previous</Button>
              <Button variant="ghost" aria-controls={cardsId} aria-label={`Next ${label.toLowerCase()} cards`} disabled={!overflow.after} onClick={() => traverse(1)}>Next</Button>
            </div>}
            {actions}
          </div>
        </header>
      )}
      <div className={styles.cards} ref={cards} id={cardsId} data-table-cards onFocusCapture={event => {
        const card = event.target;
        const el = cards.current;
        if (variant !== "hand" || !(card instanceof HTMLElement) || !card.matches("[data-text-card]") || !el) return;
        // Native focus can align a tall card's bottom with the hand viewport,
        // hiding its opening rules. Reveal its start after that native scroll;
        // leave horizontal focus scrolling and deliberate scrolling untouched.
        requestAnimationFrame(() => {
          if (!card.isConnected || document.activeElement !== card) return;
          el.scrollTo({ top: Math.max(0, el.scrollTop + card.getBoundingClientRect().top - el.getBoundingClientRect().top) });
        });
      }}>
        {children}
        {!count && emptyText && <p className={styles.empty}>{emptyText}</p>}
      </div>
    </section>
  );
}
