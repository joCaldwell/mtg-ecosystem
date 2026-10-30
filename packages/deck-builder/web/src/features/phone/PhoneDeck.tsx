import { ColorIdentity, CardRef, DeckArt } from "../../mtg/index.ts";
// A deck on a phone (spec §11.2): the same read-only composition as a shared
// page, fed by the owner's own deck, with its pending proposals drawn in place
// and ruled on here. Accept and reject are the only writes a phone makes.
// The deck store refetches on the live stream, so an agent's new proposal
// appears without a reload.

import { useMemo } from "react";
import { href, useScrollRestore } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { pendingItems } from "../../domain/inbox.ts";
import { fromOwner } from "../../domain/readOnlyDeck.ts";
import { DeckProvider, ReadOnlyDeck, useDeck, useDeckView } from "../deck/index.ts";
import { Banner, Button, Chip, EmptyState, Icon, Loading } from "../../ui/index.ts";
import { PhoneDecisions } from "./PhoneDecisions.tsx";
import styles from "./PhoneDeck.module.css";

export function PhoneDeck({ deckId, proposalId }: { deckId: number; proposalId?: number }) {
  return (
    <DeckProvider deckId={deckId} key={deckId}>
      <Gate proposalId={proposalId} />
    </DeckProvider>
  );
}

function Gate({ proposalId }: { proposalId?: number }) {
  const { state, missing, loadError, retry } = useDeck();
  if (missing)
    return <EmptyState title="This deck does not exist">It may have been deleted. <a href={href({ page: "decks" })}>Back to your decks</a></EmptyState>;
  if (!state && loadError)
    return <EmptyState title="Could not load this deck">The server could not be reached. <Button size="touch" onClick={retry}>Retry</Button></EmptyState>;
  if (!state) return <Loading />;
  return <Deck proposalId={proposalId} />;
}

function Deck({ proposalId }: { proposalId?: number }) {
  const { state, error, clearError } = useDeck();
  useScrollRestore(true);
  const model = useMemo(() => fromOwner(state!), [state]);
  const deck = state!.deck;
  const edits = state!.document_edits.filter((e) => e.group_id == null || e.group_id === e.id).length;
  return (
    <ReadOnlyDeck
      model={model}
      useView={useDeckView}
      header={<>
        <DeckArt cards={proposalId ? [] : state!.cards.filter(c => c.role === "commander" && !c.maybe)} identity={deck.color_identity} variant="banner" priority="auto"><header className={styles.head}>
          <a className={styles.back} href={href({ page: proposalId ? "inbox" : "decks" })}><Icon name="back" /> {proposalId ? "Inbox" : "Decks"}</a>
          <div className={styles.top}>
            <h1 className={styles.title}>{deck.name}</h1>
            {!proposalId && <> <Chip className={styles.lifecycle}>{DECK_CATEGORY_LABELS[deck.category]}</Chip></>}
          </div>
          {!proposalId && <div className={styles.commanders}>
            {state!.cards.filter(c => c.role === "commander" && !c.maybe).map(c => <CardRef key={c.oracle_id} name={c.name} card={c} />)}
            <ColorIdentity colors={deck.color_identity} />
          </div>}
          {!proposalId && <a className={styles.readDeck} href="#deck-list">Read deck · {model.list.computed.card_count} cards</a>}
          {!proposalId && deck.visibility !== "private" && deck.slug && <div className={styles.meta}>
            <a href={href({ page: "shared", slug: deck.slug })}><Icon name={deck.visibility === "public" ? "globe" : "link"} /> {deck.visibility === "public" ? "Public" : "Unlisted"} page</a>
          </div>}
        </header></DeckArt>
        {error && <Banner tone="error" actions={<Button size="touch" variant="ghost" onClick={clearError}>Dismiss</Button>}>{error}</Banner>}
        {edits > 0 && !proposalId && (
          <Banner tone="agent" actions={<a className={styles.reviewInbox} href={href({ page: "inbox" })}>Review in inbox</a>}>
            {edits} proposed brief {edits === 1 ? "edit waits" : "edits wait"} for you.
          </Banner>
        )}
      </>}
      decisions={(view) => view.units.length > 0 && <section id="deck-proposals" aria-label="Proposals" tabIndex={-1}>
        <PhoneDecisions view={view} proposalId={proposalId} />
      </section>}
      listNavigation={model.list.proposals.some(p => pendingItems(p).length > 0) && <a className={styles.jump} href="#deck-proposals">Review proposals</a>}
    />
  );
}
