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
  Eyebrow,
  Icon,
  Loading,
  Page,
  PageHeader,
  Text,
} from "../../ui/index.ts";
import { PlaytestTable } from "./PlaytestTable.tsx";
import { usePlaytest } from "./usePlaytest.ts";
import styles from "./PlaytestPage.module.css";

const MODES: Array<{ value: Mode; label: string; description: string }> = [
  {
    value: "commander",
    label: "Commander mulligans",
    description:
      "First mulligan free. After that, choose cards to put on the bottom when you keep.",
  },
  {
    value: "practice",
    label: "Practice redraws",
    description:
      "Try unlimited seven-card hands. No cards to bottom and no mulligan penalty.",
  },
];

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
      <Page
        header={
          <PageHeader
            title="Playtest"
            subtitle={
              <>
                {deck.data.state.deck.name} ·{" "}
                <Text mono>r{deck.data.state.deck.revision}</Text>
              </>
            }
          />
        }
      >
        <div className={styles.setup}>
          <a className={styles.back} href={href({ page: "deck", deckId })}>
            <Icon name="back" /> Back to deck
          </a>
          {controller.error && (
            <Banner tone="error">
              {controller.error}
              <Button onClick={() => void controller.reload()}>
                Retry resume
              </Button>
            </Banner>
          )}
          <section className={styles.panel} aria-labelledby="opening-title">
            <div className={styles.intro}>
              <Eyebrow>New session</Eyebrow>
              <h2 id="opening-title" className={styles.title}>
                Test an opening hand
              </h2>
              <p className={styles.description}>
                Explore your opening hands and play through the early turns. Start
                with a shuffled snapshot of{" "}
                <Text mono>r{deck.data.state.deck.revision}</Text>.
              </p>
            </div>
            <fieldset className={styles.modes} disabled={controller.pending > 0}>
              <legend className={styles.legend}>Opening hand mode</legend>
              <div className={styles.options}>
                {MODES.map((option) => (
                  <label className={styles.option} key={option.value}>
                    <input
                      className={styles.radio}
                      type="radio"
                      name="opening-mode"
                      value={option.value}
                      checked={mode === option.value}
                      onChange={() => setMode(option.value)}
                      aria-labelledby={`mode-${option.value}-label`}
                      aria-describedby={`mode-${option.value}-description`}
                    />
                    <span className={styles.optionText}>
                      <span
                        id={`mode-${option.value}-label`}
                        className={styles.optionTitle}
                      >
                        {option.label}
                      </span>
                      <span
                        id={`mode-${option.value}-description`}
                        className={styles.optionDescription}
                      >
                        {option.description}
                      </span>
                    </span>
                  </label>
                ))}
              </div>
            </fieldset>
            <div className={styles.actions}>
              <Button
                className={styles.start}
                variant="primary"
                disabled={controller.pending > 0}
                onClick={() =>
                  controller.error ? setReplace(true) : void controller.start(mode)
                }
              >
                {controller.pending > 0 ? "Starting session…" : "Start new session"}
              </Button>
              <p className={styles.saveHint}>Your session saves as you play.</p>
            </div>
          </section>
          <dl className={styles.notes}>
            <div>
              <dt>Command zone</dt>
              <dd>Commanders start here, outside the library.</dd>
            </div>
            <div>
              <dt>Maybe list</dt>
              <dd>Stays outside the playtest.</dd>
            </div>
            <div>
              <dt>Manual play</dt>
              <dd>You move cards and resolve effects.</dd>
            </div>
          </dl>
        </div>
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
