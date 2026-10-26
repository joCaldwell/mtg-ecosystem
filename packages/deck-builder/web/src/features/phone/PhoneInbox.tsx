// The inbox on a phone: card proposals open their deck, where each change is
// drawn in place; brief and profile edits are ruled right here as diffs.

import { useState } from "react";
import { useInbox } from "../../app/Inbox.tsx";
import { navigate } from "../../app/routes.ts";
import { EmptyState, Loading, Text } from "../../ui/index.ts";
import { InboxEntry, inboxEntries } from "../inbox/index.ts";
import { DocumentEditCard } from "../rulings/index.ts";
import styles from "./PhoneInbox.module.css";

export function PhoneInbox() {
  const { inbox, apply } = useInbox();
  const [openKey, setOpenKey] = useState<string | null>(null);
  if (!inbox) return <Loading />;
  const entries = inboxEntries(inbox);
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Inbox</h1>
      <Text size="sm" tone="muted" as="p">Everything an agent needs your decision on. Nothing here has been applied.</Text>
      {entries.length === 0 ? (
        <EmptyState title="Nothing is waiting on you.">Ask your agent for changes; what it proposes appears here without a reload.</EmptyState>
      ) : (
        <ul className={styles.list}>
          {entries.map((entry) => (
            <li key={entry.key} className={styles.item}>
              <InboxEntry
                entry={entry}
                selected={openKey === entry.key}
                onSelect={() => entry.type === "proposal"
                  ? navigate({ page: "deck", deckId: entry.proposal.deck_id, review: true })
                  : setOpenKey(openKey === entry.key ? null : entry.key)}
              />
              {entry.type === "edit" && openKey === entry.key && (
                <DocumentEditCard edit={entry.edit} showDeck onRuled={(result) => { apply(result.inbox); setOpenKey(null); }} />
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
