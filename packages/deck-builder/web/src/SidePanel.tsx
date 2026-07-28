// Search and agent chat share one column, and that column is the one you work
// in — so it can be dragged wider and, at the limit, maximized over the whole
// page. Width lives in a CSS custom property on :root rather than React state:
// dragging then costs no re-render of the decklist, which is the expensive
// part of this page. The committed value is persisted; maximize is not, since
// landing on a deck page with the deck itself hidden reads as a broken app.

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useDeck } from "./store.tsx";
import { useDismissable } from "./lib.ts";

// Corner brackets rather than glyphs: the arrow characters render at whatever
// weight and baseline the system font feels like, which never matched the
// panel's line work. Drawn on a 14px grid at the same stroke as the UI text.
function ExpandIcon({ collapse }: { collapse?: boolean }) {
  return (
    <svg viewBox="0 0 14 14" width="12" height="12" aria-hidden="true" focusable="false">
      <path
        d={
          collapse
            ? "M2 5.5h3.5V2M12 5.5H8.5V2M8.5 12V8.5H12M5.5 12V8.5H2"
            : "M5.5 2H2v3.5M8.5 2H12v3.5M12 8.5V12H8.5M5.5 12H2V8.5"
        }
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

const KEY = "deck.sideWidth";
const DEFAULT_W = 372;
const MIN_W = 300;

/** Widest the panel may get by dragging, leaving the decklist a usable column.
 *  Above 1400px the slot rail is a third column and costs its width too; below
 *  it, the panel and the slots share one rail. Past this you want maximize. */
function maxWidth(): number {
  const wide = window.matchMedia("(min-width: 1401px)").matches;
  return Math.max(MIN_W, window.innerWidth - (wide ? 620 : 360));
}

function clamp(w: number): number {
  return Math.min(Math.max(Math.round(w), MIN_W), maxWidth());
}

function apply(w: number) {
  document.documentElement.style.setProperty("--side-w", `${w}px`);
}

export function SidePanel({
  search,
  chat,
}: {
  // Both tabs are rendered by the caller so this component stays about the
  // container — its size, not its contents.
  search: ReactNode;
  chat: ReactNode;
}) {
  // The active tab lives in the store so any surface can raise one — the
  // audit's "ask agent" brings up the chat without a hand-rolled ref channel.
  const { sideTab: tab, setSideTab: setTab } = useDeck();
  const [maxed, setMaxed] = useState(false);
  const panelRef = useRef<HTMLElement>(null);
  const width = useRef(DEFAULT_W);

  function setWidth(w: number, commit = false) {
    width.current = clamp(w);
    apply(width.current);
    if (commit) localStorage.setItem(KEY, String(width.current));
  }

  useEffect(() => {
    const stored = Number(localStorage.getItem(KEY));
    setWidth(Number.isFinite(stored) && stored > 0 ? stored : DEFAULT_W);
    // A stored width that fit the last window may swallow the decklist in this
    // one, so re-clamp as the window changes. Not committed: shrinking the
    // window shouldn't overwrite the width the user chose on a big one.
    const onResize = () => setWidth(width.current);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  useDismissable(
    useCallback(() => setMaxed(false), []),
    maxed,
  );

  function onHandleDown(e: React.PointerEvent<HTMLDivElement>) {
    if (e.button !== 0) return;
    e.preventDefault();
    // Measure once: the panel's right edge is fixed while the left edge moves.
    const right = panelRef.current!.getBoundingClientRect().right;
    // Class rather than state — the cursor has to win over every element the
    // pointer crosses, and re-rendering the page mid-drag would be wasteful.
    document.body.classList.add("col-resizing");
    const move = (ev: PointerEvent) => setWidth(right - ev.clientX);
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      document.body.classList.remove("col-resizing");
      setWidth(width.current, true);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function onHandleKey(e: React.KeyboardEvent) {
    const step = e.shiftKey ? 64 : 16;
    if (e.key === "ArrowLeft") setWidth(width.current + step, true);
    else if (e.key === "ArrowRight") setWidth(width.current - step, true);
    else return;
    e.preventDefault();
  }

  return (
    <aside className={`side-panel ${maxed ? "is-max" : ""}`} ref={panelRef}>
      <div
        className="side-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize panel — double-click to reset"
        tabIndex={0}
        onPointerDown={onHandleDown}
        onDoubleClick={() => setWidth(DEFAULT_W, true)}
        onKeyDown={onHandleKey}
      />
      <div className="tab-bar">
        <button className={`tab ${tab === "search" ? "active" : ""}`} onClick={() => setTab("search")}>
          Search
        </button>
        <button className={`tab ${tab === "chat" ? "active" : ""}`} onClick={() => setTab("chat")}>
          Agent chat
        </button>
        <button
          className="tab tab-max"
          onClick={() => setMaxed(!maxed)}
          title={maxed ? "Restore panel (esc)" : "Expand over the page"}
          aria-label={maxed ? "Restore panel" : "Expand panel over the page"}
          aria-pressed={maxed}
        >
          <ExpandIcon collapse={maxed} />
        </button>
      </div>
      {/* Both tabs stay mounted — a flip must not cost the search results or
          the chat transcript's scroll position — so the inactive one hides
          with CSS rather than unmounting. */}
      <div className={`side-body ${tab === "search" ? "is-search" : "is-chat"}`}>
        <div className={`side-tab ${tab === "search" ? "" : "is-hidden"}`}>{search}</div>
        <div className={`side-tab ${tab === "chat" ? "" : "is-hidden"}`}>{chat}</div>
      </div>
    </aside>
  );
}
