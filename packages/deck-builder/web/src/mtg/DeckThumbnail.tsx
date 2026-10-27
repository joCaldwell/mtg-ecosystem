import { artCredit, creditedArt, type ArtFacts } from "../domain/art.ts";
import { useNearView } from "../ui/index.ts";
import styles from "./DeckArt.module.css";

/** The rail's small commander art. It is decorative: the link it sits in is
 * named by the deck, and the credit is the native tooltip (larger art on the
 * deck's card and header credits it visibly). Downloads wait until the row
 * is near view in the rail. */
export function DeckThumbnail({ cards, className }: { cards: readonly ArtFacts[] | null | undefined; className?: string }) {
  const [ref, near] = useNearView<HTMLSpanElement>();
  const art = creditedArt(cards);
  if (!art.length) return null;
  return <span ref={ref} className={`${styles.surface} ${styles.thumbnail} ${className ?? ""}`} aria-hidden="true"
    title={art.map(c => artCredit(c, false)).join(" / ")}>
    <span className={styles.visual}>{art.map(c => <span className={styles.crop} key={c.oracle_id}>
      {near && <img src={c.image_art_crop} alt="" loading="lazy" fetchPriority="low" decoding="async" width="626" height="457" onError={e => { e.currentTarget.parentElement!.dataset.failed = ""; }} />}
    </span>)}</span>
  </span>;
}
