// Shared card facts only: no tenant data and no outbound lookup (D21).
import { randomInt } from "node:crypto";
import type { Database } from "./sqlite.ts";

export interface CardArt {
  oracle_id: string;
  name: string;
  image_art_crop: string;
  artist: string;
}

/** Unknown illustrators and non-CDN/non-crop URLs never become decorative art. */
export function creditedArt(card: { oracle_id: string; name: string; image_art_crop?: string | null; artist?: string | null }): CardArt | null {
  if (!card.artist?.trim() || !card.image_art_crop?.startsWith("https://cards.scryfall.io/art_crop/")) return null;
  return { oracle_id: card.oracle_id, name: card.name, image_art_crop: card.image_art_crop, artist: card.artist.trim() };
}

// These exact names were verified in the cards table; basic-land environments
// keep first-run art quiet. Resolve their current printing and artist together.
const BACKDROPS = ["Forest", "Island", "Mountain"];
export function visitBackdrop(db: Database): CardArt | null {
  const cards = db.prepare(`SELECT oracle_id, name, image_art_crop, artist FROM cards
    WHERE name IN (${BACKDROPS.map(() => "?").join(",")}) AND retired = 0`).all(...BACKDROPS) as unknown as CardArt[];
  const known = cards.map(creditedArt).filter((c): c is CardArt => c !== null);
  return known.length ? known[randomInt(known.length)]! : null;
}
