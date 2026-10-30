import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
// The deck view. The list is home: the header, the add bar, the proposal
// banner and the side panel all sit around it and none of them covers it.
// This file only composes — each region is its own component and reads the
// deck through the two stores — and owns the keyboard shortcuts, since they
// cut across regions.

import { useCallback, useEffect, useRef, useState } from "react";
import { href, navigate } from "../../app/routes.ts";
import { CardOpenProvider, useCardLookup } from "../../mtg/index.ts";
import { Banner, Button, EmptyState, Loading, Page, pressesFocusedControl, useHotkeys } from "../../ui/index.ts";
import { DeckBody } from "./DeckBody.tsx";
import { DeckDialogs } from "./dialogs/DeckDialogs.tsx";
import { DeckHeader } from "./DeckHeader.tsx";
import { DeckProvider, useDeck, useDeckState } from "./DeckStore.tsx";
import { DeckToolbar } from "./DeckToolbar.tsx";
import { DeckFilterSummary } from "./DeckFilterSummary.tsx";
import { DeckUiProvider, useDeckUi } from "./DeckUi.tsx";
import { DeckList } from "./list/DeckList.tsx";
import { SelectionBar } from "./list/SelectionBar.tsx";
import { DeckPanel } from "./panel/DeckPanel.tsx";
import { ProposalBanner } from "./ProposalBanner.tsx";
import { DeckViewProvider } from "./DeckViewContext.ts";
import { useDeckView } from "./useDeckView.ts";
import { useReportFocus } from "../../app/useReportFocus.ts";
import { useReview } from "./useReview.tsx";
import { focusReviewArrival } from "./reviewArrival.ts";
import { firstUnitOf, proposalParam } from "../../domain/reviewStart.ts";
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
  const { state, missing, loadError, retry } = useDeck();
  if (missing)
    return (
      <Page>
        <EmptyState title="This deck does not exist">
          It may have been deleted. <a href={href({ page: "decks" })}>Back to all decks</a>
        </EmptyState>
      </Page>
    );
  if (!state && loadError) return <Page><EmptyState title={loadError.status === 403 ? "You do not have access to this deck" : "Could not load this deck"}>
    {loadError.status === 403 ? "Return to all decks or retry if your access has changed." : "The server could not be reached or returned an error. Please retry."}
    <Button onClick={retry}>Retry</Button>
  </EmptyState></Page>;
  if (!state) return <Loading />;
  return <DeckScreen startInReview={startInReview} />;
}

function DeckScreen({ startInReview }: { startInReview: boolean }) {
  const state = useDeckState();
  const { error, clearError } = useDeck();
  const ui = useDeckUi();
  useEffect(() => {
    if (new URLSearchParams(location.search).get("import") === "1") {
      ui.openDialog("import");
      navigate({ page: "deck", deckId: state.deck.id }, { replace: true });
    }
  }, [ui.openDialog, state.deck.id]);
  // Only this deck's and the global memories can mark its rows (spec §7.2).
  const memories = useQuery(() => api.deckMemories(state.deck.id), [state.deck.id]);
  const view = useDeckView(memories.data ?? undefined);
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

  const pending = state.deck.category !== "archived" && view.units.length > 0;

  // Arriving from the inbox: go straight into the review it linked to.
  const { startReview, setCursor } = ui;
  const arriving = useRef(false);
  useEffect(() => {
    if (startInReview && pending) {
      arriving.current = true;
      startReview();
    }
    // Once, on arrival; later decisions must not restart it.
    // (pending is read at mount only.)
  }, [startInReview, startReview]);
  // Cursor changes render the chosen card before the next animation frame.
  // Reveal the list first, then focus the decision without moving its ancestors.
  useEffect(() => {
    if (!ui.reviewing || !arriving.current) return;
    arriving.current = false;
    const index = firstUnitOf(view.units, proposalParam(location.search));
    const unit = view.units[index];
    setCursor(index);
    if (!unit) return;
    let focusFrame = 0;
    const frame = requestAnimationFrame(() => {
      // An inbox handoff explicitly asks for the selected proposal's controls.
      // On tablets this opens the existing sheet after revealing the card.
      window.dispatchEvent(new Event("deck-details"));
      focusFrame = requestAnimationFrame(() => focusReviewArrival(document, unit.key));
    });
    return () => { cancelAnimationFrame(frame); cancelAnimationFrame(focusFrame); };
  }, [ui.reviewing, setCursor]);

  // Review shortcuts act in the list or the Review destination, never from
  // another focused panel view whose visible controls describe another task.
  function canReview(event: KeyboardEvent) {
    const inPanel = event.target instanceof Element && event.target.closest("[data-deck-panel]");
    return ui.reviewing && (!inPanel || (!ui.openCard && ui.tab === "review"));
  }

  // "Accept all left" asks for confirmation in the banner first.
  const [confirmingAll, setConfirmingAll] = useState(false);
  useEffect(() => { if (!ui.reviewing) setConfirmingAll(false); }, [ui.reviewing]);

  useHotkeys({
    // Escape backs out one layer at a time, innermost first.
    escape: () => {
      if (confirmingAll) setConfirmingAll(false);
      else if (ui.openCard) ui.openCardPanel(null);
      else if (ui.rejecting) ui.setRejecting(null);
      else if (ui.selected.length) ui.clearSelection();
      else if (ui.reviewing) ui.stopReview();
    },
    enter: (e) => pending && !ui.reviewing && !pressesFocusedControl(e) && ui.startReview(),
    j: (event) => canReview(event) && ui.setCursor(Math.min(ui.cursor + 1, view.units.length - 1)),
    k: (event) => canReview(event) && ui.setCursor(ui.cursor - 1),
    a: (event) => canReview(event) && review.current && void review.accept(review.current),
    r: (e) => {
      if (!canReview(e) || !review.current) return;
      e.preventDefault(); // or the "r" lands in the reason field it opens
      ui.setRejecting(review.current.key);
    },
    "shift+a": (event) => canReview(event) && setConfirmingAll(true),
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
          {state.deck.category === "archived" ? <Banner tone="neutral" actions={<Button size="sm" onClick={() => ui.openDialog("fork")} aria-haspopup="dialog">Fork</Button>}>Archived decks are read-only. To make changes, switch this deck to Paper or Theorycrafted, or fork it.</Banner> : <ProposalBanner units={view.units} review={review} confirmingAll={confirmingAll} setConfirmingAll={setConfirmingAll} />}
          <DeckBody
            list={
              <>
                <DeckFilterSummary />
                <DeckList view={view} emptyAction={state.deck.category === "archived" ? undefined : <><p>Search above to add cards, or bring a list.</p><Button onClick={() => ui.openDialog("import")} aria-haspopup="dialog">Import a list</Button></>} />
                {state.deck.category !== "archived" && <SelectionBar />}
              </>
            }
            panel={<DeckPanel view={view} review={review} />}
          />
          {review.confirmation}
          <DeckDialogs />
        </div>
      </DeckViewProvider>
    </CardOpenProvider>
  );
}
