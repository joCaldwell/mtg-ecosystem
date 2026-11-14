/** Image URLs are facts from bulk ingest, never API lookups by name. */
export function cardImageUrl(card: { image_normal?: string | null; image_small?: string | null }): string | undefined {
  return card.image_normal ?? card.image_small ?? undefined;
}
