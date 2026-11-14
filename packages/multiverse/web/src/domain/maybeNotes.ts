// Exact provenance strings written by deck/interop.ts:land. Unknown or edited
// notes remain card-specific; this presentation never changes notes or order.
const IMPORT_NOTES = new Map([
  ["From the pasted list's Sideboard", "Sideboard"],
  ["From the pasted list's Maybeboard", "Maybeboard"],
]);

export function maybeNotes(cards: readonly { maybe_note: string | null }[]) {
  const origins = [...IMPORT_NOTES].flatMap(([note, label]) => {
    const count = cards.filter(card => card.maybe_note === note).length;
    return count ? [{ note, label, count }] : [];
  });
  const first = cards[0]?.maybe_note;
  const shared = cards.length > 1 && first && !IMPORT_NOTES.has(first) && cards.every(card => card.maybe_note === first) ? first : null;
  return { origins, shared };
}
