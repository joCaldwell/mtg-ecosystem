import { type ReactNode } from "react";
import styles from "./DeckArt.module.css";

type Art = { oracle_id: string; name: string; image_art_crop?: string | null; artist?: string | null };
/** Decorative crops only; credits remain real text, including on image failure. */
export function DeckArt({ cards, identity = "", variant = "strip", collapsed = false, children }: {
  cards: readonly Art[]; identity?: string; variant?: "strip" | "banner" | "backdrop";
  collapsed?: boolean; children?: ReactNode;
}) {
  const art = cards.filter(c => c.artist?.trim() && c.image_art_crop?.startsWith("https://cards.scryfall.io/art_crop/")).slice(0, 2);
  return <div className={`${styles.surface} ${styles[variant]}`}
    data-identity={identity} data-art={art.length > 0 || undefined} data-collapsed={collapsed || undefined}>
    {art.length > 0 && <div className={styles.visual}>
      <div className={styles.images}>{art.map(c => <div className={styles.crop} key={c.oracle_id}>
        <img src={c.image_art_crop!} alt="" loading="lazy" decoding="async" width="626" height="457" onError={e => { e.currentTarget.hidden = true; }} />
      </div>)}</div>
    </div>}
    <div className={styles.identity} aria-hidden="true">{[...new Set(identity)].filter(c => "WUBRG".includes(c)).map(c => <i key={c} data-color={c} />)}</div>
    {children && <div className={styles.content}>{children}</div>}
    {art.length > 0 && <p className={styles.credit}>{art.map(c => <span key={c.oracle_id}>Art by {c.artist} · {c.name}</span>)}</p>}
  </div>;
}
