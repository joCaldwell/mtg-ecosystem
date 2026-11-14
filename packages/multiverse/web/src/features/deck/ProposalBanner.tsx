// The strip under the toolbar that says whether anything is waiting on the
// owner (spec §8.2): an agent's proposal, or the review in progress. Archived
// decks keep their read-only notice inside the art header instead.

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

export function ProposalBanner({ units, review, confirmingAll, setConfirmingAll, tablet = false }: {
  units: ReviewUnit[];
  review: Review;
  /** "Accept all left" asks first; ⇧A in DeckPage opens the same question. */
  confirmingAll: boolean;
  setConfirmingAll: (on: boolean) => void;
  tablet?: boolean;
}) {
  const { proposals } = useDeckState();
  const ui = useDeckUi();

  // How many decisions the review began with, so "X of Y left" has a Y that
  // does not shrink as the owner rules.
  const [began, setBegan] = useState(units.length);
  useEffect(() => {
    if (!ui.reviewing) setBegan(units.length);
  }, [ui.reviewing, units.length]);

  if (units.length === 0 || (tablet && !ui.reviewing)) return null;

  if (tablet) return <Banner className={styles.tabletReview} data-tablet-review tone="agent" actions={<>
    <Button size="touch" aria-haspopup="dialog" onClick={() => ui.setTab("review")}>Current proposal</Button>
    <Button size="touch" variant="ghost" onClick={ui.stopReview}>Done</Button>
  </>}>
    <div className={styles.line}>
      <Text tone="agent" weight="medium">Reviewing</Text>
      <Text tone="soft">{units.length} of {Math.max(began, units.length)} {Math.max(began, units.length) === 1 ? "decision" : "decisions"} left</Text>
    </div>
  </Banner>;

  if (ui.reviewing)
    return (
      <Banner
        className={`${styles.banner} ${styles.desktopReview}`}
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
  // Every agent label behind the waiting proposals, newest first.
  const labels = [...new Set([...open].sort((a, b) => b.id - a.id).map((p) => p.author_label || p.author))];
  const what = `${changes} ${changes === 1 ? "change" : "changes"} in ${waiting.size} ${waiting.size === 1 ? "proposal" : "proposals"}`;

  return (
    <Banner
      className={styles.banner}
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
          {what} · from{" "}
          {labels.length > 2 ? (
            <Text tone="agent" weight="medium">{labels.length} agents</Text>
          ) : labels.map((l, i) => (
            <span key={l}>{i > 0 && " and "}<Text tone="agent" weight="medium">{l}</Text></span>
          ))}
        </span>
        <span className={styles.tail}>
          <span className={styles.summary}>
            {waiting.size > 1 && <Text tone="muted">latest: </Text>}“<InlineText text={newest.summary} />”
          </span>
          <Text tone="faint" size="sm" className={styles.when}>· {ago(newest.created_at)}</Text>
        </span>
      </div>
    </Banner>
  );
}
