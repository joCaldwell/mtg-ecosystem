// Scryfall writes a mana cost as a run of braced symbols: "{3}{W}{U}", with
// "{B/G}" for hybrid, "{W/P}" for Phyrexian and " // " between the faces of a
// two-faced card. Symbol notation: https://scryfall.com/docs/api/card-symbols
// This file only parses that notation; drawing is ManaSymbol's job.

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

/** The faces of a cost, each a list of the symbols inside its braces. Empty faces are dropped. */
export function costFaces(cost: string | null | undefined): string[][] {
  if (!cost) return [];
  return cost
    .split(" // ")
    .map((face) => Array.from(face.matchAll(/\{([^}]+)\}/g), (m) => m[1]))
    .filter((symbols) => symbols.length > 0);
}
