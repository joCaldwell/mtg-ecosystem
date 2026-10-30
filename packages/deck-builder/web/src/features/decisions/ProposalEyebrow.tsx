import type { Proposal } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import styles from "./ProposalEyebrow.module.css";

/**
 * A card proposal's one label, wherever it is shown (inbox, deck Review,
 * card details): "Proposal #5 · rowan via claude-code · yesterday". Sentence
 * case, so handles and agent labels read as they are written. Violet marks
 * an agent's proposal (DESIGN.md). Review alone adds its queue counter at
 * the right; the inbox, which spans decks, appends the deck's name.
 */
export function ProposalEyebrow({
  proposal,
  counter,
  deck,
  className,
}: {
  proposal: Pick<Proposal, "id" | "author" | "author_display" | "author_via" | "created_at">;
  counter?: string;
  deck?: string;
  className?: string;
}) {
  const agent = proposal.author_via !== "ui" ? styles.agent : undefined;
  return (
    <div className={className ? `${styles.eyebrow} ${className}` : styles.eyebrow}>
      <span className={styles.parts}>
        <span className={agent}>Proposal #{proposal.id}</span>
        <span className={agent}>{proposal.author_display ?? proposal.author}</span>
        <span>{ago(proposal.created_at)}</span>
        {deck ? <span>{deck}</span> : null}
      </span>
      {counter ? <span className={styles.counter}>{counter}</span> : null}
    </div>
  );
}
