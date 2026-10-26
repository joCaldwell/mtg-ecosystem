// Decision on a phone (spec §8.2, §11.2): each decision — an add, a cut, a swap
// or a group — with Accept and Reject, and Accept all and Reject all above them. Each takes
// the optional reason, which the agent reads at its next session. The changes
// themselves are drawn into the list below, as on desktop, and the decision
// last touched is the one outlined there. The decisions go through the same
// useReview the desktop uses, so staleness, undo and the inbox behave alike.

import { useState } from "react";
import type { ReviewUnit } from "../../domain/review.ts";
import { ago } from "../../domain/time.ts";
import { InlineText } from "../../mtg/index.ts";
import { Button, Eyebrow, Text, TextField } from "../../ui/index.ts";
import { ReviewItemLine, useDeckState, useReview, type Review } from "../deck/index.ts";
import styles from "./PhoneDecisions.module.css";

const KIND: Record<ReviewUnit["kind"], string> = {
  swap: "Swap · ruled together",
  add: "Add",
  cut: "Cut",
  group: "Group · ruled together",
};

export function PhoneDecisions({ view }: { view: { units: ReviewUnit[] } }) {
  const state = useDeckState();
  const review = useReview(view.units, { sheet: true });
  const [reason, setReason] = useState("");
  if (!view.units.length) return null;
  if (state.deck.category === "archived")
    return <p className={styles.legacy}>This Archived deck has {view.units.length} pending decisions, kept until it is restored on a larger screen.</p>;
  const open = state.proposals.filter((p) => view.units.some((u) => u.proposalId === p.id)).length;
  return (
    <section className={styles.decisions} aria-label="Waiting on you">
      <div className={styles.all}>
        <Eyebrow>Waiting on you · {view.units.length} decision{view.units.length === 1 ? "" : "s"}</Eyebrow>
        <Text size="sm" tone="muted" as="p">Each change is drawn into the list below. Rule on each one, or on everything{open > 1 ? ` in all ${open} proposals` : ""} at once.</Text>
        <TextField fieldSize="lg" className={styles.reason} value={reason} onChange={(e) => setReason(e.target.value)} maxLength={500}
          placeholder="Reason for everything, optional" aria-label="Reason for accepting or rejecting everything, optional" />
        <div className={styles.buttons}>
          <Button size="touch" variant="accept" onClick={() => void review.acceptAll(reason, () => setReason(""))}>
            Accept all
          </Button>
          <Button size="touch" variant="reject" onClick={() => void review.rejectAll(reason, () => setReason(""))}>
            Reject all
          </Button>
        </div>
      </div>
      <ol className={styles.units}>
        {view.units.map((unit, index) => <Unit key={unit.key} unit={unit} index={index} review={review} />)}
      </ol>
      {review.confirmation}
    </section>
  );
}

function Unit({ unit, index, review }: { unit: ReviewUnit; index: number; review: Review }) {
  const state = useDeckState();
  const [reason, setReason] = useState("");
  const proposal = state.proposals.find((p) => p.id === unit.proposalId);
  return (
    <li className={styles.unit} aria-label={`Decision ${index + 1}: ${KIND[unit.kind]}`}>
      <div className={styles.unitHead}>
        <span className={styles.kind}>{KIND[unit.kind]}</span>
        {proposal && <Text size="xs" tone="agent">{proposal.author_display ?? proposal.author} · {ago(proposal.created_at)}</Text>}
      </div>
      {proposal?.summary && <Text size="sm" tone="soft" as="p"><InlineText text={proposal.summary} /></Text>}
      {proposal?.stale && <Text size="sm" tone="warn" as="p">The deck changed since this was proposed (r{proposal.base_revision ?? "?"}, now r{proposal.current_revision}). Accepting asks you to confirm.</Text>}
      <div className={styles.items}>
        {unit.items.map((item) => <ReviewItemLine key={item.id} item={item} />)}
      </div>
      <TextField fieldSize="lg" className={styles.reason} value={reason} onChange={(e) => setReason(e.target.value)} maxLength={500}
        placeholder="Why? Optional. The agent reads this next session." aria-label={`Reason for decision ${index + 1}, optional`} />
      <div className={styles.buttons}>
        <Button size="touch" variant="accept" onClick={() => void review.accept(unit, reason)}>Accept</Button>
        <Button size="touch" variant="reject" onClick={() => void review.reject(unit, reason)}>Reject</Button>
      </div>
    </li>
  );
}
