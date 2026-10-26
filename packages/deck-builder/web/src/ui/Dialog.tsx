import { useEffect, useId, useLayoutEffect, useRef, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./Icon.tsx";
import { IconButton } from "./IconButton.tsx";
import { FOCUSABLE, dismissOnEscape } from "./internal.ts";
import styles from "./Dialog.module.css";

export interface DialogProps {
  title: ReactNode;
  onClose: () => void;
  width?: "md" | "lg";
  /** `sheet` rises from the bottom edge at full width: the touch form (DESIGN.md). */
  placement?: "center" | "sheet" | "drawer";
  children: ReactNode;
  footer?: ReactNode;
}

/**
 * A modal. Focus goes to the first `data-autofocus` element inside (else the
 * first focusable one), Tab stays inside, Escape or a backdrop click closes,
 * and focus returns to where it was when the dialog closes.
 */
export function Dialog({ title, onClose, width = "md", placement = "center", children, footer }: DialogProps) {
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

    const overflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const releaseEscape = dismissOnEscape(() => closeRef.current());

    return () => {
      releaseEscape();
      document.body.style.overflow = overflow;
      previous?.focus();
    };
  }, []);

  function trapTab(e: KeyboardEvent) {
    if (e.key !== "Tab" || !panel.current) return;
    // Advanced keeps hidden content mounted. Native Tab skips it; the
    // focus trap must use the same set of visible controls.
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
        onKeyDown={trapTab}
      >
        <div className={styles.head}>
          <h2 id={titleId} className={styles.title}>
            {title}
          </h2>
          <IconButton label="Close" size={placement !== "center" ? "touch" : "sm"} onClick={onClose}>
            <Icon name="close" />
          </IconButton>
        </div>
        <div className={styles.body}>{children}</div>
        {footer ? <div className={styles.footer}>{footer}</div> : null}
      </div>
    </div>,
    document.body,
  );
}
