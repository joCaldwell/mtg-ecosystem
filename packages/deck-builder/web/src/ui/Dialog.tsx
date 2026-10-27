import { useEffect, useId, useLayoutEffect, useRef, type MouseEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./Icon.tsx";
import { IconButton } from "./IconButton.tsx";
import { FOCUSABLE, escapeLayers } from "./internal.ts";
import styles from "./Dialog.module.css";

export interface DialogProps {
  title: ReactNode;
  onClose: () => void;
  width?: "md" | "lg";
  /** `sheet` rises from the bottom edge at full width: the touch form (DESIGN.md). */
  placement?: "center" | "sheet" | "drawer";
  children: ReactNode;
  footer?: ReactNode;
  /** A workflow with its own scrolling region can size the body. */
  bodyClassName?: string;
}

/**
 * A modal. Focus goes to the first `data-autofocus` element inside (else the
 * first focusable one), Tab stays inside, Escape or a backdrop click closes,
 * and focus returns to where it was when the dialog closes.
 */
export function Dialog({ title, onClose, width = "md", placement = "center", children, footer, bodyClassName }: DialogProps) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  useLayoutEffect(() => {
    closeRef.current = onClose;
  });

  // Focus in on open, back out on close; hold the page still meanwhile.
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const el = panel.current;
    const visible = (selector: string) => Array.from(el?.querySelectorAll<HTMLElement>(selector) ?? []).find(item => item.getClientRects().length > 0);
    const target = visible("[data-autofocus]") ?? visible(FOCUSABLE);
    (target ?? el)?.focus();

    const unlock = lockBodyScroll();
    // A field inside the dialog that handles its own Escape (an inline
    // editor, a reason) gets the first press; the next one closes the dialog.
    const releaseEscape = dismissOnEscape(() => closeRef.current());

    return () => {
      releaseEscape();
      unlock();
      previous?.focus();
    };
  }, []);

  // Native, not onKeyDown: content may be portalled into the panel from
  // elsewhere in the React tree (the deck panel in its sheet), and React
  // bubbles those events past the dialog. A nested surface that already
  // moved focus marks the event handled.
  useEffect(() => {
    const el = panel.current;
    el?.addEventListener("keydown", trapTab);
    return () => el?.removeEventListener("keydown", trapTab);
  }, []);

  function trapTab(e: KeyboardEvent) {
    if (e.key !== "Tab" || e.defaultPrevented || !panel.current) return;
    // Native Tab skips hidden content; trap the same visible controls.
    const items = Array.from(panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(item => item.getClientRects().length > 0);
    const first = items[0];
    const last = items[items.length - 1];
    if (!first || !last) {
      e.preventDefault();
      return;
    }
    const active = document.activeElement;
    if (e.shiftKey && (active === first || active === panel.current)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function onBackdropMouseDown(e: MouseEvent) {
    // Only a press that starts on the backdrop itself; dragging a text selection out of the panel must not close.
    if (e.target === e.currentTarget) onClose();
  }

  return createPortal(
    <div className={`${styles.backdrop} ${placement === "sheet" ? styles.sheetBackdrop : placement === "drawer" ? styles.drawerBackdrop : ""}`} onMouseDown={onBackdropMouseDown}>
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        className={`${styles.dialog} ${placement === "sheet" ? styles.sheet : placement === "drawer" ? styles.drawer : width === "lg" ? styles.lg : styles.md}`}
      >
        <div className={styles.head}>
          <h2 id={titleId} className={styles.title}>
            {title}
          </h2>
          <IconButton label="Close" size={placement !== "center" ? "touch" : "sm"} onClick={onClose}>
            <Icon name="close" />
          </IconButton>
        </div>
        <div className={`${styles.body} ${bodyClassName ?? ""}`}>{children}</div>
        {footer ? <div className={styles.footer}>{footer}</div> : null}
      </div>
    </div>,
    document.body,
  );
}

// Nested dialogs can close in the same commit, parent first. Each saving and
// restoring body.style.overflow let the child write "hidden" back after the
// parent restored it, so the lock is counted, like app/windowScroll.ts.
// (Here, not in internal.ts, so it stays out of the public entry chunk.)
let scrollLocks = 0;
let unlockedOverflow = "";
export function lockBodyScroll(): () => void {
  if (scrollLocks++ === 0) {
    unlockedOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
  }
  return () => {
    if (--scrollLocks === 0) document.body.style.overflow = unlockedOverflow;
  };
}

// The shared Escape stack (internal.ts), with one difference from a popover:
// when Escape starts in a field, the field's own handler runs first, and the
// dialog closes only if the press bubbles out unhandled (not stopped, not
// prevented). An inline editor in a sheet cancels on the first press and the
// sheet closes on the second.
function dismissOnEscape(close: () => void): () => void {
  escapeLayers.push(close);
  let deferred: Event | null = null;
  const onKey = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || escapeLayers.at(-1) !== close) return;
    if (event.target instanceof Element && event.target.closest("input, textarea, select, [contenteditable]")) {
      deferred = event;
      return;
    }
    event.preventDefault(); event.stopImmediatePropagation(); close();
  };
  const onUnhandled = (event: KeyboardEvent) => {
    if (event !== deferred) return;
    deferred = null;
    if (!event.defaultPrevented && escapeLayers.at(-1) === close) { event.preventDefault(); close(); }
  };
  document.addEventListener("keydown", onKey, true);
  window.addEventListener("keydown", onUnhandled);
  return () => {
    const index = escapeLayers.indexOf(close);
    if (index >= 0) escapeLayers.splice(index, 1);
    document.removeEventListener("keydown", onKey, true);
    window.removeEventListener("keydown", onUnhandled);
  };
}
