import { useState } from "react";
import { api, type DeckComparison } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { Button, Dialog, ErrorNote, Field, Loading, Row, Select, Spacer, Stack, Text } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { CardLine } from "./CardLine.tsx";
import { useAttempt } from "./useAttempt.ts";
import styles from "./CompareDialog.module.css";

// Compare this deck with any other, and record which deck it was forked
// from — a deck copied by hand has no such link until it is set here.
export function CompareDialog({ onClose }: { onClose: () => void }) {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const parent = deck.parent;
  const [other, setOther] = useState<number | null>(parent?.id ?? null);
  const { error, mutate } = useAttempt();

  const decks = useQuery(() => api.listDecks(), []);
  const others = (decks.data ?? []).filter((d) => d.id !== deckId);

  // Refetched when this deck's revision moves, so the diff never describes a list that has changed.
  const comparison = useQuery(
    () => (other == null ? Promise.resolve(null) : api.compareDecks(deckId, other)),
    [deckId, other, deck.revision],
  );
  // The previous result stays while a refetch runs; it must not be shown for a different deck.
  const diff = comparison.data && comparison.data.b.id === other ? comparison.data : null;

  return (
    <Dialog title="Compare decks" width="lg" onClose={onClose} footer={<Button onClick={onClose}>Close</Button>}>
      <Stack gap={6}>
        <Row align="end" wrap>
          <Field label="Compare with">
            <Select
              value={other ?? ""}
              onChange={(e) => setOther(e.target.value === "" ? null : Number(e.target.value))}
              data-autofocus=""
            >
              <option value="">Choose a deck…</option>
              {others.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                  {d.id === parent?.id ? " (parent)" : ""}
                </option>
              ))}
            </Select>
          </Field>
          <Spacer />
          {other != null && other !== parent?.id ? (
            <Button
              size="sm"
              title="Record that this deck was forked from the one selected"
              onClick={() => mutate(() => api.setDeckParent(deckId, other))}
            >
              Mark this deck as a fork of it
            </Button>
          ) : null}
          {parent ? (
            <Button size="sm" onClick={() => mutate(() => api.setDeckParent(deckId, null))}>
              Unlink from {parent.name}
            </Button>
          ) : null}
        </Row>

        {error ? <ErrorNote>{error}</ErrorNote> : null}
        {comparison.error ? <ErrorNote>{comparison.error}</ErrorNote> : null}
        {decks.data && others.length === 0 ? (
          <Text tone="muted" as="p">
            There are no other decks to compare with. Make or fork another deck first.
          </Text>
        ) : null}
        {other != null && !diff && !comparison.error ? <Loading /> : null}
        {diff ? <Differences diff={diff} /> : null}
      </Stack>
    </Dialog>
  );
}

function Differences({ diff }: { diff: DeckComparison }) {
  const { a, b } = diff;
  const none = diff.only_in_a.length + diff.only_in_b.length + diff.quantity_differs.length + diff.role_differs.length === 0;
  return (
    <Stack gap={6}>
      <Text tone="muted" as="p">
        <Text mono tone="soft">
          {diff.shared_count}
        </Text>{" "}
        cards are in both. Maybe lists are not compared.
        {none ? " The two lists are the same." : ""}
      </Text>

      <div className={styles.columns}>
        <Side title={`Only in ${a.name}`} cards={diff.only_in_a} />
        <Side title={`Only in ${b.name}`} cards={diff.only_in_b} />
      </div>

      {diff.quantity_differs.length > 0 ? (
        <Stack gap={2}>
          <Text weight="medium" size="sm" as="h3">
            Different quantities · {diff.quantity_differs.length}
          </Text>
          <ul className={styles.list}>
            {diff.quantity_differs.map((q) => (
              <li key={q.name} className={styles.diffLine}>
                <span className={styles.diffName}>{q.name}</span>
                <Text tone="muted" size="sm" mono>
                  {q.in_a} in {a.name} · {q.in_b} in {b.name}
                </Text>
              </li>
            ))}
          </ul>
        </Stack>
      ) : null}

      {diff.role_differs.length > 0 ? (
        <Stack gap={2}>
          <Text weight="medium" size="sm" as="h3">
            Different roles · {diff.role_differs.length}
          </Text>
          <ul className={styles.list}>
            {diff.role_differs.map((r) => (
              <li key={r.name} className={styles.diffLine}>
                <span className={styles.diffName}>{r.name}</span>
                <Text tone="muted" size="sm">
                  {r.in_a} in {a.name} · {r.in_b} in {b.name}
                </Text>
              </li>
            ))}
          </ul>
        </Stack>
      ) : null}
    </Stack>
  );
}

function Side({ title, cards }: { title: string; cards: DeckComparison["only_in_a"] }) {
  return (
    <Stack gap={2} className={styles.side}>
      <Text weight="medium" size="sm" as="h3" className={styles.sideTitle}>
        {title} · <Text mono>{cards.length}</Text>
      </Text>
      {cards.length ? (
        <ul className={styles.list}>
          {cards.map((c) => (
            <CardLine
              key={c.oracle_id}
              oracleId={c.oracle_id}
              name={c.name}
              quantity={c.quantity}
              cost={c.mana_cost}
              detail={c.type_line}
            />
          ))}
        </ul>
      ) : (
        <Text tone="muted" size="sm" as="p">
          None.
        </Text>
      )}
    </Stack>
  );
}
