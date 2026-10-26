import { useState, useEffect } from "react";
import { PANEL_QUERY, useMediaQuery } from "../../app/breakpoints.ts";
import { Button, Dialog } from "../../ui/index.ts";
import { useDeckUi } from "./DeckUi.tsx";
// The list and the side panel, side by side. How wide the panel is belongs to
// the owner: they drag the edge between the two, and the width is remembered
// for every deck. The state lives here rather than on the page so a drag
// re-renders this frame and neither of the regions inside it.

import { useMemo, useRef, type CSSProperties, type ReactNode } from "react";
import { ResizeHandle, tokenPx, useLocalStorage } from "../../ui/index.ts";
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
  useEffect(() => { if (ui.openCard || ui.reviewing) setSheet(true); }, [ui.openCard, ui.reviewing]);
  // Opening a header stat can select the current tab again, so listen to
  // the explicit details request as well as changes of tab.
  useEffect(() => {
    const open = () => setSheet(true);
    window.addEventListener("deck-details", open);
    return () => window.removeEventListener("deck-details", open);
  }, []);
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

  if (tablet) return <div className={styles.stacked}>
    <div className={styles.panelActions}>
      <Button onClick={() => setSheet(true)} aria-haspopup="dialog">Open deck details</Button>
      <span>Brief, power, checks, memories and decisions</span>
    </div>
    {list}
    {sheet && <Dialog title={ui.openCard ? ui.openCard.name : "Deck details"} placement="sheet" onClose={() => { setSheet(false); if (ui.openCard) ui.openCardPanel(null); }}><div className={styles.sheetPanel}>{panel}</div></Dialog>}
  </div>;
  return (
    <div
      ref={body}
      className={styles.body}
      style={width === null ? undefined : ({ "--deck-panel-w": `${width}px` } as CSSProperties)}
    >
      <div className={styles.listPane}>{list}</div>
      <div className={styles.panelPane}>
        <ResizeHandle
          label="Resize the deck panel"
          edge="left"
          value={width ?? limits.initial}
          min={limits.min}
          max={limits.max}
          onChange={resize}
          onReset={() => setWidth(null)}
        />
        {panel}
      </div>
    </div>
  );
}
