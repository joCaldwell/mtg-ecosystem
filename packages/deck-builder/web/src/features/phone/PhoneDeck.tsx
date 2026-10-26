import { usePreferences } from "../../app/Preferences.tsx";
// A deck on a phone (spec §11.2): the same read-only composition as a shared
// page, fed by the owner's own deck, with its pending proposals drawn in place
// and ruled on here. Accept and reject are the only writes a phone makes.
// The deck store refetches on the live stream, so an agent's new proposal
// appears without a reload.

import { useMemo } from "react";
import { href, useScrollRestore } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { fromOwner } from "../../domain/readOnlyDeck.ts";
import { DeckProvider, ReadOnlyDeck, useDeck } from "../deck/index.ts";
import { Banner, Button, Chip, EmptyState, Icon, Loading } from "../../ui/index.ts";
import { PhoneDecisions } from "./PhoneDecisions.tsx";
import styles from "./PhoneDeck.module.css";

export function PhoneDeck({ deckId }: { deckId: number }) {
  return (
    <DeckProvider deckId={deckId} key={deckId}>
      <Gate />
    </DeckProvider>
  );
}

function Gate() {
  const { state, missing, loadError, retry } = useDeck();
  if (missing)
    return <EmptyState title="This deck does not exist">It may have been deleted. <a href={href({ page: "decks" })}>Back to your decks</a></EmptyState>;
  if (!state && loadError)
    return <EmptyState title="Could not load this deck">The server could not be reached. <Button size="touch" onClick={retry}>Retry</Button></EmptyState>;
  if (!state) return <Loading />;
  return <Deck />;
}

function Deck() {
  const { advanced } = usePreferences();
  const { state, error, clearError } = useDeck();
  useScrollRestore(true);
  const model = useMemo(() => fromOwner(state!), [state]);
  const deck = state!.deck;
  const edits = state!.document_edits.filter((e) => e.group_id == null || e.group_id === e.id).length;
  return (
    <ReadOnlyDeck
      model={model}
      header={
        <header className={styles.head}>
          <a className={styles.back} href={href({ page: "decks" })}><Icon name="back" /> Decks</a>
          <h1 className={styles.title}>{deck.name}</h1>
          <div className={styles.meta}>
            <Chip>{DECK_CATEGORY_LABELS[deck.category]}</Chip>
            {advanced && <span className={styles.rev}>r{deck.revision}</span>}
            {deck.visibility !== "private" && deck.slug && (
              <a href={href({ page: "shared", slug: deck.slug })}><Icon name={deck.visibility === "public" ? "globe" : "link"} /> {deck.visibility === "public" ? "Public" : "Unlisted"} page</a>
            )}
          </div>
          {error && <Banner tone="error" actions={<Button size="touch" variant="ghost" onClick={clearError}>Dismiss</Button>}>{error}</Banner>}
          {edits > 0 && (
            <Banner tone="agent">
              {edits} proposed brief {edits === 1 ? "edit waits" : "edits wait"} for you. <a className={styles.tap} href={href({ page: "inbox" })}>Review in the inbox</a>
            </Banner>
          )}
        </header>
      }
      decisions={(view) => <PhoneDecisions view={view} />}
    />
  );
}
