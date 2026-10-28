import { api, type Proposal } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { ago } from "../../domain/time.ts";
import { reviewUnits } from "../../domain/review.ts";
import { InlineText } from "../../mtg/index.ts";
import { ErrorNote, Eyebrow, Kbd, Loading, Section, Stack, Text } from "../../ui/index.ts";
import { ChangeUnit } from "./ChangeUnit.tsx";
import { DeckProjection } from "./DeckProjection.tsx";
import { ProposalDecision } from "./ProposalDecision.tsx";
import styles from "./ProposalSummary.module.css";

/**
 * The selected proposal, with a whole-proposal decision and a link to review
 * individual changes in the deck.
 */
export function ProposalSummary({ proposal }: { proposal: Proposal }) {
  const deck = useQuery(() => api.getDeck(proposal.deck_id), [proposal.deck_id]);
  const units = reviewUnits([proposal]);
  const state = deck.data?.state ?? null;

  return (
    <>
      <Stack gap={2}>
        <Eyebrow className={styles.eyebrow}>
          <span className={styles.agent}>Proposal {proposal.id}</span>
          <span className={styles.agent}>{proposal.author_display ?? proposal.author}</span>
          <span>{ago(proposal.created_at)}</span>
          <span>{proposal.deck_name}</span>
        </Eyebrow>
        <div className={styles.title}>
          {proposal.summary ? <InlineText text={proposal.summary} /> : "No summary given"}
        </div>
      </Stack>

      {proposal.stale && <Text tone="warn">The deck changed since r{proposal.base_revision ?? "unknown"}. Review current r{proposal.current_revision} before approval.</Text>}
      {deck.error && <ErrorNote>{deck.error}</ErrorNote>}
      {!state && !deck.error && <Loading />}

      <Section title="Changes in this proposal">
        <Stack gap={3}>
          {units.map((unit) => (
            <ChangeUnit key={unit.key} unit={unit} state={state} />
          ))}
        </Stack>
      </Section>
      {state && state.proposals.length > 0 && (state.proposals.length > 1
        ? <details className={styles.projection}>
            <summary>Deck impact if all {state.proposals.length} pending proposals are accepted</summary>
            <DeckProjection state={state} heading={false} />
          </details>
        : <DeckProjection state={state} />)}
      {state && <ProposalDecision proposal={proposal} state={state} onDecided={deck.reload} />}
      <div className={styles.action}>
        <a className={styles.review} href={href({ page: "deck", deckId: proposal.deck_id, review: true, proposal: proposal.id })}>
          Review in the list <Kbd>↵</Kbd>
        </a>
      </div>
    </>
  );
}
