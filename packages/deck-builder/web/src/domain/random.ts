/** Inclusive integer ranges for the playtester's random number roller. Rolls
 * are table aids outside game state: they are not saved and game undo never
 * reverses them. */
export const RANDOM_BOUND_LIMIT = 1_000_000_000;
export interface RandomRange {
  min: number;
  max: number;
}

export function parseRange(
  lower: string,
  upper: string,
): RandomRange | { error: string } {
  const min = parseBound(lower),
    max = parseBound(upper);
  if (min === null || max === null)
    return {
      error: `Enter whole numbers between ${(-RANDOM_BOUND_LIMIT).toLocaleString()} and ${RANDOM_BOUND_LIMIT.toLocaleString()}`,
    };
  if (min > max)
    return { error: "The lower bound must not exceed the upper bound" };
  return { min, max };
}

function parseBound(raw: string): number | null {
  const text = raw.trim();
  if (!/^-?\d+$/.test(text)) return null;
  const n = Number(text);
  return Math.abs(n) <= RANDOM_BOUND_LIMIT ? n : null;
}

/** A uniform integer in [min, max]. Rejection sampling removes the modulo bias
 * a plain `x % span` has whenever span does not divide 2^32; the bound limit
 * keeps span within one 32-bit draw. */
export function rollInRange(
  { min, max }: RandomRange,
  next: () => number = randomUint32,
): number {
  const span = max - min + 1,
    limit = 2 ** 32 - (2 ** 32 % span);
  let x = next();
  while (x >= limit) x = next();
  return min + (x % span);
}

function randomUint32(): number {
  return crypto.getRandomValues(new Uint32Array(1))[0];
}
