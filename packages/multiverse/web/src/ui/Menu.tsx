import { Fragment, useCallback, useEffect, useId, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { Button, type ButtonProps } from "./Button.tsx";
import { Chevron } from "./Chevron.tsx";
import { Icon, type IconName } from "./Icon.tsx";
import { IconButton } from "./IconButton.tsx";
import { Popover } from "./Popover.tsx";
import styles from "./Menu.module.css";
import { cx } from "./internal.ts";

export interface MenuItem {
  id: string;
  label: ReactNode;
  onSelect: () => void;
  tone?: "cut";
  disabled?: boolean;
  /** Set on the choices to make the menu a single choice: each is a
   *  menuitemradio and the checked one shows a tick. An item without it in
   *  such a menu is an ordinary action, lined up with the choices. */
  checked?: boolean;
  /** A rule above this item, setting it apart from the ones before. */
  separated?: boolean;
}

export type MenuProps = {
  /** Accessible name when the visible label repeats across rows. */
  ariaLabel?: string;
  items: MenuItem[];
  variant?: ButtonProps["variant"];
  size?: ButtonProps["size"];
  /** Which edge of the trigger the list lines up with. */
  align?: "start" | "end";
  /** Draw the trigger as a state pill (the current value) rather than a button. */
  pill?: boolean;
  /** Receives the trigger, for a panel that an item opens beside it. */
  triggerRef?: (node: HTMLButtonElement | null) => void;
} & (
  | { label: ReactNode; icon?: undefined }
  /** An icon trigger uses its label as the accessible name and tooltip. */
  | { label: string; icon: IconName }
);

export function Menu({ label, icon, ariaLabel, items, variant = "default", size = "md", align = "start", pill = false, triggerRef }: MenuProps) {
  const [open, setOpen] = useState(false);
  const [trigger, setTriggerState] = useState<HTMLButtonElement | null>(null);
  const setTrigger = useCallback((node: HTMLButtonElement | null) => {
    setTriggerState(node);
    triggerRef?.(node);
  }, [triggerRef]);
  const list = useRef<HTMLDivElement>(null);
  const menuId = useId();
  const triggerId = `${menuId}-trigger`;
  const enter = useRef<"first" | "last" | null>(null);
  const nativeFocusGroup = typeof HTMLElement !== "undefined" && "focusGroup" in HTMLElement.prototype;
  const typed = useRef({ text: "", at: 0 });

  const enabled = () => Array.from(list.current?.querySelectorAll<HTMLElement>('button:not(:disabled)') ?? []);

  const choice = items.some((item) => item.checked !== undefined);

  // Keyboard opens enter the menu; pointer opens leave focus on the trigger.
  useEffect(() => {
    if (!open) return;
    const els = enabled();
    if (enter.current) (enter.current === "last" ? els.at(-1) : els[0])?.focus({ preventScroll: true });
    else trigger?.focus({ preventScroll: true });
    enter.current = null;
  }, [open]);

  function close(reason?: "outside" | "escape") {
    setOpen(false);
    if (reason !== "outside") trigger?.focus();
  }

  function onTriggerKeyDown(e: KeyboardEvent) {
    if (e.defaultPrevented || e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return;
    if (e.key === "Escape" && open) { e.stopPropagation(); return; }
    if (e.key === "Tab" && open) {
      setOpen(false);
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const direction = e.key === "ArrowUp" ? "last" : "first";
    if (open) {
      const els = enabled();
      (direction === "last" ? els.at(-1) : els[0])?.focus({ preventScroll: true });
    } else {
      enter.current = direction;
      setOpen(true);
    }
  }

  function onListKeyDown(e: KeyboardEvent) {
    if (e.defaultPrevented || e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return;
    const els = enabled();
    const i = els.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Tab") {
      // Restore the trigger before native sequential navigation chooses its next stop.
      trigger?.focus();
      setOpen(false);
      return;
    }
    if (nativeFocusGroup && ["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
    if (e.key.length === 1 && e.key !== " " && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const now = Date.now();
      const text = (now - typed.current.at < 500 ? typed.current.text + e.key : e.key).toLocaleLowerCase();
      typed.current = { text, at: now };
      const query = [...text].every(letter => letter === text[0]) ? text[0] : text;
      for (let offset = 1; offset <= els.length; offset++) {
        const item = els[(i + offset) % els.length];
        if (item.textContent?.trim().toLocaleLowerCase().startsWith(query)) { e.preventDefault(); item.focus(); break; }
      }
      return;
    }
    let next = -1;
    if (e.key === "ArrowDown") next = (i + 1) % els.length;
    else if (e.key === "ArrowUp") next = (i - 1 + els.length) % els.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = els.length - 1;
    else return;
    e.preventDefault();
    els[next]?.focus();
  }

  const triggerProps = {
    ref: setTrigger,
    id: triggerId,
    "aria-haspopup": "menu" as const,
    "aria-expanded": open,
    "aria-controls": open ? menuId : undefined,
    onClick: (event: MouseEvent<HTMLButtonElement>) => {
      enter.current = event.detail === 0 ? "first" : null;
      setOpen((o) => !o);
    },
    onKeyDown: onTriggerKeyDown,
  };

  return (
    <>
      {icon ? (
        <IconButton {...triggerProps} size={size} label={ariaLabel ?? label}>
          <Icon name={icon} />
        </IconButton>
      ) : pill ? (
        // The visible pill is the inner face, so a touch layout can make the
        // button a full tap target without fattening the pill.
        <button {...triggerProps} type="button" className={styles.pill} aria-label={ariaLabel}>
          <span className={styles.pillFace}>
            {label}
            <Chevron />
          </span>
        </button>
      ) : (
        <Button {...triggerProps} size={size} variant={variant} aria-label={ariaLabel}>
          {label}
          <Chevron />
        </Button>
      )}
      <Popover anchor={trigger} open={open} onClose={close} align={align}>
        <div
          id={menuId}
          ref={list}
          {...({ focusgroup: "menu nomemory" } as Record<string, string>)}
          role={nativeFocusGroup ? undefined : "menu"}
          aria-labelledby={triggerId}
          className={styles.list}
          onKeyDown={onListKeyDown}
        >
          {items.map((item) => (
            <Fragment key={item.id}>
              {item.separated && <hr className={styles.separator} />}
              <button
                type="button"
                role={choice && item.checked !== undefined ? "menuitemradio" : nativeFocusGroup ? undefined : "menuitem"}
                aria-checked={choice && item.checked !== undefined ? item.checked : undefined}
                tabIndex={nativeFocusGroup ? undefined : -1}
                disabled={item.disabled}
                className={cx(styles.item, item.tone === "cut" && styles.cut)}
                onPointerMove={(event) => { if (!item.disabled) event.currentTarget.focus({ preventScroll: true }); }}
                onClick={() => {
                  setOpen(false);
                  trigger?.focus();
                  item.onSelect();
                }}
              >
                {choice && <span className={styles.tick}>{item.checked && <Icon name="check" size={12} />}</span>}
                {item.label}
              </button>
            </Fragment>
          ))}
        </div>
      </Popover>
    </>
  );
}
