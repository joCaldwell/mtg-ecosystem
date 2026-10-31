import type { ReactNode } from "react";
import { tokenSummary, type DeckToken } from "../domain/tokens.ts";
import { CardImage } from "./CardImage.tsx";
import { cardImageUrl } from "./cardImage.ts";
import { OracleText } from "./OracleText.tsx";
import { tokenCardInfo } from "./tokenCard.ts";
import styles from "./TokenDetail.module.css";

/**
 * A token's or emblem's details: its line, rules text, picture with a subtle
 * artist credit, and, beside the picture, the cards that make it (`producers`,
 * drawn by the caller, which knows how a card opens there). The caller owns the
 * title.
 */
export function TokenDetail({ token, producers }: { token: DeckToken; producers: ReactNode }) {
  const info = tokenCardInfo(token);
  // Only the token's own stored image: a lookup by name could find a card.
  const pictured = !!cardImageUrl(info);
  return (
    <div className={styles.detail}>
      <div className={styles.card}>
        {pictured && <figure className={styles.figure}>
          <CardImage name={token.name} card={info} />
          {token.artist && <figcaption className={styles.credit}>Art by {token.artist}</figcaption>}
        </figure>}
        <p className={styles.summary}>{tokenSummary(token)}</p>
        {token.oracle_text && <OracleText text={token.oracle_text} bare />}
      </div>
      <div className={styles.producers}>
        <h3 className={styles.heading}>Made by</h3>
        {producers}
      </div>
    </div>
  );
}
