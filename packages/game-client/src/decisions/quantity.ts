/** Input syntax only. maxDigits is an explicitly supplied input-size limit, not a
 * derived Magic cost/range. Exact decimal arithmetic never uses Number. */
export function parseQuantity(input: string, maxDigits: number): bigint | null {
  if (!Number.isSafeInteger(maxDigits) || maxDigits < 1) throw new Error('Invalid supplied input bound');
  // JS `$` can match before a final newline; require the entire matched input.
  if (input.length > maxDigits || /^(0|[1-9][0-9]*)$/.exec(input)?.[0] !== input) return null;
  return BigInt(input);
}
