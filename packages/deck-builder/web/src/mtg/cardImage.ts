// The endpoint is Scryfall's "named" card lookup with `format=image`
// (https://scryfall.com/docs/api/cards/named); checked by request on
// 2026-10-10, it answers 302 to the image on cards.scryfall.io. This is the
// ONE place image URLs are built, so storing image links at ingest later
// changes only this file.
export function cardImageUrl(name: string): string {
  return `https://api.scryfall.com/cards/named?format=image&version=normal&exact=${encodeURIComponent(name)}`;
}
