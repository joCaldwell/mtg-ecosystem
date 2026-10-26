import { Advanced } from "../../../app/Preferences.tsx";
// The History tab (spec §8.2): the decisions log is the deck's version history,
// and any entry can be undone. Undo writes a new entry rather than erasing one.

import { api, type LogEntry } from "../../../api/client.ts";
import { Eyebrow, Text, useToast } from "../../../ui/index.ts";
import { LogRow } from "../../decisions/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./HistoryPanel.module.css";

export function HistoryPanel() {
  const { log, deck } = useDeckState();
  const { deckId, run } = useDeck();
  const { toast } = useToast();

  async function undo(entry: LogEntry) {
    const result = await run(() => api.undo(deckId, entry.id));
    if (!result) return;
    if (result.skipped.length > 0)
      toast(`Undone, except what the deck had already moved past: ${result.skipped.join("; ")}.`, { duration: 12000 });
    else toast("Undone. The undo is in the log too.");
  }

  return (
    <>
      <section className={styles.section}>
        <Eyebrow>Decisions and versions</Eyebrow>
        <Advanced><Text size="sm" tone="muted">Revision numbers and agent review flags are visible below.</Text></Advanced>
        {log.length === 0 ? (
          <Text tone="muted" size="sm" as="p">
            Nothing yet. Every accepted or rejected proposal, edit and import lands here, with who made it.
          </Text>
        ) : (
          <div className={styles.list}>
            {log.map((entry) => (
              <LogRow key={entry.id} entry={entry} onUndo={(e) => void undo(e)}
                onToggleHidden={deck.publish_decisions ? (e) => void run(() => api.hideLogEntry(deckId, e.id, !e.hidden)) : undefined} />
            ))}
          </div>
        )}
      </section>
      <Text tone="faint" size="xs" as="p">
        Shows the latest 40 entries.{deck.publish_decisions ? " This deck publishes its decisions: hide an entry to keep it off the shared page." : ""}
      </Text>
    </>
  );
}
