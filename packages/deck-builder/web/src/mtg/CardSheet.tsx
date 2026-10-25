// The hover card's touch form (DESIGN.md, narrow screens): the same facts, in
// a dialog the reader opened by tapping a card, so it never depends on hover.
// Bottom sheet on a phone, a small centred dialog otherwise.

import { useState } from "react";
import { ptString } from "../domain/cards.ts";
import { Dialog } from "../ui/index.ts";
import type { CardInfo } from "./CardInfo.ts";
import { cardImageUrl } from "./cardImage.ts";
import { ManaCost } from "./ManaCost.tsx";
import { OracleText } from "./OracleText.tsx";
import { Price } from "./Price.tsx";
import styles from "./CardSheet.module.css";

export function CardSheet({ card, onClose, sheet = false }: { card: CardInfo; onClose: () => void; sheet?: boolean }) {
  const [image, setImage] = useState<"loading" | "loaded" | "failed">("loading");
  const pt = ptString(card);
  const url = cardImageUrl(card);
  return (
    <Dialog title={card.name} onClose={onClose} placement={sheet ? "sheet" : "center"}>
      <div className={styles.card}>
        {url && image !== "failed" && (
          <img className={image === "loaded" ? styles.image : styles.pending} src={url} alt={`${card.name}, card image`}
            loading="lazy" onLoad={() => setImage("loaded")} onError={() => setImage("failed")} />
        )}
        <div className={styles.facts}>
          <div className={styles.head}>
            <ManaCost cost={card.mana_cost} />
          </div>
          <div className={styles.type}>
            {card.type_line}
            {pt && <span className={styles.pt}>{pt}</span>}
            <span className={styles.price}><Price cents={card.usd_cents} /></span>
          </div>
          <OracleText text={card.oracle_text} bare />
        </div>
      </div>
    </Dialog>
  );
}
