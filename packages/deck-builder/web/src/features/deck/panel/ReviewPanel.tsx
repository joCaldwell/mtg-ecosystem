// The Review tab (spec §8): what is waiting on the owner, one card per
// decision, with the projected deck above it. Decision happens here, in the list
// (review mode) and in the banner through the same `Review` object.

import { ago } from "../../../domain/time.ts";
import { InlineText } from "../../../mtg/index.ts";
import { Button, EmptyState, Explainer, Eyebrow, Text } from "../../../ui/index.ts";
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

  if (view.units.length === 0) return <EmptyState title="No pending proposals" />;
  if (state.deck.category === "archived") return <Text tone="muted">Pending proposals are kept. To accept or reject them, switch this deck to Paper or Theorycrafted.</Text>;

  // Units come oldest proposal first, so the proposals do too.
  const proposals = state.proposals
    .filter((p) => view.units.some((u) => u.proposalId === p.id))
    .sort((a, b) => a.id - b.id);

  return (
    <>
      {proposals.map((proposal, n) => (
        <div key={proposal.id} className={styles.proposal} data-proposal={proposal.id}>
          <div className={styles.head}>
            <Eyebrow className={styles.eyebrow}>
              <span className={styles.id}>Proposal {proposal.id}</span>
              <span>
                {proposal.author_display ?? proposal.author} · {ago(proposal.created_at)}
              </span>
            </Eyebrow>
            <p className={styles.summary}>
              <InlineText text={proposal.summary} />
            </p>
          </div>
          {proposal.stale && <Text tone="warn">The deck changed since this proposal (based on r{proposal.base_revision ?? "unknown"}, current r{proposal.current_revision}). Acceptance requires reviewing and acknowledging those changes.</Text>}
          {proposal.supersedes.length > 0 && <Text tone="faint">Replaces proposals {proposal.supersedes.join(", ")}.</Text>}
          {/* The projection covers every open proposal, so it appears once. */}
          {n === 0 && <ReviewDelta />}
          <div className={styles.units}>
            {view.units.map((unit, index) =>
              unit.proposalId === proposal.id ? (
                <ReviewUnitCard key={unit.key} unit={unit} index={index} review={review} />
              ) : null,
            )}
          </div>
        </div>
      ))}
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
