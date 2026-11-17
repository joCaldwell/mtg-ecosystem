/** Bounded to the product's one-minute account request window. */
export const MAX_READ_RETRY_MS = 60_000;

// RFC 9110 §10.2.3: delay-seconds or HTTP-date.
// https://www.rfc-editor.org/rfc/rfc9110.html#section-10.2.3
export function retryAfterMilliseconds(header: string | null, now = Date.now()): number | null {
  if (header === null || header.length > 128) return null;
  const value = header.trim();
  const delay = /^\d+$/.test(value) ? Number(value) * 1000
    : /[a-z]/i.test(value) ? Date.parse(value) - now : NaN;
  if (Number.isNaN(delay)) return null;
  return Math.min(MAX_READ_RETRY_MS, Math.max(0, delay));
}
