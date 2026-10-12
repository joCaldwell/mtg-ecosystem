// The right-hand column of the deck screen: one tab at a time, or the open
// card over whichever tab that is. DeckBody sets the width; this fills it.

import { useEffect, useRef } from "react";
import { Tabs, type TabSpec } from "../../../ui/index.ts";
import { useDeckUi, type PanelTab } from "../DeckUi.tsx";
import type { DeckViewModel } from "../useDeckView.ts";
import type { Review } from "../useReview.ts";
import { BriefPanel } from "./BriefPanel.tsx";
import { CardPanel } from "./CardPanel.tsx";
import { ChecksPanel } from "./ChecksPanel.tsx";
import { HistoryPanel } from "./HistoryPanel.tsx";
import { MemoriesPanel } from "./MemoriesPanel.tsx";
import { PowerPanel } from "./PowerPanel.tsx";
import { ReviewPanel } from "./ReviewPanel.tsx";
import styles from "./DeckPanel.module.css";

const TABS: Array<{ id: PanelTab; label: string }> = [
  { id: "brief", label: "Brief" },
  { id: "power", label: "Power" },
  { id: "checks", label: "Checks" },
  { id: "memories", label: "Memories" },
  { id: "history", label: "History" },
];

export function DeckPanel({ view, review }: { view: DeckViewModel; review: Review }) {
  const ui = useDeckUi();
  const pending = view.units.length;
  // The Review tab exists only while there is something to rule on, and the
  // stored tab may be left over from before the last ruling.
  const tab: PanelTab = ui.tab === "review" && pending === 0 ? "brief" : ui.tab;

  const tabs: TabSpec[] = [
    ...(pending > 0 ? [{ id: "review", label: "Review", badge: pending, tone: "agent" as const }] : []),
    ...TABS,
  ];

  // A new tab or a new card starts at the top, not wherever the last one was left.
  const body = useRef<HTMLDivElement>(null);
  const openId = ui.openCard?.oracleId ?? null;
  useEffect(() => {
    body.current?.scrollTo({ top: 0 });
  }, [tab, openId]);

  return (
    <aside className={styles.panel} aria-label="Deck panel">
      {/* With a card open no tab is highlighted, but the selected one stays the
          keyboard stop so the row is still reachable. */}
      <Tabs
        tabs={tabs}
        value={tab}
        onChange={(id) => ui.setTab(id as PanelTab)}
        className={ui.openCard ? `${styles.tabs} ${styles.none}` : styles.tabs}
        aria-label="Panel"
      />
      <div ref={body} className={styles.body} role="tabpanel">
        {ui.openCard ? (
          <CardPanel key={ui.openCard.oracleId} card={ui.openCard} />
        ) : tab === "review" ? (
          <ReviewPanel view={view} review={review} />
        ) : tab === "brief" ? (
          <BriefPanel />
        ) : tab === "power" ? (
          <PowerPanel />
        ) : tab === "checks" ? (
          <ChecksPanel />
        ) : tab === "memories" ? (
          <MemoriesPanel />
        ) : (
          <HistoryPanel />
        )}
      </div>
    </aside>
  );
}
