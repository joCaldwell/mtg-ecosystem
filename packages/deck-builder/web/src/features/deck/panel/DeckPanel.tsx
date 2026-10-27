// One explicit destination drives the header, content region and shared sheet.
import { useEffect, useId, useRef } from "react";
import { Button, Select } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useDeckUi, type PanelTab } from "../DeckUi.tsx";
import { PANEL_VIEWS, panelViewLabel } from "../panelViews.ts";
import type { DeckViewModel } from "../useDeckView.ts";
import type { Review } from "../useReview.tsx";
import { BriefPanel } from "./BriefPanel.tsx";
import { CardPanel } from "./CardPanel.tsx";
import { ChecksPanel } from "./ChecksPanel.tsx";
import { HistoryPanel } from "./HistoryPanel.tsx";
import { MemoriesPanel } from "./MemoriesPanel.tsx";
import { PowerPanel } from "./PowerPanel.tsx";
import { ReviewPanel } from "./ReviewPanel.tsx";
import { StatsPanel } from "./StatsPanel.tsx";
import styles from "./DeckPanel.module.css";

export function DeckPanel({ view, review }: { view: DeckViewModel; review: Review }) {
  const ui = useDeckUi();
  const state = useDeckState();
  const pending = view.units.length;
  const active = ui.openCard ? "card" : ui.tab;
  const name = ui.openCard ? `Card details for ${ui.openCard.name}` : panelViewLabel(ui.tab);
  const selectId = useId();
  const bodyId = useId();
  const selector = useRef<HTMLSelectElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const openId = ui.openCard?.oracleId ?? null;
  useEffect(() => { body.current?.scrollTo({ top: 0 }); }, [ui.tab, openId]);
  useEffect(() => {
    if (pending === 0 && ui.tab === "review" && !openId && document.activeElement === document.body) selector.current?.focus({ preventScroll: true });
  }, [pending, ui.tab, openId]);

  return <aside className={styles.panel} aria-label="Deck panel" data-deck-panel>
    <div className={styles.navigation}>
      <label className={styles.label} htmlFor={selectId}>Panel view</label>
      <div className={styles.controls}>
        <Select ref={selector} id={selectId} aria-controls={bodyId} value={active} onChange={event => {
          if (event.target.value !== "card") ui.setTab(event.target.value as PanelTab);
        }}>
          {ui.openCard && <option value="card">Card details</option>}
          {PANEL_VIEWS.map(item => <option key={item.id} value={item.id}>{item.label}{item.id === "review" && pending ? ` (${pending})` : ""}</option>)}
        </Select>
        {ui.openCard && <Button size="sm" variant="ghost" onClick={() => { ui.openCardPanel(null); selector.current?.focus(); }}>Back to {panelViewLabel(ui.tab)}</Button>}
      </div>
    </div>
    <div ref={body} id={bodyId} className={styles.body} role="region" aria-label={name} tabIndex={0} data-panel-body>
      {ui.openCard ? <CardPanel key={ui.openCard.oracleId} card={ui.openCard} />
        : ui.tab === "review" ? <ReviewPanel view={view} review={review} />
        : ui.tab === "brief" ? <BriefPanel />
        : ui.tab === "power" ? <PowerPanel />
        : ui.tab === "stats" ? <StatsPanel />
        : ui.tab === "checks" ? <ChecksPanel checks={state.checks} />
        : ui.tab === "memories" ? <MemoriesPanel />
        : <HistoryPanel />}
    </div>
  </aside>;
}
