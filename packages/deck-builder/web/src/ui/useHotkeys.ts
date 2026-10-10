import { useEffect, useLayoutEffect, useRef } from "react";

export interface HotkeyOptions {
  enabled?: boolean;
  /** Fire even while focus is in an input, textarea, select or editable element. */
  allowInInputs?: boolean;
}

type Handler = (e: KeyboardEvent) => void;

interface Combo {
  key: string;
  mod: boolean;
  shift: boolean;
  alt: boolean;
}

const ALIASES: Record<string, string> = { esc: "escape", space: " ", spacebar: " ", return: "enter", del: "delete" };

function parse(combo: string): Combo {
  // A trailing "+" is the plus key itself ("shift++").
  const plusKey = combo === "+" || combo.endsWith("++");
  const parts = (plusKey ? combo.slice(0, -1) : combo).toLowerCase().split("+").filter(Boolean);
  const key = plusKey ? "+" : (parts.pop() ?? "");
  return {
    key: ALIASES[key] ?? key,
    mod: parts.includes("mod") || parts.includes("ctrl") || parts.includes("cmd") || parts.includes("meta"),
    shift: parts.includes("shift"),
    alt: parts.includes("alt"),
  };
}

function matches(c: Combo, e: KeyboardEvent): boolean {
  if (e.key.toLowerCase() !== c.key) return false;
  if ((e.ctrlKey || e.metaKey) !== c.mod) return false;
  if (e.altKey !== c.alt) return false;
  // "?" and "/" need shift on some layouts: shift only counts for keys that have a case.
  const hasCase = e.key.length > 1 || e.key.toLowerCase() !== e.key.toUpperCase();
  return !hasCase || e.shiftKey === c.shift;
}

function inEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT" ||
    target.isContentEditable
  );
}

/**
 * Enter and Space already mean "press this" on a focused control. A handler
 * bound to either key should usually stand down when this is true, or one
 * keypress does two things.
 */
export function pressesFocusedControl(e: KeyboardEvent): boolean {
  if (e.key !== "Enter" && e.key !== " ") return false;
  return e.target instanceof HTMLElement && e.target.closest("button, a[href], summary, [role='button']") != null;
}

/**
 * Keyboard shortcuts for one screen. Keys are lowercase combos: "j", "shift+a",
 * "enter", "escape", "/", "mod+k" (mod is Ctrl or Cmd). Handlers decide whether
 * to preventDefault. Bindings may change every render; the listener is added once.
 */
export function useHotkeys(bindings: Record<string, Handler>, opts: HotkeyOptions = {}): void {
  const { enabled = true, allowInInputs = false } = opts;
  const ref = useRef(bindings);
  useLayoutEffect(() => {
    ref.current = bindings;
  });

  useEffect(() => {
    if (!enabled) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.isComposing) return;
      const isEscape = e.key === "Escape";
      if (!allowInInputs && !isEscape && inEditable(e.target)) return;
      // A modal dialog owns the keyboard while it is open.
      if (document.querySelector('[aria-modal="true"]')) return;
      for (const [combo, handler] of Object.entries(ref.current)) {
        if (matches(parse(combo), e)) {
          handler(e);
          return;
        }
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [enabled, allowInInputs]);
}
