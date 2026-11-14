// Slot targets are typed as text. Empty is "no limit on this side"; the same
// whole number in both fields means exactly that many.

export type ParsedTarget = { ok: true; value: number | null } | { ok: false };

export function parseTarget(text: string): ParsedTarget {
  const t = text.trim();
  if (t === "") return { ok: true, value: null };
  return /^\d+$/.test(t) ? { ok: true, value: Number(t) } : { ok: false };
}

export const TARGET_ERROR = "A slot target must be a whole number, zero or more.";

export function targetText(n: number | null): string {
  return n == null ? "" : String(n);
}
