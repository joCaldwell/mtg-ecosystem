// One heading and its rows. In review mode the group also draws the cards
// the pending proposals would add, after its own rows.

import type { ReactNode } from "react";
import type { DeckCard, ProposalItem } from "../../../api/client.ts";
import type { GroupStatus } from "../../../domain/grouping.ts";
import type { CardMark } from "../../../domain/marks.ts";
import type { GroupCost } from "../../../domain/price.ts";
import { CardRow } from "./CardRow.tsx";
import { GroupHeader } from "./GroupHeader.tsx";
import styles from "./GroupBlock.module.css";

// A group this long would leave the other columns empty if it stayed whole,
// as in a deck with no slots where every card is unfiled.
const LONG_GROUP = 30;

export interface GroupBlockProps {
  title: string;
  count?: number;
  target?: string | null;
  status?: GroupStatus;
  delta?: number;
  projected?: number;
  cost?: GroupCost;
  cards: DeckCard[];
  matchingCards?: ReadonlySet<string>;
  matchingAdds?: ReadonlySet<number>;
  /** Pending adds, drawn only while reviewing. */
  adds?: ProposalItem[];
  /** oracle_ids with a pending cut, drawn only while reviewing. */
  cuts?: Set<string>;
  /** oracle_ids belonging to the decision under the review cursor. */
  current: Set<string>;
  marks: Map<string, CardMark>;
  bold?: boolean;
  /** Shown when the group has nothing in it. */
  empty?: ReactNode;
}

export function GroupBlock({ cards, matchingCards, matchingAdds, adds = [], cuts, current, marks, bold, empty, ...head }: GroupBlockProps) {
  return (
    <section className={`${styles.group} ${cards.length + adds.length > LONG_GROUP ? styles.long : ""}`}>
      <GroupHeader {...head} />
      {cards.map((c) => (
        <CardRow
          key={c.oracle_id}
          card={c}
          filterMatch={matchingCards?.has(c.oracle_id)}
          quantity={c.quantity}
          change={cuts?.has(c.oracle_id) ? "cut" : undefined}
          mark={marks.get(c.oracle_id)}
          bold={bold}
          current={current.has(c.oracle_id)}
        />
      ))}
      {adds.map((item) => (
        <CardRow
          key={`add-${item.id}`}
          card={{ ...item, name: item.card_name }}
          filterMatch={matchingAdds?.has(item.id)}
          change="add"
          mark={marks.get(item.oracle_id)}
          current={current.has(item.oracle_id)}
        />
      ))}
      {cards.length === 0 && adds.length === 0 && empty}
    </section>
  );
}
