import { useInbox } from "../../app/Inbox.tsx";
import { deckWaitingState } from "../../domain/inbox.ts";
import { DeckFilters } from "./DeckFilters.tsx";
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
  EmptyState,
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
  const { deckWaiting } = useInbox();
  const { count: waiting, pending } = deckWaitingState(deck, deckWaiting);
  return <DeckCard roster name={deck.name} href={href({ page: "deck", deckId: deck.id })}
    art={deck.commander_art} identity={deck.color_identity} category={deck.category} count={deck.card_count}
    commanders={<Commanders deck={deck} />} lineage={fork && deck.parent_name ? `Fork of ${deck.parent_name}` : undefined}
    actions={<DeckActions deck={deck} onAction={onAction} />}>
    {deck.visibility !== "private" && <span className={styles.shared} role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"} title={deck.visibility === "public" ? "Public" : "Unlisted"}><Icon name={deck.visibility === "public" ? "globe" : "link"} size={12} /></span>}
    {pending && <Chip data-deck-waiting={deck.category !== "archived" || undefined} tone={deck.category === "archived" ? "neutral" : "agent"}>{waiting !== null && `${waiting} `}{deck.category === "archived" ? "paused" : "waiting"}</Chip>}
  </DeckCard>;
}

/** The roster, spec §9: every deck, how far along it is, and what is waiting on the owner. */
export function DecksPage() {
  const [scrolled, setScrolled] = useState(false);
  const [importing, setImporting] = useState(false);
  const decks = useQuery(() => api.listDecks(), []);
  const [creating, setCreating] = useState(false);
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState("all");
  const [editing, setEditing] = useState<{ deck: DeckSummary; action: DeckAction } | null>(null);
  const rows = familyOrder(decks.data ?? []);
  const query = search.trim().toLocaleLowerCase();
  const shown = rows.filter(({ deck }) => (category === "all" || deck.category === category)
    && (!query || [deck.name, ...deck.commanders].some(name => name.toLocaleLowerCase().includes(query))));

  return (
    <Page
      width="lg"
      className={`${styles.page} ${scrolled ? styles.scrolled : ""}`}
      onScroll={event => setScrolled(event.currentTarget.scrollTop > 0)}
      header={
        <PageHeader
          title="My decks"
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
            <div className={styles.collection}>
              <DeckFilters search={search} category={category} shown={shown.length} total={rows.length}
                onSearch={setSearch} onCategory={setCategory} />
              {shown.length ? <ul className={styles.roster}>
                {shown.map(({ deck, fork }) => <RosterCard key={deck.id} deck={deck} fork={fork} onAction={action => setEditing({ deck, action })} />)}
              </ul> : <EmptyState title="No decks match">Try another name or lifecycle.</EmptyState>}
            </div>
          ))}
        {rows.length > 0 && <PlayerStatsPanel />}
      </div>
      {creating && <NewDeckDialog onClose={() => setCreating(false)} />}
      {importing && <ImportDialog onClose={() => setImporting(false)} />}
      {editing && <DeckActionDialog key={`${editing.deck.id}-${editing.action}`} {...editing} onClose={() => setEditing(null)} onSaved={decks.reload} />}
    </Page>
  );
}
