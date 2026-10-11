/**
 * A size token from design/tokens.css as a number of pixels, for the few
 * places script has to do arithmetic with one. The stylesheet stays the only
 * place the number is written.
 */
export function tokenPx(name: `--${string}`): number {
  return parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name));
}
