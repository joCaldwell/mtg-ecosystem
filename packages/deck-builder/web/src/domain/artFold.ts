// Kept apart from art.ts so the public page, which never folds, does not load it.
/** The desktop banner's fold, from the deck list's scroll position. It folds
 * past `threshold`, unfolds near the top at `expandThreshold`, and only folds a list with room
 * to stay folded: a list that would clamp back above the threshold once the
 * band's space is returned to it keeps its banner instead of flickering. */
export function nextFold(
  folded: boolean,
  list: { scrollTop: number; scrollHeight: number; clientHeight: number },
  space: { threshold: number; expandThreshold: number; band: number },
): boolean {
  // Keep a dead band when reversing scroll, including browser scrollTop
  // clamps as the growing list takes back the art's space.
  if (folded) return list.scrollTop > space.expandThreshold;
  if (list.scrollTop <= space.threshold) return false;
  return list.scrollHeight - list.clientHeight > space.threshold + space.band;
}
