import { familyOrder } from "../../domain/familyOrder.ts";
import { Fragment, useState } from "react";
import { api, type DeckSummary } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { navigate } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { CardRef, ColorIdentity } from "../../mtg/index.ts";
import {
  Button,
  Chip,
  ClickableRow,
  EmptyState,
  ErrorNote,
  Icon,
  Loading,
  Page,
  PageHeader,
  Table,
  Text,
} from "../../ui/index.ts";
import styles from "./DecksPage.module.css";
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

/** The roster, spec §9: every deck, how far along it is, and what is waiting on the owner. */
export function DecksPage() {
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
          subtitle="Theorycrafted cards are open to agents. Paper cards need your approval. Legacy decks are read-only."
          actions={
            <Button onClick={() => setCreating(true)} aria-haspopup="dialog">
              New deck
            </Button>
          }
        />
      }
    >
      <div className={styles.body}>
        {decks.error && !decks.data && <ErrorNote>{decks.error}</ErrorNote>}
        {decks.loading && <Loading />}
        {decks.data &&
          (rows.length === 0 ? (
            <EmptyState title="No decks yet">
              Create one with New deck, or ask a connected agent to build a Theorycrafted deck.
            </EmptyState>
          ) : (
            <div className={styles.tableWrap}>
              <Table>
                <thead>
                  <tr>
                    <th>Deck</th>
                    <th>Commander</th>
                    <th>Identity</th>
                    <th>Lifecycle</th>
                    <th>Cards</th>
                    <th>Inbox</th>
                    <th aria-label="Actions" className={styles.actions} />
                  </tr>
                </thead>
                <tbody>
                  {rows.map(({ deck, fork }) => {
                    const waiting = deck.pending_items + deck.pending_document_edits;
                    return (
                      <ClickableRow
                        key={deck.id}
                        onClick={() => navigate({ page: "deck", deckId: deck.id })}
                      >
                        <td>
                          {fork && <Text tone="faint">└ </Text>}
                          <Text weight="medium">{deck.name}</Text>
                          {deck.visibility !== "private" && (
                            <span className={styles.shared} role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"} title={deck.visibility === "public" ? "Public" : "Unlisted: anyone with the link"}>
                              {" "}<Icon name={deck.visibility === "public" ? "globe" : "link"} size={12} />
                            </span>
                          )}
                          {fork && deck.parent_name && (
                            <Text tone="faint" size="sm">
                              {" "}
                              fork of {deck.parent_name}
                            </Text>
                          )}
                        </td>
                        <td className={styles.commander}>
                          <Commanders deck={deck} />
                        </td>
                        <td>
                          <ColorIdentity colors={deck.color_identity} />
                        </td>
                        <td>
                          <Chip className={styles.lifecycle} data-lifecycle={deck.category}>
                            {DECK_CATEGORY_LABELS[deck.category]}
                          </Chip>
                        </td>
                        <td>
                          <Text mono tone={deck.card_count === 100 ? "muted" : "warn"}>
                            {deck.card_count}/100
                          </Text>
                        </td>
                        <td>{waiting > 0 && <Chip tone={deck.category === "legacy" ? "neutral" : "agent"}>{waiting} {deck.category === "legacy" ? "paused" : "waiting"}</Chip>}</td>
                        <td className={styles.actions} onClick={(e) => e.stopPropagation()}>
                          <DeckActions deck={deck} onAction={(action) => setEditing({ deck, action })} />
                        </td>
                      </ClickableRow>
                    );
                  })}
                </tbody>
              </Table>
            </div>
          ))}
        <PlayerStatsPanel />
      </div>
      {creating && <NewDeckDialog onClose={() => setCreating(false)} />}
      {editing && <DeckActionDialog key={`${editing.deck.id}-${editing.action}`} {...editing} onClose={() => setEditing(null)} onSaved={decks.reload} />}
    </Page>
  );
}
