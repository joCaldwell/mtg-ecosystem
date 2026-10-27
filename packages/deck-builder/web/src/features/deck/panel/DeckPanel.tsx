// One explicit destination drives the header, content region and shared sheet.
import { useEffect, useLayoutEffect, useRef } from "react";
import { Badge } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
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
  const panel = useRef<HTMLElement>(null);
  const links = useRef(new Map<string, HTMLButtonElement>());
  const back = useRef<HTMLButtonElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const cardBody = useRef<HTMLDivElement>(null);
  const deckScroll = useRef(0);
  const previous = useRef({ tab: ui.tab, card: null as string | null });
  const openId = ui.openCard?.oracleId ?? null;
  useLayoutEffect(() => {
    const before = previous.current;
    const active = document.activeElement;
    if (ui.tab !== before.tab) deckScroll.current = 0;
    if (openId !== before.card) {
      if (openId) {
        if (cardBody.current) cardBody.current.scrollTop = 0;
        const origin = ui.cardOrigin.current;
        if ((!before.card && origin && panel.current?.contains(origin)) || active === document.body) back.current?.focus({ preventScroll: true });
      } else if (before.card && (active === document.body || (active instanceof Element && panel.current?.contains(active)))) {
        const origin = ui.cardOrigin.current;
        const visible = origin?.isConnected && origin.getClientRects().length && !origin.closest("[hidden], [inert]");
        (visible ? origin : links.current.get(ui.tab))?.focus({ preventScroll: true });
      }
    }
    if (!openId && body.current) body.current.scrollTop = deckScroll.current;
    previous.current = { tab: ui.tab, card: openId };
  }, [ui.tab, openId, ui.cardOrigin]);
  useEffect(() => {
    if (pending === 0 && ui.tab === "review" && !openId && document.activeElement === document.body) links.current.get("review")?.focus({ preventScroll: true });
  }, [pending, ui.tab, openId]);

  return <aside ref={panel} className={styles.panel} aria-label="Deck panel" data-deck-panel onKeyDown={event => {
    if (event.key === "Escape" && ui.openCard && !event.defaultPrevented) {
      event.preventDefault(); event.stopPropagation(); ui.openCardPanel(null);
    }
  }}>
    {!ui.openCard && <div className={styles.navigation}>
      <nav className={styles.links} aria-label="Deck panel views">
        {PANEL_VIEWS.map(item => <button key={item.id} type="button" ref={node => { if (node) links.current.set(item.id, node); else links.current.delete(item.id); }} aria-current={ui.tab === item.id ? "true" : undefined} onClick={() => ui.setTab(item.id)}>
          {item.label}{item.id === "review" && pending > 0 && <Badge className={styles.count} tone="agent">{pending}</Badge>}
        </button>)}
      </nav>
    </div>}
    <div ref={body} className={styles.body} role="region" aria-label={panelViewLabel(ui.tab)} tabIndex={0} hidden={!!ui.openCard} inert={!!ui.openCard} data-panel-body={ui.openCard ? undefined : true} onScroll={() => { if (!ui.openCard && body.current) deckScroll.current = body.current.scrollTop; }}>
      {ui.tab === "review" ? <ReviewPanel view={view} review={review} />
        : ui.tab === "brief" ? <BriefPanel />
        : ui.tab === "power" ? <PowerPanel />
        : ui.tab === "stats" ? <StatsPanel />
        : ui.tab === "checks" ? <ChecksPanel checks={state.checks} />
        : ui.tab === "memories" ? <MemoriesPanel />
        : <HistoryPanel />}
    </div>
    {ui.openCard && <CardPanel key={ui.openCard.oracleId} card={ui.openCard} backRef={back} bodyRef={cardBody} />}
  </aside>;
}
