import { Explainer } from "../../ui/index.ts";
import { useVoice } from "../../app/Preferences.tsx";
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
import styles from "./ProposalBanner.module.css";

export function ProposalBanner({ units, review }: { units: ReviewUnit[]; review: Review }) {
  const voice = useVoice();
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
          <>
            <Text tone="faint" size="xs" className={styles.keys}>
              <Kbd>J</Kbd> <Kbd>K</Kbd> move · <Kbd>A</Kbd> accept · <Kbd>R</Kbd> reject
            </Text>
            <Button size="sm" variant="accept" kbd="⇧A" onClick={() => void review.acceptAll()}>
              Accept all left
            </Button>
            <Button size="sm" variant="ghost" kbd="Esc" onClick={ui.stopReview}>
              Done
            </Button>
          </>
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
  const newest = proposals.filter((p) => waiting.has(p.id)).reduce((a, b) => (b.id > a.id ? b : a));
  const changes = units.reduce((n, u) => n + u.itemIds.length, 0);

  return (
    <><div className={styles.explanation}><Explainer title="Proposed changes" summary={voice("Proposals wait for your decision before anything changes.", "Your agent has suggested changes for you to review.")}>Review the changes among the cards in the list. Accept or reject one item, a linked group, or all open proposals. A reason is optional and stays with the decision. Nothing is applied until you accept it.</Explainer></div><Banner
      tone="agent"
      actions={
        <Button size="sm" variant="agent" kbd="↵" className={styles.tap} onClick={ui.startReview}>
          Review in the list
        </Button>
      }
    >
      <div className={styles.line}>
        <Dot tone="agent" pulse />
        <span className={styles.who}>
          <Text tone="agent" weight="medium">{newest.author_display ?? newest.author}</Text>{" "}
          {waiting.size > 1
            ? `proposes ${changes} changes across ${waiting.size} proposals`
            : `proposes ${changes} ${changes === 1 ? "change" : "changes"}`}
        </span>
        <span className={styles.summary}>
          “<InlineText text={newest.summary} />”
        </span>
        <Text tone="faint" size="sm" className={styles.when}>{ago(newest.created_at)}</Text>
      </div>
    </Banner></>
  );
}
