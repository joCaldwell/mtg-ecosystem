import { FirstRun } from "./FirstRun.tsx";
import { familyOrder } from "../../domain/familyOrder.ts";
import { Fragment, useState } from "react";
import { api, type DeckSummary } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, navigate } from "../../app/routes.ts";
import { CardRef, DeckCard } from "../../mtg/index.ts";
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
  const waiting = deck.pending_items + deck.pending_document_edits;
  return <DeckCard name={deck.name} href={href({ page: "deck", deckId: deck.id })}
    art={deck.commander_art} identity={deck.color_identity} category={deck.category} count={deck.card_count}
    commanders={<Commanders deck={deck} />} lineage={fork && deck.parent_name ? `Fork of ${deck.parent_name}` : undefined}
    actions={<DeckActions deck={deck} onAction={onAction} />}>
    {deck.visibility !== "private" && <span className={styles.shared} role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"} title={deck.visibility === "public" ? "Public" : "Unlisted"}><Icon name={deck.visibility === "public" ? "globe" : "link"} size={12} /></span>}
    {waiting > 0 && <Chip tone={deck.category === "archived" ? "neutral" : "agent"}>{waiting} {deck.category === "archived" ? "paused" : "waiting"}</Chip>}
  </DeckCard>;
}

/** The roster, spec §9: every deck, how far along it is, and what is waiting on the owner. */
export function DecksPage() {
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
