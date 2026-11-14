// Server timestamps and how they read. SQLite's datetime('now') writes
// "YYYY-MM-DD HH:MM:SS" in UTC with no zone marker, so a bare string must be
// read as UTC, not as local time.

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const NO_ZONE = /^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?$/;

/** Milliseconds since the epoch, or NaN if unreadable. Also accepts ISO strings with a zone. */
export function parseServerTime(iso: string): number {
  const s = iso.trim();
  if (NO_ZONE.test(s)) return Date.parse(s.replace(" ", "T") + "Z");
  return Date.parse(s);
}

function shortDate(t: number, now: number): string {
  const d = new Date(t);
  const base = `${d.getDate()} ${MONTHS[d.getMonth()]}`;
  return d.getFullYear() === new Date(now).getFullYear() ? base : `${base} ${d.getFullYear()}`;
}

/** "just now" under a minute, "12 min ago", "3 h ago", "yesterday", "6 days ago", then a short date. */
export function ago(iso: string, now: number = Date.now()): string {
  const t = parseServerTime(iso);
  if (Number.isNaN(t)) return iso;
  const secs = Math.floor((now - t) / 1000);
  if (secs < 60) return "just now"; // includes clock skew putting it slightly ahead
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "yesterday";
  if (days < 7) return `${days} days ago`;
  return shortDate(t, now);
}

/** Local "16:42". */
export function clockTime(iso: string): string {
  const t = parseServerTime(iso);
  if (Number.isNaN(t)) return "";
  const d = new Date(t);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

function dayStart(t: number): number {
  const d = new Date(t);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** "Today", "Yesterday", or a short date, by local calendar day. */
export function dayLabel(iso: string, now: number = Date.now()): string {
  const t = parseServerTime(iso);
  if (Number.isNaN(t)) return iso;
  // Round: a calendar day is 23 or 25 hours across a clock change.
  const days = Math.round((dayStart(now) - dayStart(t)) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  return shortDate(t, now);
}
