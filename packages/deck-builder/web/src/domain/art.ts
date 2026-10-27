// Which stored commander facts may be drawn as decorative art (D21), and the
// words that credit them. The server applies the same admission rule
// (src/art.ts); the client checks again because deck and public states pass
// card facts straight through.

/** A card's stored art facts. Either may be missing; then nothing is drawn. */
export interface ArtFacts {
  oracle_id: string;
  name: string;
  image_art_crop?: string | null;
  artist?: string | null;
}

export interface CreditedArt {
  oracle_id: string;
  /** The front face only: a double-faced commander is credited by the face shown. */
  name: string;
  image_art_crop: string;
  artist: string;
}

/** Only Scryfall's own art crops: https, its CDN host, and the art_crop path. */
export function isArtCrop(url: string): boolean {
  try {
    const u = new URL(url);
    return u.protocol === "https:" && u.host === "cards.scryfall.io" && u.pathname.startsWith("/art_crop/");
  } catch {
    return false;
  }
}

/** Up to two crops (a partner pair) that can be drawn with a named artist. */
export function creditedArt(cards: readonly ArtFacts[] | null | undefined): CreditedArt[] {
  const out: CreditedArt[] = [];
  for (const c of cards ?? []) {
    const artist = c.artist?.trim();
    if (out.length < 2 && artist && c.image_art_crop && isArtCrop(c.image_art_crop))
      out.push({ oracle_id: c.oracle_id, name: c.name.split(" // ")[0]!, image_art_crop: c.image_art_crop, artist });
  }
  return out;
}

/** "Art by …" alone where the card is named beside it; with the card where it is not. */
export function artCredit(art: CreditedArt, cardNamed: boolean): string {
  return cardNamed ? `Art by ${art.artist}` : `Art by ${art.artist} · ${art.name}`;
}
