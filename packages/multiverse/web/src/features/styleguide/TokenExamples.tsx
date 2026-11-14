// TokenRow and TokenDetail with real tokens. Unlike the card examples these
// are typed in: the server does not serve the token catalog yet
// (feat/deck-tokens). Every value is copied from Scryfall's oracle-cards bulk
// file; the producers are real cards that make them. Replace with fetched
// data once the catalog lands.

import { useState } from "react";
import type { DeckToken } from "../../domain/tokens.ts";
import { CardRef, TokenDetail, TokenRow } from "../../mtg/index.ts";
import styles from "./TokenExamples.module.css";

const img = (path: string, v: string) => ({
  image_small: `https://cards.scryfall.io/small/front/${path}.jpg?${v}`,
  image_normal: `https://cards.scryfall.io/normal/front/${path}.jpg?${v}`,
  image_art_crop: `https://cards.scryfall.io/art_crop/front/${path}.jpg?${v}`,
});

const ELSPETH = { oracle_id: "05e6b243-48a6-4a42-bc5f-413441de9c33", name: "Elspeth, Sun's Champion", quantity: 1, role: "card" as const };

export const SAMPLE_TOKENS: DeckToken[] = [
  {
    oracle_id: "2f12324a-e14e-4776-bc51-9fd42aaf3222", name: "Elspeth, Sun's Champion Emblem", kind: "emblem",
    type_line: "Emblem — Elspeth", oracle_text: "Creatures you control get +2/+2 and have flying.",
    power: null, toughness: null, colors: "", artist: "Tyler Jacobson",
    ...img("d/f/dffbc23b-23d3-4cc5-9ea2-fa3aab785ffe", "1783915225"),
    made_by: [ELSPETH], producer_count: 1,
  },
  {
    oracle_id: "eac25f12-6459-438c-a09e-93e23d2cf80d", name: "Soldier", kind: "token",
    type_line: "Token Creature — Soldier", oracle_text: "",
    power: "1", toughness: "1", colors: "W", artist: "Magali Villeneuve",
    ...img("a/5/a58c2fc3-c07f-4f06-a9d5-1d0fe3dbde1f", "1789737284"),
    made_by: [ELSPETH, { oracle_id: "5b2364d7-a811-4595-a1b4-224c70555ffa", name: "Raise the Alarm", quantity: 1, role: "card" }],
    producer_count: 2,
  },
  {
    oracle_id: "3c549374-6c37-42e0-8d88-a8555d46732d", name: "Treasure", kind: "token",
    type_line: "Token Artifact — Treasure", oracle_text: "{T}, Sacrifice this token: Add one mana of any color.",
    power: null, toughness: null, colors: "", artist: "Alayna Danner",
    ...img("0/3/03992f88-7a15-4234-9bed-b7617d7ff09c", "1789735245"),
    made_by: [
      { oracle_id: "697bcfe1-ecbf-42a1-bfc7-0766d48ca56b", name: "Dockside Extortionist", quantity: 1, role: "card" },
      { oracle_id: "153376c9-dffd-458c-8ce3-a4c8269bc4e9", name: "Smothering Tithe", quantity: 1, role: "card" },
    ],
    producer_count: 2,
  },
];

export function TokenExamples() {
  const [open, setOpen] = useState(SAMPLE_TOKENS[1].oracle_id);
  const token = SAMPLE_TOKENS.find((t) => t.oracle_id === open)!;
  return (
    <div className={styles.examples}>
      <div className={styles.rows}>
        {SAMPLE_TOKENS.map((t) => (
          <TokenRow key={t.oracle_id} token={t} open={t.oracle_id === open} onOpen={() => setOpen(t.oracle_id)} />
        ))}
      </div>
      <div className={styles.detail}>
        <h4 className={styles.title}>{token.name}</h4>
        <TokenDetail
          token={token}
          producers={<ul className={styles.producers}>{token.made_by.map((p) => <li key={p.oracle_id}><CardRef name={p.name} /></li>)}</ul>}
        />
      </div>
    </div>
  );
}
