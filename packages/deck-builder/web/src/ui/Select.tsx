import { useId, useLayoutEffect, useRef, useState, type ButtonHTMLAttributes, type KeyboardEvent } from "react";
import { Button } from "./Button.tsx";
import { Chevron } from "./Chevron.tsx";
import { cx } from "./internal.ts";
import { Popover } from "./Popover.tsx";
import listbox from "./Listbox.module.css";
import styles from "./Select.module.css";

export interface SelectOption {
  value: string;
  label: string;
  title?: string;
  disabled?: boolean;
}

export interface SelectProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "value" | "defaultValue" | "onChange" | "children"> {
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  /** Shown while `value` matches no option. */
  placeholder?: string;
  invalid?: boolean;
  /** For the style guide: starts open without taking focus. */
  defaultOpen?: boolean;
}

// Long enough to type a word, short enough that a later letter starts afresh.
const TYPEAHEAD_MS = 500;

/** Picks one of a list, drawn like MultiSelect. This is the WAI-ARIA select-only
 *  combobox: focus stays on the trigger and the list follows through
 *  aria-activedescendant, so a surrounding Dialog keeps its focus trap. */
export function Select({ options, value, onChange, placeholder, invalid = false, defaultOpen = false, className, onKeyDown, ...rest }: SelectProps) {
  const [open, setOpen] = useState(defaultOpen);
  const [active, setActive] = useState(-1);
  // Pointer users get the hover look; the focus ring is for keyboard travel.
  const [keyed, setKeyed] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const typed = useRef({ text: "", at: 0 });
  const id = useId();
  const listId = `${id}-list`;
  const optionId = (index: number) => `${id}-option-${index}`;
  const selected = options.findIndex((option) => option.value === value);
  const shown = open && !rest.disabled;
  const enabled = (index: number) => options[index] !== undefined && !options[index].disabled;
  const first = () => options.findIndex((option) => !option.disabled);
  const last = () => options.map((option) => !option.disabled).lastIndexOf(true);

  useLayoutEffect(() => {
    if (shown && active >= 0) document.getElementById(optionId(active))?.scrollIntoView({ block: "nearest" });
  }, [shown, active]);

  function show(index: number, keyboard: boolean) {
    setActive(index >= 0 ? index : first());
    setKeyed(keyboard);
    setOpen(true);
  }

  function close() {
    setOpen(false);
    anchor?.focus();
  }

  function commit(index: number) {
    if (!enabled(index)) return;
    close();
    if (options[index].value !== value) onChange(options[index].value);
  }

  // The nearest enabled option at or past `from`, stopping at the list's ends.
  function step(from: number, direction: 1 | -1) {
    for (let index = from; index >= 0 && index < options.length; index += direction) {
      if (enabled(index)) return index;
    }
    return active;
  }

  function typeAhead(key: string, from: number) {
    const now = Date.now();
    const text = (now - typed.current.at < TYPEAHEAD_MS ? typed.current.text + key : key).toLocaleLowerCase();
    typed.current = { text, at: now };
    // Repeating one letter walks through the options that start with it.
    const repeated = [...text].every((letter) => letter === text[0]);
    const query = repeated ? text[0] : text;
    const start = text.length === 1 || repeated ? from + 1 : Math.max(from, 0);
    for (let offset = 0; offset < options.length; offset++) {
      const index = (start + offset) % options.length;
      if (enabled(index) && options[index].label.toLocaleLowerCase().startsWith(query)) return index;
    }
    return -1;
  }

  function handleKey(event: KeyboardEvent<HTMLButtonElement>) {
    onKeyDown?.(event);
    if (event.ctrlKey || event.metaKey) return;
    const { key } = event;
    const typing = key === " " ? Date.now() - typed.current.at < TYPEAHEAD_MS && typed.current.text !== "" : key.length === 1;
    if (!shown) {
      if (typing) {
        event.preventDefault();
        const match = typeAhead(key, selected);
        show(match >= 0 ? match : selected, true);
      } else if (key === "ArrowDown" || key === "ArrowUp" || key === "Enter" || key === " ") {
        event.preventDefault();
        show(selected, true);
      } else if (key === "Home" || key === "End") {
        event.preventDefault();
        show(key === "Home" ? first() : last(), true);
      }
      return;
    }
    if (key === "Tab") {
      // Tab keeps what was highlighted, as a native select does, then moves on.
      setOpen(false);
      if (enabled(active) && options[active].value !== value) onChange(options[active].value);
      return;
    }
    let next = active;
    if (typing) {
      next = typeAhead(key, active);
      if (next < 0) next = active;
    } else if (key === "Enter" || key === " ") {
      event.preventDefault();
      commit(active);
      return;
    } else if (key === "ArrowDown") next = step(active + 1, 1);
    else if (key === "ArrowUp") next = step(active - 1, -1);
    else if (key === "Home") next = first();
    else if (key === "End") next = last();
    else if (key === "PageDown") next = step(Math.min(active + 10, options.length - 1), -1);
    else if (key === "PageUp") next = step(Math.max(active - 10, 0), 1);
    else return;
    event.preventDefault();
    setActive(next);
    setKeyed(true);
  }

  return (
    <>
      <Button
        {...rest}
        ref={setAnchor}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={shown}
        aria-controls={shown ? listId : undefined}
        aria-activedescendant={shown && active >= 0 ? optionId(active) : undefined}
        aria-invalid={invalid || undefined}
        data-value={value}
        className={cx(listbox.trigger, styles.trigger, className)}
        onClick={() => (shown ? close() : show(selected, false))}
        onKeyDown={handleKey}
        // preventDefault on keydown does not stop every browser's Space click.
        onKeyUp={(event) => { if (event.key === " ") event.preventDefault(); }}
        onBlur={() => setOpen(false)}
      >
        <span className={styles.value}>
          {/* Every label is laid out unseen, so the trigger keeps one width
              whichever is chosen, as a native select does. */}
          {[...options.map((option) => option.label), ...(placeholder ? [placeholder] : [])].map((label, index) => (
            <span key={index} className={styles.sizer} aria-hidden="true">{label}</span>
          ))}
          <span className={styles.current}>{options[selected]?.label ?? placeholder}</span>
        </span>
        <Chevron />
      </Button>
      <Popover anchor={anchor} open={shown} onClose={close} fitAnchor className={styles.popover} onMouseDown={(event) => event.preventDefault()}>
        <div className={listbox.panel}>
          <div
            id={listId}
            role="listbox"
            aria-label={rest["aria-label"] ?? anchor?.labels?.[0]?.textContent ?? undefined}
            className={cx(listbox.options, keyed && styles.keyed)}
          >
            {options.map((option, index) => (
              <div
                key={option.value}
                id={optionId(index)}
                role="option"
                aria-selected={index === selected}
                aria-disabled={option.disabled || undefined}
                title={option.title}
                data-value={option.value}
                data-active={index === active || undefined}
                className={cx(listbox.option, styles.option)}
                onMouseMove={() => {
                  if (option.disabled) return;
                  setActive(index);
                  setKeyed(false);
                }}
                onClick={() => commit(index)}
              >
                <span aria-hidden="true">{index === selected ? "✓" : "○"}</span>{option.label}
              </div>
            ))}
          </div>
        </div>
      </Popover>
    </>
  );
}
