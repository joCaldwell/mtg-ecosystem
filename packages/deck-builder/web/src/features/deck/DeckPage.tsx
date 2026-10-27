import { useArtCollapse } from "../../app/useArtCollapse.ts";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
// The deck view. The list is home: the header, the add bar, the proposal
// banner and the side panel all sit around it and none of them covers it.
// This file only composes — each region is its own component and reads the
// deck through the two stores — and owns the keyboard shortcuts, since they
// cut across regions.

import { useCallback, useEffect, useRef } from "react";
import { href, navigate } from "../../app/routes.ts";
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
import { useReview } from "./useReview.tsx";
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
  const art = useArtCollapse();
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
  // Then move to the proposal selected there. Only once the Review tab is
  // open: the panel starts a new tab at the top after its children have
  // scrolled. The list brings the decision's rows into view; the panel shows
  // the proposal from its heading, at the top, so the context still loading
  // into the decisions above it cannot push it out of view.
  useEffect(() => {
    if (!ui.reviewing || !arriving.current) return;
    arriving.current = false;
    const index = firstUnitOf(view.units, proposalParam(location.search));
    setCursor(index);
    // The panel's own scroll box only: scrollIntoView would also move the
    // pinned desktop page. Under 1100 px the panel is not on the page.
    const block = document.querySelector(`[data-proposal="${view.units[index]?.proposalId}"]`);
    const pane = block?.closest('[role="tabpanel"]');
    if (block && pane) pane.scrollTop += block.getBoundingClientRect().top - pane.getBoundingClientRect().top;
  }, [ui.reviewing, setCursor]);

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
        <div className={styles.page} onScrollCapture={art.onScrollCapture}>
          <DeckHeader collapsed={art.collapsed} />
          <DeckToolbar />
          {error && (
            <Banner tone="error" actions={<Button size="sm" variant="ghost" onClick={clearError}>Dismiss</Button>}>
              {error}
            </Banner>
          )}
          {state.deck.category === "archived" ? <Banner tone="neutral" actions={<Button size="sm" onClick={() => ui.openDialog("fork")} aria-haspopup="dialog">Fork</Button>}>Archived decks are read-only. To make changes, switch this deck to Paper or Theorycrafted, or fork it.</Banner> : <ProposalBanner units={view.units} review={review} />}
          <DeckBody
            list={
              <>
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
