import styles from "./DeckArt.module.css";
/** The rail is too tight for a visible caption; larger deck art credits it. */
export function DeckThumbnail({cards}:{cards:readonly {oracle_id:string;name:string;image_art_crop:string;artist:string}[]}) {
 const art=cards.filter(c=>c.artist.trim() && c.image_art_crop.startsWith("https://cards.scryfall.io/art_crop/")).slice(0,2);
 if(!art.length)return null;
 const credit=art.map(c=>`Art by ${c.artist} · ${c.name}`).join(" / ");
 return <span className={`${styles.surface} ${styles.thumbnail}`} role="img" aria-label={credit} title={credit}>
  <span className={styles.images}>{art.map(c=><span className={styles.crop} key={c.oracle_id}><img src={c.image_art_crop} alt="" loading="lazy" decoding="async" width="626" height="457" onError={e=>{e.currentTarget.hidden=true;}} /></span>)}</span>
 </span>;
}
