import { FirstRun } from "./FirstRun.tsx";
import { useVoice } from "../../app/Preferences.tsx";
import { familyOrder } from "../../domain/familyOrder.ts";
import { Fragment, useState } from "react";
import { api, type DeckSummary } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, navigate } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { CardRef, ColorIdentity, DeckArt } from "../../mtg/index.ts";
import {
  Button,
  Chip,
  ErrorNote,
  Icon,
  Loading,
  Page,
  PageHeader,
  Row,
  Text,
  useNearView,
} from "../../ui/index.ts";
import styles from "./DecksPage.module.css";
import { ImportDialog } from "../import/index.ts";
import { NewDeckDialog } from "./NewDeckDialog.tsx";
import { PlayerStatsPanel } from "./PlayerStatsPanel.tsx";
import { DeckActions, type DeckAction } from "./DeckActions.tsx";
import { DeckActionDialog } from "./DeckActionDialog.tsx";


function Commanders({ deck }: { deck: DeckSummary }) {
  if (deck.commanders.length === 0) return <Text tone="faint">none yet</Text>;
  // A card name inside a row would swallow the click; send it to the deck instead.
  const open = () => navigate({ page: "deck", deckId: deck.id });
  return (
    <>
      {deck.commanders.map((name, i) => (
        <Fragment key={name}>
          {i > 0 && <Text tone="faint"> + </Text>}
          <CardRef name={name} onOpen={open} />
        </Fragment>
      ))}
    </>
  );
}

/** One deck on the roster. The name's link covers the whole card, as the
 * table row did; the commander links and the actions sit above it. */
function RosterCard({ deck, fork, onAction }: { deck: DeckSummary; fork: boolean; onAction: (action: DeckAction) => void }) {
  const [ref, near] = useNearView<HTMLLIElement>();
  const waiting = deck.pending_items + deck.pending_document_edits;
  return <li ref={ref} className={styles.card}>
    <DeckArt cards={deck.commander_art} identity={deck.color_identity} load={near}>
      <div className={styles.deckCard}>
        <div className={styles.cardHeading}>
          <h2><a className={styles.deckLink} href={href({ page: "deck", deckId: deck.id })}>{deck.name}</a></h2>
          <DeckActions deck={deck} onAction={onAction} />
        </div>
        <div className={styles.commander}><Commanders deck={deck} /></div>
        {fork && deck.parent_name && <Text tone="faint" size="xs">Fork of {deck.parent_name}</Text>}
        <div className={styles.cardStats}>
          <ColorIdentity colors={deck.color_identity} />
          <Chip className={styles.lifecycle} data-lifecycle={deck.category}>{DECK_CATEGORY_LABELS[deck.category]}</Chip>
          <Text mono tone={deck.card_count === 100 ? "muted" : "warn"}>{deck.card_count}/100</Text>
          {deck.visibility !== "private" && <span className={styles.shared} role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"} title={deck.visibility === "public" ? "Public" : "Unlisted"}><Icon name={deck.visibility === "public" ? "globe" : "link"} size={12} /></span>}
          {waiting > 0 && <Chip tone={deck.category === "archived" ? "neutral" : "agent"}>{waiting} {deck.category === "archived" ? "paused" : "waiting"}</Chip>}
        </div>
      </div>
    </DeckArt>
  </li>;
}

/** The roster, spec §9: every deck, how far along it is, and what is waiting on the owner. */
export function DecksPage() {
  const voice = useVoice();
  const [importing, setImporting] = useState(false);
  const decks = useQuery(() => api.listDecks(), []);
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<{ deck: DeckSummary; action: DeckAction } | null>(null);
  const rows = familyOrder(decks.data ?? []);

  return (
    <Page
      width="full"
      header={
        <PageHeader
          title="All decks"
          subtitle={
            // Lifecycle words mean nothing before the first deck exists.
            decks.data && rows.length === 0
              ? "Your Commander decks, from first idea to the list you play."
              : voice("Your Commander decks, from first idea to the list you play. Archived decks stay read-only.", "Theorycrafted cards are open to agents. Paper cards need your approval. Archived decks are read-only.")
          }
          actions={
            <Row gap={4} wrap><Button onClick={() => setCreating(true)} aria-haspopup="dialog">New deck</Button>
            <Button onClick={() => setImporting(true)} aria-haspopup="dialog">Import deck</Button></Row>
          }
        />
      }
    >
      <div className={styles.body}>
        {decks.error && !decks.data && <ErrorNote>{decks.error}</ErrorNote>}
        {decks.loading && <Loading />}
        {decks.data &&
          (rows.length === 0 ? (
            <FirstRun onCreate={() => setCreating(true)} onImport={() => setImporting(true)} />
          ) : (
            <ul className={styles.roster}>
              {rows.map(({ deck, fork }) => <RosterCard key={deck.id} deck={deck} fork={fork} onAction={action => setEditing({ deck, action })} />)}
            </ul>
          ))}
        {rows.length > 0 && <PlayerStatsPanel />}
      </div>
      {creating && <NewDeckDialog onClose={() => setCreating(false)} />}
      {importing && <ImportDialog onClose={() => setImporting(false)} />}
      {editing && <DeckActionDialog key={`${editing.deck.id}-${editing.action}`} {...editing} onClose={() => setEditing(null)} onSaved={decks.reload} />}
    </Page>
  );
}
