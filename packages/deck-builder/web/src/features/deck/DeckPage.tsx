// The deck view. The list is home: the header, the add bar, the proposal
// banner and the side panel all sit around it and none of them covers it.
// This file only composes — each region is its own component and reads the
// deck through the two stores — and owns the keyboard shortcuts, since they
// cut across regions.

import { useCallback, useEffect } from "react";
import { href } from "../../app/routes.ts";
import { CardOpenProvider, useCardLookup } from "../../mtg/index.ts";
import { Banner, Button, EmptyState, Loading, Page, pressesFocusedControl, useHotkeys } from "../../ui/index.ts";
import { DeckBody } from "./DeckBody.tsx";
import { DeckDialogs } from "./dialogs/DeckDialogs.tsx";
import { DeckHeader } from "./DeckHeader.tsx";
import { DeckProvider, useDeck, useDeckState } from "./DeckStore.tsx";
import { DeckToolbar } from "./DeckToolbar.tsx";
import { DeckUiProvider, useDeckUi } from "./DeckUi.tsx";
import { DeckList } from "./list/DeckList.tsx";
import { SelectionBar } from "./list/SelectionBar.tsx";
import { DeckPanel } from "./panel/DeckPanel.tsx";
import { ProposalBanner } from "./ProposalBanner.tsx";
import { DeckViewProvider, useDeckView } from "./useDeckView.ts";
import { useReportFocus } from "../../app/useReportFocus.ts";
import { useReview } from "./useReview.ts";
import styles from "./DeckPage.module.css";

export function DeckPage({ deckId, review = false }: { deckId: number; review?: boolean }) {
  return (
    <DeckProvider deckId={deckId} key={deckId}>
      <DeckUiProvider>
        <DeckGate startInReview={review} />
      </DeckUiProvider>
    </DeckProvider>
  );
}

function DeckGate({ startInReview }: { startInReview: boolean }) {
  const { state, missing } = useDeck();
  if (missing)
    return (
      <Page>
        <EmptyState title="This deck does not exist">
          It may have been deleted. <a href={href({ page: "decks" })}>Back to all decks</a>
        </EmptyState>
      </Page>
    );
  if (!state) return <Loading />;
  return <DeckScreen startInReview={startInReview} />;
}

function DeckScreen({ startInReview }: { startInReview: boolean }) {
  const state = useDeckState();
  const { error, clearError } = useDeck();
  const ui = useDeckUi();
  const view = useDeckView();
  const review = useReview(view.units);
  const { lookup, prime } = useCardLookup();

  useReportFocus(state.deck.id, state.deck.name, ui.selected);

  // The deck's own cards never need a request to show their hover card.
  useEffect(() => prime(state.cards), [prime, state.cards]);

  // Any card named anywhere on this screen opens in the side panel, whether
  // or not it is in the deck.
  const { openCardPanel } = ui;
  const openByName = useCallback(
    (name: string) => {
      const known =
        state.cards.find((c) => c.name === name) ??
        state.proposals.flatMap((p) => p.items).find((i) => i.card_name === name);
      if (known) return openCardPanel({ oracleId: known.oracle_id, name });
      void lookup(name).then((card) => card?.oracle_id && openCardPanel({ oracleId: card.oracle_id, name }));
    },
    [state.cards, state.proposals, lookup, openCardPanel],
  );

  const pending = view.units.length > 0;

  // Arriving from the inbox: go straight into the review it linked to.
  const { startReview } = ui;
  useEffect(() => {
    if (startInReview && pending) startReview();
    // Once, on arrival; later rulings must not restart it.
    // (pending is read at mount only.)
  }, [startInReview, startReview]);

  useHotkeys({
    // Escape backs out one layer at a time, innermost first.
    escape: () => {
      if (ui.rejecting) ui.setRejecting(null);
      else if (ui.openCard) ui.openCardPanel(null);
      else if (ui.selected.length) ui.clearSelection();
      else if (ui.reviewing) ui.stopReview();
    },
    enter: (e) => pending && !ui.reviewing && !pressesFocusedControl(e) && ui.startReview(),
    j: () => ui.reviewing && ui.setCursor(Math.min(ui.cursor + 1, view.units.length - 1)),
    k: () => ui.reviewing && ui.setCursor(ui.cursor - 1),
    a: () => ui.reviewing && review.current && void review.accept(review.current),
    r: (e) => {
      if (!ui.reviewing || !review.current) return;
      e.preventDefault(); // or the "r" lands in the reason field it opens
      ui.setRejecting(review.current.key);
    },
    "shift+a": () => ui.reviewing && void review.acceptAll(),
  });

  return (
    <CardOpenProvider onOpen={openByName}>
      <DeckViewProvider value={view}>
        <div className={styles.page}>
          <DeckHeader />
          <DeckToolbar />
          {error && (
            <Banner tone="error" actions={<Button size="sm" variant="ghost" onClick={clearError}>Dismiss</Button>}>
              {error}
            </Banner>
          )}
          <ProposalBanner units={view.units} review={review} />
          <DeckBody
            list={
              <>
                <DeckList view={view} />
                <SelectionBar />
              </>
            }
            panel={<DeckPanel view={view} review={review} />}
          />
          <DeckDialogs />
        </div>
      </DeckViewProvider>
    </CardOpenProvider>
  );
}
