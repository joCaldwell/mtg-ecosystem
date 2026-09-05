// Supported mana symbols: CR 107.4 and Scryfall symbology (checked 2026-09-05).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt
// https://api.scryfall.com/symbology
// Exotic/fractional symbols are deliberately unsupported. Generic brace
// tokenization does not imply that the contents are valid mana.
const COLORED_HYBRIDS = new Set([
  "W/U", "W/B", "U/B", "U/R", "B/R", "B/G", "R/G", "R/W", "G/W", "G/U",
]);
export function isManaSymbol(value: string): boolean {
  if (/^([WUBRGCSXYZ]|0|[1-9][0-9]*|[2C]\/[WUBRG]|[WUBRGC]\/P)$/.test(value)) return true;
  return COLORED_HYBRIDS.has(value) ||
    (value.endsWith("/P") && COLORED_HYBRIDS.has(value.slice(0, -2)));
}
