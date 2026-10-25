import { useEffect, useRef, useState } from "react";
import { navigate } from "../../app/routes.ts";
import { useInbox } from "../../app/Inbox.tsx";
import { Button, EmptyState, Loading, PageHeader, SplitPane, Text, useHotkeys } from "../../ui/index.ts";
import { DocumentEditCard } from "../rulings/index.ts";
import { inboxEntries } from "./entries.ts";
import { InboxEntry } from "./InboxEntry.tsx";
import { ProposalHistory } from "./ProposalHistory.tsx";
import { ProposalSummary } from "./ProposalSummary.tsx";
import styles from "./InboxPage.module.css";

/**
 * Everything waiting on the owner, across decks (spec §8.4, §11). A queue that
 * hands proposals off to the deck, where each change is ruled among the cards
 * around it. Document edits are ruled here, because a text diff is the whole
 * decision.
 */
export function InboxPage() {
  const { inbox, apply } = useInbox();
  const [history, setHistory] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  const entries = inbox ? inboxEntries(inbox) : [];
  const found = entries.findIndex((e) => e.key === selectedKey);
  // Where the selection was, so one that disappears hands over to its neighbour.
  const lastIndex = useRef(0);
  const index = found >= 0 ? found : Math.min(lastIndex.current, entries.length - 1);
  const selected = entries[index] ?? null;
  useEffect(() => {
    if (index >= 0) lastIndex.current = index;
  }, [index]);

  const step = (by: number) => {
    const next = entries[Math.min(Math.max(index + by, 0), entries.length - 1)];
    if (next) setSelectedKey(next.key);
  };
  useHotkeys({
    j: () => step(1),
    k: () => step(-1),
    enter: (e) => {
      if (!selected || selected.type !== "proposal") return;
      // Enter on a button or link already does what that control says.
      const target = e.target instanceof HTMLElement ? e.target.closest("button, a") : null;
      if (target && !target.matches("[data-entry][aria-current='true']")) return;
      e.preventDefault();
      navigate({ page: "deck", deckId: selected.proposal.deck_id, review: true });
    },
  });

  const header = (
    <PageHeader
      title="Inbox"
      subtitle="Everything an agent needs your decision on, across all decks. Nothing here has been applied."
    />
  );

  if (!inbox) {
    return (
      <div className={styles.page}>
        {header}
        <Loading />
      </div>
    );
  }

  return (
    <div className={styles.page}>
      {header}
      <Button variant="ghost" onClick={() => setHistory(!history)}>{history ? "Hide proposal history" : "Show proposal history"}</Button>
      {history && <ProposalHistory />}
      <SplitPane
        list={entries.map((entry) => (
          <InboxEntry
            key={entry.key}
            entry={entry}
            selected={entry.key === selected?.key}
            onSelect={() => setSelectedKey(entry.key)}
          />
        ))}
        detail={
          selected?.type === "proposal" ? (
            <ProposalSummary key={selected.key} proposal={selected.proposal} />
          ) : selected?.type === "edit" ? (
            <DocumentEditCard key={selected.key} edit={selected.edit} showDeck onRuled={(result) => apply(result.inbox)} />
          ) : (
            <div className={styles.empty}>
              <EmptyState title="Nothing is waiting on you.">
                Your rulings and reasons are queued. An agent reads them the next time it starts a session,
                whether or not it was running when you ruled.
              </EmptyState>
              {inbox.unreviewed > 0 && (
                <Text tone="muted" size="sm">
                  {inbox.unreviewed} of your rulings {inbox.unreviewed === 1 ? "has" : "have"} not been read by an agent
                  yet.
                </Text>
              )}
            </div>
          )
        }
      />
    </div>
  );
}
