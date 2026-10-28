import { useVoice } from "../../app/Preferences.tsx";
import { useEffect, useRef, useState } from "react";
import { href, navigate, useScrollRestore } from "../../app/routes.ts";
import { useMediaQuery, PANEL_QUERY } from "../../app/breakpoints.ts";
import { useInbox } from "../../app/Inbox.tsx";
import { Button, EmptyState, Loading, Page, PageHeader, SplitPane, Text, useHotkeys } from "../../ui/index.ts";
import { DocumentEditCard } from "../decisions/index.ts";
import { inboxEntries } from "./entries.ts";
import { InboxEntry } from "./InboxEntry.tsx";
import { ProposalHistory } from "./ProposalHistory.tsx";
import { ProposalSummary } from "./ProposalSummary.tsx";
import styles from "./InboxPage.module.css";

/**
 * Everything waiting on the owner, across decks (spec §8.4, §11). Decide a
 * whole proposal here or review individual changes among the cards in the deck.
 */
export function InboxPage({ entryKey }: { entryKey?: string }) {
  const tablet = useMediaQuery(PANEL_QUERY);
  const voice = useVoice();
  const { inbox, apply } = useInbox();
  const [history, setHistory] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  const entries = inbox ? inboxEntries(inbox) : [];
  const found = entries.findIndex((e) => e.key === (entryKey ?? selectedKey));
  // Where the selection was, so one that disappears hands over to its neighbour.
  const lastIndex = useRef(0);
  const index = found >= 0 ? found : Math.min(lastIndex.current, entries.length - 1);
  const selected = entryKey && found < 0 ? null : entries[index] ?? null;
  const detailOpen = entryKey != null;
  useScrollRestore(inbox != null);
  useEffect(() => {
    if (index >= 0) lastIndex.current = index;
  }, [index]);

  const step = (by: number) => {
    const next = entries[Math.min(Math.max(index + by, 0), entries.length - 1)];
    if (next) {
      setSelectedKey(next.key);
      if (detailOpen) navigate({ page: "inbox", entry: next.key });
    }
  };
  useHotkeys({
    j: () => step(1),
    k: () => step(-1),
    enter: (e) => {
      if (!selected) return;
      // Native controls own Enter, including disclosure summaries.
      const target = e.target instanceof HTMLElement ? e.target.closest("button, a, summary") : null;
      if (target) return;
      if (tablet && !detailOpen) {
        e.preventDefault();
        navigate({ page: "inbox", entry: selected.key });
      } else if (selected.type === "proposal") {
        e.preventDefault();
        navigate({ page: "deck", deckId: selected.proposal.deck_id, review: true, proposal: selected.proposal.id });
      }
    },
  });

  const header = (
    <>
      <a className={styles.back} href={href({ page: "inbox" })}>← Back to inbox</a>
      <PageHeader
        title={tablet && detailOpen && selected ? (selected.type === "proposal" ? "Proposal" : "Document edit") : "Inbox"}
        meta={!(tablet && detailOpen) && inbox != null && inbox.unreviewed > 0 ?
          <Text tone="muted" size="sm">{inbox.unreviewed} {inbox.unreviewed === 1 ? "decision" : "decisions"} unreviewed by agents</Text> : undefined}
        actions={inbox != null && !(tablet && detailOpen) ?
          <Button variant="ghost" aria-expanded={history} onClick={() => setHistory(!history)}>{history ? "Hide proposal history" : "Show proposal history"}</Button> : undefined}
      />
    </>
  );

  if (!inbox) {
    return (
      <div className={styles.page} data-detail={detailOpen || undefined}>
        <Page scroll={false} header={header}><Loading /></Page>
      </div>
    );
  }

  return (
    <div className={styles.page} data-detail={detailOpen || undefined}>
      <Page scroll={false} header={header}>
        {history && <div className={styles.history}><ProposalHistory /></div>}
        <SplitPane
          className={styles.panes}
          list={entries.length === 0 && tablet ? <EmptyState title="Nothing is waiting on you">There are no proposed changes to review.</EmptyState> : entries.map((entry) => (
            <InboxEntry
              key={entry.key}
              entry={entry}
              selected={entry.key === selected?.key}
              href={href({ page: "inbox", entry: entry.key })}
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
                <EmptyState title={detailOpen ? "This item is no longer waiting" : "Nothing is waiting on you"}>
                  {voice("There are no proposed changes to review. Keep building your decks; your decisions stay in each deck’s History.", "Your decisions and reasons are queued. An agent reads them the next time it starts a session, whether or not it was running when you decided.")}
                </EmptyState>
              </div>
            )
          }
        />
      </Page>
    </div>
  );
}
