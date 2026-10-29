import { type ReactNode } from "react";
import { artCredit, creditedArt, type ArtFacts } from "../domain/art.ts";
import styles from "./DeckArt.module.css";

/** Decorative crops in a band above the content, each credited inside its own
 * crop. Images load as soon as they render: `load` false keeps the frame but
 * defers the download and its credit (the roster passes whether the card is
 * near view, which native lazy loading judges far too generously), and
 * `priority="high"` puts the public page's art first. The frame's size comes from the band, never from
 * the image, so nothing moves when it arrives. The art follows the content in
 * the DOM, so a screen reader meets the deck before its credit. */
export function DeckArt({ cards, identity = "", variant = "strip", load = true, priority, children }: {
  cards: readonly ArtFacts[] | null | undefined; identity?: string;
  variant?: "strip" | "ribbon" | "banner" | "showcase" | "backdrop";
  load?: boolean; priority?: "auto" | "high"; children?: ReactNode;
}) {
  const art = creditedArt(cards);
  return <div className={`${styles.surface} ${styles[variant]}`}
    data-art={art.length > 0 || undefined}>
    {children}
    {art.length > 0 && <div className={styles.visual}>{art.map(c => <div className={styles.crop} key={c.oracle_id}>
      {load && <img src={c.image_art_crop} alt="" loading="eager" fetchPriority={priority} decoding="async" onError={e => { e.currentTarget.parentElement!.dataset.failed = ""; }} />}
      <span className={styles.credit}>{artCredit(c, variant !== "backdrop")}</span>
    </div>)}</div>}
    <div className={styles.identity} aria-hidden="true">{[...identity].filter(c => "WUBRG".includes(c)).map(c => <i key={c} data-color={c} />)}</div>
  </div>;
}
