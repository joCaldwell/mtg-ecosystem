// What separates two decks' lists (spec §5.3): shown for one of the owner's
// decks against another, and for a shared deck against one of the viewer's.

import type { DeckComparison } from "../../../api/client.ts";
import { Stack, Text } from "../../../ui/index.ts";
import { CardLine } from "./CardLine.tsx";
import styles from "./CompareDialog.module.css";

export function Differences({ diff }: { diff: DeckComparison }) {
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
