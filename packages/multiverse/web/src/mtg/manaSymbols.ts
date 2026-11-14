// Scryfall writes a mana cost as a run of braced symbols: "{3}{W}{U}", with
// "{B/G}" for hybrid, "{W/P}" for Phyrexian and " // " between the faces of a
// two-faced card. Symbol notation: https://scryfall.com/docs/api/card-symbols
// This file parses that notation and names Scryfall's SVG for a symbol;
// drawing is ManaSymbol's job.

export const COLOR_NAMES: Record<string, string> = {
  W: "white",
  U: "blue",
  B: "black",
  R: "red",
  G: "green",
  C: "colourless",
};

/** Colour letters in the order a colour identity is always shown. */
export const WUBRG = "WUBRG";

const OTHER_NAMES: Record<string, string> = {
  S: "snow",
  T: "tap",
  Q: "untap",
  E: "energy",
};

export type SymbolShape =
  | { kind: "colour"; letter: string }
  | { kind: "colourless" }
  | { kind: "generic"; text: string }
  | { kind: "other"; letter: string }
  | { kind: "hybrid"; halves: [string, string] };

export interface ParsedSymbol {
  shape: SymbolShape;
  /** Payable with 2 life instead of mana (Comprehensive Rules 107.4f). */
  phyrexian: boolean;
  name: string;
}

function halfName(half: string): string {
  if (half in COLOR_NAMES) return COLOR_NAMES[half];
  if (/^\d+$/.test(half)) return `generic ${half}`;
  return OTHER_NAMES[half] ?? half;
}

/** One symbol from inside the braces, such as "U", "3", "B/G", "2/B" or "W/P". */
export function parseSymbol(raw: string): ParsedSymbol {
  const parts = raw.toUpperCase().split("/");
  const phyrexian = parts.length > 1 && parts.includes("P");
  const halves = parts.filter((p) => p !== "P");
  const phyName = (base: string) => (phyrexian ? `${base} or 2 life` : base);

  if (halves.length >= 2) {
    const pair: [string, string] = [halves[0], halves[1]];
    return {
      shape: { kind: "hybrid", halves: pair },
      phyrexian,
      name: phyName(`${halfName(pair[0])} or ${halfName(pair[1])}`),
    };
  }

  const only = halves[0] ?? raw.toUpperCase();
  let shape: SymbolShape;
  if (WUBRG.includes(only) && only.length === 1) shape = { kind: "colour", letter: only };
  else if (only === "C") shape = { kind: "colourless" };
  else if (only in OTHER_NAMES) shape = { kind: "other", letter: only };
  else shape = { kind: "generic", text: only };
  return { shape, phyrexian, name: phyName(halfName(only)) };
}

// Scryfall's own drawing of each symbol. The Card Symbols API
// (https://scryfall.com/docs/api/card-symbols, GET /symbology) gives every
// symbol an `svg_uri` on its CDN. Checked against that list on 2026-10-29:
// the file is the symbol with its slashes removed ("{W/U/P}" → WUP.svg,
// "{2/W}" → 2W.svg), except "½" → HALF.svg and "∞" → INFINITY.svg. A hybrid
// pair exists only in Scryfall's order, where the second colour is one or two
// steps after the first around WUBRG (W/U, W/B, … G/W, G/U; never U/W).
// {H} (one coloured mana or two life) is left out on purpose: its SVG is a
// bare black Phyrexian glyph with no disc, invisible on the black canvas.
// test/web/mana-symbols.test.ts checks this against real /symbology entries.
const SYMBOL_SVG_BASE = "https://svgs.scryfall.io/card-symbols/";

/** In Scryfall's order: the second colour follows the first by one or two steps of WUBRG. */
function isHybridPair(a: string, b: string): boolean {
  const i = WUBRG.indexOf(a);
  const j = WUBRG.indexOf(b);
  if (a.length !== 1 || b.length !== 1 || i < 0 || j < 0) return false;
  const step = (j - i + 5) % 5;
  return step === 1 || step === 2;
}

/**
 * Scryfall's SVG for one symbol from inside the braces, or null when the
 * symbol is not one of the mana, tap or energy symbols it draws (the symbol
 * then keeps its plain disc).
 */
export function symbolSvgUrl(raw: string): string | null {
  const parts = raw.toUpperCase().split("/");
  const colour = (s: string) => s.length === 1 && WUBRG.includes(s);
  let file: string | null = null;
  if (parts.length === 1) {
    const [s] = parts;
    if (s === "½") file = "HALF";
    else if (s === "∞") file = "INFINITY";
    else if (/^(?:1?\d|20|100|1000000)$/.test(s) || /^[WUBRGCSXYZTQE]$/.test(s)) file = s;
  } else if (parts.length === 2) {
    const [a, b] = parts;
    if (isHybridPair(a, b) || ((a === "C" || a === "2") && colour(b)) || ((colour(a) || a === "C") && b === "P")) file = a + b;
  } else if (parts.length === 3) {
    const [a, b, p] = parts;
    if (isHybridPair(a, b) && p === "P") file = a + b + p;
  }
  return file === null ? null : `${SYMBOL_SVG_BASE}${file}.svg`;
}

/** The faces of a cost, each a list of the symbols inside its braces. Empty faces are dropped. */
export function costFaces(cost: string | null | undefined): string[][] {
  if (!cost) return [];
  return cost
    .split(" // ")
    .map((face) => Array.from(face.matchAll(/\{([^}]+)\}/g), (m) => m[1]))
    .filter((symbols) => symbols.length > 0);
}
