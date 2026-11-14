// The Review tab (spec §8): what is waiting on the owner, with the projected
// deck above it. One proposal is open at a time, the one holding the review
// cursor; the others are one line each and open when clicked or reached with
// J/K. Decision happens here, in the list (review mode) and in the banner
// through the same `Review` object.

import { useState } from "react";
import { plainText } from "../../../domain/inbox.ts";
import { proposalQueue } from "../../../domain/review.ts";
import { changesSince, staleNote, staleTouches } from "../../../domain/staleChanges.ts";
import { InlineText } from "../../../mtg/index.ts";
import { Button, EmptyState, Explainer, Text } from "../../../ui/index.ts";
import { ProposalEyebrow } from "../../decisions/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import type { DeckViewModel } from "../useDeckView.ts";
import type { Review } from "../useReview.tsx";
import { ReviewDelta } from "./ReviewDelta.tsx";
import { ReviewUnitCard } from "./ReviewUnitCard.tsx";
import styles from "./ReviewPanel.module.css";

export function ReviewPanel({ view, review }: { view: DeckViewModel; review: Review }) {
  const state = useDeckState();
  const ui = useDeckUi();
  // Reason drafts by decision, so moving between proposals keeps them.
  const [reasons, setReasons] = useState<Record<string, string>>({});

  if (view.units.length === 0) return <EmptyState title="No pending proposals" />;
  if (state.deck.category === "archived") return <Text tone="muted">Pending proposals are kept. To accept or reject them, switch this deck to Paper or Theorycrafted.</Text>;

  // The same unit useReview treats as current, also outside review mode.
  const focus = Math.min(ui.cursor, view.units.length - 1);
  const focused = view.units[focus]?.proposalId;
  const queue = proposalQueue(view.units);

  return (
    <>
      {/* The projection covers every open proposal, so it appears once. */}
      <ReviewDelta />
      <div className={styles.queue}>
        {queue.map((entry) => {
          const proposal = state.proposals.find((p) => p.id === entry.proposalId);
          if (!proposal) return null;
          const position = `${entry.position} of ${entry.of}`;
          if (proposal.id !== focused) {
            return (
              <div key={proposal.id} className={styles.collapsed} data-proposal={proposal.id}>
                <button type="button" className={styles.line} aria-expanded={false} onClick={() => ui.setCursor(entry.firstUnit)}>
                  <span className={styles.position}>{position}</span>
                  <span className={styles.lineTitle}>{plainText(proposal.summary)}</span>
                  <span className={styles.lineMeta}>
                    {proposal.author_label || proposal.author} · {entry.changes} {entry.changes === 1 ? "change" : "changes"}
                  </span>
                </button>
              </div>
            );
          }
          const pending = proposal.items.filter((i) => i.status === "pending");
          return (
            <div key={proposal.id} className={styles.proposal} data-proposal={proposal.id}>
              <div className={styles.head}>
                <ProposalEyebrow proposal={proposal} counter={position} />
                <p className={styles.summary}>
                  <InlineText text={proposal.summary} />
                </p>
              </div>
              {/* Amber only when the changes since touch this proposal's own
                  cards; otherwise the same sentence, quietly. Accepting still
                  asks for the server's stale acknowledgement either way. */}
              {proposal.stale && <Text tone={staleTouches(state.log, proposal.base_revision, pending.map(i => i.oracle_id)) ? "warn" : "muted"}>
                {staleNote(changesSince(state.log, proposal.base_revision))}
              </Text>}
              {proposal.supersedes.length > 0 && <Text tone="faint">Replaces proposals {proposal.supersedes.join(", ")}.</Text>}
              <div className={styles.units}>
                {view.units.map((unit, index) =>
                  unit.proposalId === proposal.id ? (
                    <ReviewUnitCard
                      key={unit.key}
                      unit={unit}
                      index={index}
                      review={review}
                      focused={index === focus}
                      reason={reasons[unit.key] ?? ""}
                      onReason={(text) => setReasons((all) => ({ ...all, [unit.key]: text }))}
                    />
                  ) : null,
                )}
              </div>
            </div>
          );
        })}
      </div>
      {/* Room to scroll the last proposals up to the panel's top edge, so
          the one above is never left cut by the navigation. */}
      {ui.reviewing && <div className={styles.tail} aria-hidden="true" />}
      {!ui.reviewing && (
        <div className={styles.start}>
          <Button variant="agent" onClick={ui.startReview} kbd="↵">
            Review in the list
          </Button>
          <Explainer title="Proposals" label="How proposals work">
            <p>Review shows proposed changes among the cards already in the list. Accept or reject one decision, a linked group, or all remaining changes.</p>
            <p>Your optional reason stays in History for agents to read. A stale proposal asks you to acknowledge changes before accepting.</p>
          </Explainer>
        </div>
      )}
    </>
  );
}
