// Kept apart from art.ts so the public page, which never folds, does not load it.
/** The desktop banner's fold, from the deck list's scroll position. It folds
 * past `threshold`, unfolds at or above it, and only folds a list with room
 * to stay folded: a list that would clamp back above the threshold once the
 * band's space is returned to it keeps its banner instead of flickering. */
export function nextFold(
  folded: boolean,
  list: { scrollTop: number; scrollHeight: number; clientHeight: number },
  space: { threshold: number; band: number },
): boolean {
  if (list.scrollTop <= space.threshold) return false;
  if (list.scrollHeight - list.clientHeight > space.threshold + space.band) return true;
  return folded;
}
