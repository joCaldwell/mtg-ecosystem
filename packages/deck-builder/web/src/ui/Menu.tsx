import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Button, type ButtonProps } from "./Button.tsx";
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
  /** Set on every item to make the menu a single choice: the item is a
   *  menuitemradio and the checked one shows a tick. */
  checked?: boolean;
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
} & (
  | { label: ReactNode; icon?: undefined }
  /** An icon trigger uses its label as the accessible name and tooltip. */
  | { label: string; icon: IconName }
);

export function Menu({ label, icon, ariaLabel, items, variant = "default", size = "md", align = "start", pill = false }: MenuProps) {
  const [open, setOpen] = useState(false);
  const [trigger, setTrigger] = useState<HTMLButtonElement | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const menuId = useId();

  const enabled = () => Array.from(list.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]:not(:disabled)') ?? []);

  const choice = items.some((item) => item.checked !== undefined);

  // Focus the checked item, or the first, as the menu appears.
  useEffect(() => {
    if (!open) return;
    const els = enabled();
    (els.find((el) => el.getAttribute("aria-checked") === "true") ?? els[0])?.focus({ preventScroll: true });
  }, [open]);

  function close() {
    setOpen(false);
    trigger?.focus();
  }

  function onTriggerKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" && !open) {
      e.preventDefault();
      setOpen(true);
    }
  }

  function onListKeyDown(e: KeyboardEvent) {
    const els = enabled();
    const i = els.indexOf(document.activeElement as HTMLElement);
    let next = -1;
    if (e.key === "ArrowDown") next = (i + 1) % els.length;
    else if (e.key === "ArrowUp") next = (i - 1 + els.length) % els.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = els.length - 1;
    else if (e.key === "Tab") {
      setOpen(false);
      return;
    } else return;
    e.preventDefault();
    els[next]?.focus();
  }

  const triggerProps = {
    ref: setTrigger,
    "aria-haspopup": "menu" as const,
    "aria-expanded": open,
    "aria-controls": open ? menuId : undefined,
    onClick: () => setOpen((o) => !o),
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
            <Icon name="chevronDown" size={10} />
          </span>
        </button>
      ) : (
        <Button {...triggerProps} size={size} variant={variant} aria-label={ariaLabel}>
          {label}
          <Icon name="chevronDown" size={12} />
        </Button>
      )}
      <Popover anchor={trigger} open={open} onClose={close} align={align}>
        <div
          id={menuId}
          ref={list}
          role="menu"
          className={styles.list}
          onKeyDown={onListKeyDown}
        >
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              role={choice ? "menuitemradio" : "menuitem"}
              aria-checked={choice ? Boolean(item.checked) : undefined}
              tabIndex={-1}
              disabled={item.disabled}
              className={cx(styles.item, item.tone === "cut" && styles.cut)}
              onClick={() => {
                setOpen(false);
                trigger?.focus();
                item.onSelect();
              }}
            >
              {choice && <span className={styles.tick}>{item.checked && <Icon name="check" size={12} />}</span>}
              {item.label}
            </button>
          ))}
        </div>
      </Popover>
    </>
  );
}
