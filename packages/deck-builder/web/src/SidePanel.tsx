// Search and agent chat share one column, and that column is the one you work
// in — so it can be dragged wider and, at the limit, maximized over the whole
// page. Width lives in a CSS custom property on :root rather than React state:
// dragging then costs no re-render of the decklist, which is the expensive
// part of this page. The committed value is persisted; maximize is not, since
// landing on a deck page with the deck itself hidden reads as a broken app.

import { useCallback, useState, type ReactNode } from "react";
import { useDeck } from "./store.tsx";
import { useDismissable } from "./lib.ts";
import { SidebarResize } from "./SidebarResize.tsx";

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
  useDismissable(
    useCallback(() => setMaxed(false), []),
    maxed,
  );

  return (
    <aside className={`side-panel ${maxed ? "is-max" : ""}`}>
      <SidebarResize side="right" />
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
