// Slot targets are typed as text. Empty is "no limit on this side"; the same
// whole number in both fields means exactly that many.

type ParsedTarget = { ok: true; value: number | null } | { ok: false };

function parseTarget(text: string): ParsedTarget {
  const t = text.trim();
  if (t === "") return { ok: true, value: null };
  const value = Number(t);
  return /^\d+$/.test(t) && Number.isInteger(value) ? { ok: true, value } : { ok: false };
}

const TARGET_ERROR = "Enter a whole number of zero or more.";

export function parseTargets(min: string, max: string) {
  const lower = parseTarget(min);
  const upper = parseTarget(max);
  if (!lower.ok || !upper.ok)
    return { ok: false as const, minError: lower.ok ? undefined : TARGET_ERROR, maxError: upper.ok ? undefined : TARGET_ERROR };
  if (lower.value != null && upper.value != null && lower.value > upper.value)
    return { ok: false as const, minError: "Min must be at most Max.", maxError: undefined };
  return { ok: true as const, min: lower.value, max: upper.value };
}

export function targetText(n: number | null): string {
  return n == null ? "" : String(n);
}
