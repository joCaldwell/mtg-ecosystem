import { useId, useState, useRef, useEffect, type ReactNode } from "react";
import { Button } from "./Button.tsx";
import { FOCUSABLE } from "./internal.ts";
import { Popover } from "./Popover.tsx";
import styles from "./Explainer.module.css";

type ExplainerProps = { title: string; heading?: "h2" | "h3"; children: ReactNode } & (
  | { label: string; summary?: string; defaultOpen?: boolean; control?: undefined }
  /** Opened by another control, such as a menu item: the explanation sits
   *  beside `anchor`, which takes focus back when it closes. No trigger of its own. */
  | { control: { anchor: HTMLElement | null; open: boolean; onClose: () => void }; label?: undefined; summary?: undefined; defaultOpen?: undefined }
);

/** Optional concept help, named for the question it answers.
 *  `defaultOpen` is for the style guide: it starts open without taking focus. */
export function Explainer({ title, label, summary, heading: Heading = "h3", defaultOpen = false, control, children }: ExplainerProps) {
  const [ownOpen, setOwnOpen] = useState(defaultOpen);
  const [ownAnchor, setOwnAnchor] = useState<HTMLButtonElement | null>(null);
  const open = control ? control.open : ownOpen;
  const anchor = control ? control.anchor : ownAnchor;
  const id = useId();
  const content = useRef<HTMLElement>(null);
  // A controlled explanation only opens when asked for.
  const asked = useRef(!!control);
  // Only an explicit open moves focus into the explanation.
  useEffect(() => { if (open && asked.current) content.current?.focus(); asked.current = true; }, [open]);
  const hide = () => (control ? control.onClose() : setOwnOpen(false));
  const close = () => { hide(); anchor?.focus(); };
  // The panel is portalled to the end of the body (or its dialog), so native
  // Tab order would leave it for the page's end, with the panel still open.
  // Keep the order the trigger implies (WCAG 2.4.3): Tab past the last
  // control continues after the trigger, Shift+Tab before the first returns
  // to it, and focus leaving any other way closes the panel. Native listeners
  // run before a surrounding dialog's focus trap, which then leaves it alone.
  useEffect(() => {
    const panel = content.current;
    if (!open || !panel || !anchor) return;
    const keyDown = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const items = visible(panel);
      const active = document.activeElement;
      if (!(e.shiftKey ? active === panel || active === items[0] : active === items.at(-1))) return;
      e.preventDefault();
      hide();
      if (e.shiftKey) return anchor.focus();
      const order = visible(anchor.closest('[role="dialog"]') ?? document.body).filter(el => !panel.contains(el));
      (order[order.indexOf(anchor) + 1] ?? anchor).focus();
    };
    const focusOut = (e: FocusEvent) => {
      const next = e.relatedTarget;
      if (next instanceof Node && !panel.contains(next) && next !== anchor) hide();
    };
    panel.addEventListener("keydown", keyDown);
    panel.addEventListener("focusout", focusOut);
    return () => { panel.removeEventListener("keydown", keyDown); panel.removeEventListener("focusout", focusOut); };
  }, [open, anchor]);
  const panel = <Popover anchor={anchor} open={open} onClose={close}>
    <section ref={content} tabIndex={-1} id={id} role="region" aria-label={title} className={styles.panel}>
      <Heading className={styles.heading}>{title}</Heading><div>{children}</div><Button size="sm" onClick={close}>Close explanation</Button>
    </section>
  </Popover>;
  if (control) return panel;
  return <div className={styles.explainer}>
    {summary && <span>{summary} </span>}
    <button ref={setOwnAnchor} type="button" className={styles.learn} aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => setOwnOpen(!open)}>{label}</button>
    {panel}
  </div>;
}

function visible(root: Element): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(el => el.getClientRects().length > 0);
}
