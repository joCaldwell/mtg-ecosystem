// The strip under the toolbar that says whether anything is waiting on the
// owner (spec §8.2): an agent's proposal, or the review in progress. Archived
// decks show an archive notice instead.

import { useEffect, useState } from "react";
import { ago } from "../../domain/time.ts";
import type { ReviewUnit } from "../../domain/review.ts";
import { InlineText } from "../../mtg/index.ts";
import { Banner, Button, Dot, Kbd, Text } from "../../ui/index.ts";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import type { Review } from "./useReview.tsx";
import { ConfirmStrip } from "./dialogs/ConfirmStrip.tsx";
import styles from "./ProposalBanner.module.css";

export function ProposalBanner({ units, review, confirmingAll, setConfirmingAll }: {
  units: ReviewUnit[];
  review: Review;
  /** "Accept all left" asks first; ⇧A in DeckPage opens the same question. */
  confirmingAll: boolean;
  setConfirmingAll: (on: boolean) => void;
}) {
  const { proposals } = useDeckState();
  const ui = useDeckUi();

  // How many decisions the review began with, so "X of Y left" has a Y that
  // does not shrink as the owner rules.
  const [began, setBegan] = useState(units.length);
  useEffect(() => {
    if (!ui.reviewing) setBegan(units.length);
  }, [ui.reviewing, units.length]);

  if (units.length === 0) return null;

  if (ui.reviewing)
    return (
      <Banner
        tone="agent"
        actions={
          confirmingAll ? (
            <ConfirmStrip
              tone="accept"
              confirmLabel="Accept"
              onCancel={() => setConfirmingAll(false)}
              onConfirm={() => { setConfirmingAll(false); void review.acceptAll(); }}
            >
              Accept {units.length} {units.length === 1 ? "decision" : "decisions"}?
            </ConfirmStrip>
          ) : (
            <>
              <Text tone="faint" size="xs" className={styles.keys}>
                <Kbd>J</Kbd> <Kbd>K</Kbd> move · <Kbd>A</Kbd> accept · <Kbd>R</Kbd> reject
              </Text>
              <Button size="sm" kbd="⇧A" onClick={() => setConfirmingAll(true)}>
                Accept all left
              </Button>
              <Button size="sm" variant="ghost" kbd="Esc" onClick={ui.stopReview}>
                Done
              </Button>
            </>
          )
        }
      >
        <div className={styles.line}>
          <Dot tone="agent" pulse />
          <Text tone="agent" weight="medium">Reviewing</Text>
          <Text tone="muted">
            {units.length} of {Math.max(began, units.length)} {Math.max(began, units.length) === 1 ? "decision" : "decisions"} left · the list shows the deck as it would be
          </Text>
        </div>
      </Banner>
    );

  // Units only hold pending items, so these are the open proposals that still need a decision.
  const waiting = new Set(units.map((u) => u.proposalId));
  const open = proposals.filter((p) => waiting.has(p.id));
  const newest = open.reduce((a, b) => (b.id > a.id ? b : a));
  const changes = units.reduce((n, u) => n + u.itemIds.length, 0);
  // Every agent behind the waiting proposals, newest first: one is named in
  // full ("rowan via codex"), several by their agent label.
  const agents = [...new Map([...open].sort((a, b) => b.id - a.id).map((p) => [p.author_display ?? p.author, p])).values()];
  const labels = [...new Set(agents.map((p) => p.author_label || p.author))];
  const what = waiting.size > 1
    ? `${changes} changes across ${waiting.size} proposals`
    : `${changes} ${changes === 1 ? "change" : "changes"}`;

  return (
    <Banner
      tone="agent"
      actions={
        <>
          <Button size="sm" variant="agent" kbd="↵" className={styles.tap} onClick={ui.startReview}>Review in the list</Button>
        </>
      }
    >
      <div className={styles.line}>
        <Dot tone="agent" pulse />
        <span className={styles.who}>
          {agents.length === 1 ? (
            <><Text tone="agent" weight="medium">{newest.author_display ?? newest.author}</Text> proposes {what}</>
          ) : labels.length === 2 ? (
            <>{what} from <Text tone="agent" weight="medium">{labels[0]}</Text> and <Text tone="agent" weight="medium">{labels[1]}</Text></>
          ) : (
            <>{what} from <Text tone="agent" weight="medium">{agents.length} agents</Text></>
          )}
        </span>
        <span className={styles.summary}>
          “<InlineText text={newest.summary} />”
        </span>
        <Text tone="faint" size="sm" className={styles.when}>{ago(newest.created_at)}</Text>
      </div>
    </Banner>
  );
}
