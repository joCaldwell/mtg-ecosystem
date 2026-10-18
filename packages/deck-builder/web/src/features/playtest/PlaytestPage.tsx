import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, navigate } from "../../app/routes.ts";
import { Button, EmptyState, Icon, Loading, Page } from "../../ui/index.ts";
import { NewGameDialog } from "./NewGameDialog.tsx";
import { PlaytestTable } from "./PlaytestTable.tsx";
import { usePlaytest } from "./usePlaytest.ts";
import styles from "./PlaytestTable.module.css";

export function PlaytestPage({ deckId }: { deckId: number }) {
  const deck = useQuery(() => api.getDeck(deckId), [deckId]),
    controller = usePlaytest(deckId);
  if (deck.loading || (controller.loading && !controller.view))
    return <Loading />;
  if (!deck.data)
    return (
      <Page>
        <EmptyState title="The deck could not be loaded">
          {deck.error}
          <Button onClick={deck.reload}>Retry</Button>
        </EmptyState>
      </Page>
    );
  const state = deck.data.state;
  if (!controller.view)
    // No game yet: an empty table under the new-game dialog, so starting one
    // feels like sitting down rather than filling in a form. Closing it
    // leaves the way it came.
    return (
      <div className={styles.page}>
        <header className={styles.header}>
          <div className={styles.identity}>
            <a
              className={styles.deckLink}
              href={href({ page: "deck", deckId })}
              aria-label="Back to deck overview"
              title="Deck overview"
            >
              <Icon name="back" />
            </a>
            <div className={styles.title}>
              <h1>{state.deck.name}</h1>
              <span>
                Playtest · <span className={styles.mono}>r{state.deck.revision}</span>
              </span>
            </div>
          </div>
        </header>
        <div className={styles.workspace}>
          <div className={styles.table} />
          <aside className={styles.panel} />
        </div>
        <NewGameDialog
          deckName={state.deck.name}
          revision={state.deck.revision}
          commanders={state.cards.filter((c) => c.role === "commander" && !c.maybe)}
          replaces={controller.error ? "unreadable" : undefined}
          error={controller.error}
          busy={controller.pending > 0}
          onRetry={() => void controller.reload()}
          onStart={(mode) => void controller.start(mode, !!controller.error)}
          onClose={() => navigate({ page: "deck", deckId })}
        />
      </div>
    );
  return (
    <PlaytestTable
      controller={controller}
      deck={state}
      onDeckReload={deck.reload}
    />
  );
}
