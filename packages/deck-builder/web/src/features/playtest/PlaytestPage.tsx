import { useState } from "react";
import { api } from "../../api/client.ts";
import type { Mode } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import {
  Banner,
  Button,
  ConfirmDialog,
  EmptyState,
  Loading,
  Page,
  Segmented,
} from "../../ui/index.ts";
import { PlaytestTable } from "./PlaytestTable.tsx";
import { usePlaytest } from "./usePlaytest.ts";
export function PlaytestPage({ deckId }: { deckId: number }) {
  const deck = useQuery(() => api.getDeck(deckId), [deckId]),
    controller = usePlaytest(deckId);
  const [replace, setReplace] = useState(false);
  const [mode, setMode] = useState<Mode>("commander");
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
  if (!controller.view)
    return (
      <Page>
        <h1>Playtest · {deck.data.state.deck.name}</h1>
        {controller.error && (
          <Banner tone="error">
            {controller.error}
            <Button onClick={() => void controller.reload()}>
              Retry resume
            </Button>
          </Banner>
        )}
        <EmptyState title="Test an opening hand">
          Start a shuffled snapshot of r{deck.data.state.deck.revision}.
          Commanders stay in the command zone; the maybe list stays outside the
          table.
          <Segmented
            label="Opening hand mode"
            value={mode}
            onChange={setMode}
            options={[
              { value: "commander", label: "Commander mulligans" },
              { value: "practice", label: "Practice redraws" },
            ]}
          />
          <Button
            variant="primary"
            disabled={controller.pending > 0}
            onClick={() =>
              controller.error ? setReplace(true) : void controller.start(mode)
            }
          >
            Start new session
          </Button>
          <a href={href({ page: "deck", deckId })}>Back to deck</a>
        </EmptyState>
        {replace && (
          <ConfirmDialog
            title="Replace the saved playtest?"
            confirmLabel="Start a new session"
            tone="danger"
            onClose={() => setReplace(false)}
            onConfirm={() => {
              setReplace(false);
              void controller.start(mode, true);
            }}
          >
            <p>
              The saved table could not be loaded. Replacing it discards that
              game state. Saved memories and deck tags remain.
            </p>
          </ConfirmDialog>
        )}
      </Page>
    );
  return (
    <PlaytestTable
      controller={controller}
      deck={deck.data.state}
      onDeckReload={deck.reload}
    />
  );
}
