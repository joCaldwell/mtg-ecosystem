// What the deck knows about a card (spec §7.2, §8.3): memories linked to it
// and the owner's earlier decisions on it. The same context an agent is shown
// beside the card.

import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { ago } from "../../../domain/time.ts";
import { InlineText } from "../../../mtg/index.ts";
import { ErrorNote, Eyebrow, Loading, Text } from "../../../ui/index.ts";
import { MemoryCard } from "../../memories/index.ts";
import { CardDecisionRow } from "../../decisions/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./CardHistory.module.css";

export function CardHistory({ oracleId }: { oracleId: string }) {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const history = useQuery(() => api.cardHistory(deckId, oracleId), [deckId, oracleId, deck.revision]);
  const memories = history.data?.memories ?? [];
  const decisions = history.data?.decisions ?? [];
  const ownership = history.data?.ownership ?? [];

  return (
    <>
      {history.error && <ErrorNote>{history.error}</ErrorNote>}
      {history.loading && <Loading />}
      {ownership.length > 0 && <section className={styles.section} aria-label="Ownership evidence">
        <Eyebrow>Ownership evidence</Eyebrow>
        {ownership.map((e, index) => <div key={e.id} className={styles.evidence}>
          <Text size="sm">{index > 0 ? "Previously: " : ""}{e.status}{e.quantity != null ? ` · ${e.quantity} copies` : ""}{e.available_quantity != null ? ` · ${e.available_quantity} available` : ""}</Text>
          <Text size="sm" tone="soft"><InlineText text={e.basis} /></Text>
          <Text size="xs" tone={e.author_via === "agent" ? "agent" : "faint"}>{e.author_display ?? e.author} · {ago(e.created_at)}{e.source_deck_name ? ` · ${e.source_deck_name}` : ""}</Text>
        </div>)}
      </section>}
      {/* An empty history is one quiet line each, not a heading over "None". */}
      {memories.length > 0 ? <section className={styles.section}>
        <Eyebrow>Memories linked to this card · {memories.length}</Eyebrow>
        {memories.map((m) => (
          <MemoryCard key={m.id} memory={m} onChanged={history.reload} />
        ))}
      </section> : history.data && decisions.length > 0 && <Text tone="muted" size="sm" as="p">No linked memories</Text>}
      {decisions.length > 0 ? <section className={styles.section}>
        <Eyebrow>Your decisions on it in this deck · {decisions.length}</Eyebrow>
        {decisions.map((r) => (
          <CardDecisionRow key={r.log_id} decision={r} />
        ))}
      </section> : history.data && (memories.length > 0
        ? <Text tone="muted" size="sm" as="p">No decisions in this deck</Text>
        : <Text tone="muted" size="sm" as="p">No linked memories or prior decisions</Text>)}
    </>
  );
}
