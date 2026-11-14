import { useLayoutEffect, useRef, useState } from "react";
import type { CardInfo } from "./CardInfo.ts";
import { ptString } from "../domain/cards.ts";
import { placeNear } from "../ui/floating.ts";
import { cardImageUrl } from "./cardImage.ts";
import { ManaCost } from "./ManaCost.tsx";
import { OracleText } from "./OracleText.tsx";
import { Price } from "./Price.tsx";
import styles from "./CardHover.module.css";
interface Shown { card: CardInfo; anchor: DOMRect; }
type ImageStatus = "loading" | "loaded" | "failed";

export function HoverCard({ shown }: { shown: Shown }) {
  const { card, anchor } = shown;
  const box = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const [image, setImage] = useState<ImageStatus>("loading");

  // Placed after render because the box's size is only known once the text is
  // in, and it grows wider when the picture arrives, so that re-places it.
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    setPos(placeNear(anchor, { width: el.offsetWidth, height: el.offsetHeight }, { prefer: "right" }));
  }, [anchor, image, card]);

  const pt = ptString(card);
  return (
    <div
      ref={box}
      className={styles.card}
      role="tooltip"
      style={{ left: pos?.left ?? 0, top: pos?.top ?? 0, visibility: pos ? "visible" : "hidden" }}
    >
      {image !== "failed" && cardImageUrl(card) && (
        <img
          className={image === "loaded" ? styles.image : styles.pending}
          src={cardImageUrl(card)}
          loading="lazy"
          alt=""
          onLoad={() => setImage("loaded")}
          onError={() => setImage("failed")}
        />
      )}
      <div className={styles.body}>
        <div className={styles.head}>
          <span className={styles.name}>{card.name}</span>
          <ManaCost cost={card.mana_cost} />
        </div>
        <div className={styles.type}>
          {card.type_line}
          {pt && <span className={styles.pt}>{pt}</span>}
          <span className={styles.price}>
            <Price currency={card.currency} cents={card.cents} />
          </span>
        </div>
        {card.cents != null && card.store && <div className={styles.type}>
          {card.store === "tcgplayer" ? "TCGplayer" : "Cardmarket"} · Scryfall estimate · {card.as_of?.slice(0, 10) ?? "date unknown"}
        </div>}
        <div className={styles.oracle}>
          <OracleText text={card.oracle_text} bare />
        </div>
      </div>
    </div>
  );
}

/** Renders the app's one floating hover card, and owns the timing for every trigger. */
