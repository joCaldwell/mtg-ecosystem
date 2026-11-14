// The cards that make a token, drawn as the list draws them, so each one
// hovers, inspects and opens its own details exactly as a row in the list.

import type { DeckToken } from "../../../domain/tokens.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { CardRow } from "../list/CardRow.tsx";

export function TokenProducers({ token }: { token: DeckToken }) {
  const { cards } = useDeckList().state;
  return (
    <div>
      {token.made_by.map((p) => {
        const card = cards.find((c) => c.oracle_id === p.oracle_id)
          ?? { oracle_id: p.oracle_id, name: p.name, mana_cost: null, type_line: "", oracle_text: "", power: null, toughness: null, loyalty: null };
        return <CardRow key={p.oracle_id} card={card} quantity={p.quantity} bold={p.role === "commander"} />;
      })}
    </div>
  );
}
