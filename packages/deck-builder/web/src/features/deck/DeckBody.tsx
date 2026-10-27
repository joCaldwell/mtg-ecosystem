// The list and the side panel, side by side. How wide the panel is belongs to
// the owner: they drag the edge between the two, and the width is remembered
// for every deck. The state lives here rather than on the page so a drag
// re-renders this frame and neither of the regions inside it.

// Under 1100 px the panel moves into a sheet the owner opens. It is rendered
// once, through a portal into one host element that is attached to the side
// pane or to the sheet, so crossing the breakpoint (an iPad rotating) keeps a
// Brief draft, a half-typed reason and the panel's tab instead of remounting.

import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { PANEL_QUERY, useMediaQuery } from "../../app/breakpoints.ts";
import { Dialog, ResizeHandle, tokenPx, useLocalStorage } from "../../ui/index.ts";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckBody.module.css";

const PANEL_WIDTH_KEY = "deck.panelWidth";

// null is "never resized, or reset": the stylesheet's default applies.
const decodeWidth = (raw: string | null): number | null => {
  const width = Number(raw);
  return raw && Number.isFinite(width) && width > 0 ? width : null;
};
const encodeWidth = (width: number | null) => (width === null ? "" : String(width));

export function DeckBody({ list, panel }: { list: ReactNode; panel: ReactNode }) {
  const tablet = useMediaQuery(PANEL_QUERY);
  const ui = useDeckUi();
  const [sheet, setSheet] = useState(false);
  const tabletNow = useRef(tablet);
  tabletNow.current = tablet;
  // Only a card or an explicit request opens the sheet. A review runs in the
  // list, where its keys work; a modal sheet would cover it and disable them.
  useEffect(() => { if (ui.openCard && tabletNow.current) setSheet(true); }, [ui.openCard]);
  // R in a review opens the reason field, which is in the panel: show it,
  // and, if the sheet opened only for that, go back to the list once the
  // reason is sent or cancelled.
  const forReason = useRef(false);
  useEffect(() => {
    if (ui.rejecting && tabletNow.current) setSheet(open => { if (!open) forReason.current = true; return true; });
    else if (!ui.rejecting && forReason.current) { forReason.current = false; setSheet(false); }
  }, [ui.rejecting]);
  // Opening a header stat can select the current tab again, so listen to
  // the explicit details request as well as changes of tab.
  useEffect(() => {
    const open = () => tabletNow.current && setSheet(true);
    window.addEventListener("deck-details", open);
    return () => window.removeEventListener("deck-details", open);
  }, []);
  // Wider than a tablet, the panel is beside the list; a stale sheet must not
  // reappear when the window narrows again.
  useEffect(() => { if (!tablet) setSheet(false); }, [tablet]);
  const [host] = useState(() => {
    const el = document.createElement("div");
    el.className = styles.panelHost;
    return el;
  });
  const attach = useCallback((slot: HTMLDivElement | null) => { if (slot) slot.appendChild(host); }, [host]);
  const body = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useLocalStorage(PANEL_WIDTH_KEY, decodeWidth, encodeWidth);
  const limits = useMemo(
    () => ({
      initial: tokenPx("--panel-w"),
      min: tokenPx("--panel-w-min"),
      max: tokenPx("--panel-w-max"),
      list: tokenPx("--list-w-min"),
    }),
    [],
  );

  // The stylesheet holds the same limits, so a stored width from a wider
  // window still fits; this keeps what is stored equal to what is shown.
  const resize = (next: number) => {
    const room = (body.current?.clientWidth ?? Infinity) - limits.list;
    setWidth(Math.max(limits.min, Math.min(next, room)));
  };

  // Every child keeps its position whichever layout applies, so the list is
  // never remounted either.
  return (
    <div
      ref={body}
      className={tablet ? styles.stacked : styles.body}
      style={tablet || width === null ? undefined : ({ "--deck-panel-w": `${width}px` } as CSSProperties)}
    >
      <div className={tablet ? styles.stackedList : styles.listPane}>{list}</div>
      {!tablet && <div className={styles.panelPane}>
        <ResizeHandle
          label="Resize the deck panel"
          edge="left"
          value={width ?? limits.initial}
          min={limits.min}
          max={limits.max}
          onChange={resize}
          onReset={() => setWidth(null)}
        />
        <div ref={attach} className={styles.panelSlot} />
      </div>}
      {tablet && sheet && <Dialog title={ui.openCard ? ui.openCard.name : "Deck details"} placement="sheet" onClose={() => { setSheet(false); forReason.current = false; if (ui.openCard) ui.openCardPanel(null); }}>
        <div ref={attach} className={styles.sheetPanel} />
      </Dialog>}
      {createPortal(panel, host)}
    </div>
  );
}
