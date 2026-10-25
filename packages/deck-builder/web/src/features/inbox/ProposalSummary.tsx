import { api, type Proposal } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { ago } from "../../domain/time.ts";
import { reviewUnits } from "../../domain/review.ts";
import { InlineText } from "../../mtg/index.ts";
import { ErrorNote, Eyebrow, Kbd, Loading, Section, Stack, Text } from "../../ui/index.ts";
import { ChangeUnit } from "./ChangeUnit.tsx";
import { DeckProjection } from "./DeckProjection.tsx";
import styles from "./ProposalSummary.module.css";

/**
 * A proposal, read-only. Rulings happen in the deck, where each change sits
 * among the cards around it; this page only hands the owner over to it.
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
      <div className={styles.action}>
        <a className={styles.review} href={href({ page: "deck", deckId: proposal.deck_id, review: true })}>
          Review in the list <Kbd>↵</Kbd>
        </a>
        <Text tone="faint" size="sm">
          Rulings happen in the deck, where each change sits among the cards around it.
        </Text>
      </div>

      {deck.error && <ErrorNote>{deck.error}</ErrorNote>}
      {!state && !deck.error && <Loading />}
      {state && <DeckProjection state={state} />}

      <Section title="What it changes">
        <Stack gap={3}>
          {units.map((unit) => (
            <ChangeUnit key={unit.key} unit={unit} state={state} />
          ))}
        </Stack>
      </Section>
    </>
  );
}
