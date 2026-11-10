// UI focus is an expiring, per-user lease, not MCP session state. A hidden
// tab stops renewing but remains useful as last-viewed context for an agent.
import { requireDeckOwner, requireUi, type Actor } from "./actor.ts";
import type { Database } from "./sqlite.ts";
import { getSetting, setSetting } from "./db.ts";
import { ServiceError } from "./errors.ts";
const STALE_MS = 3 * 60_000;
const EXPIRE_MS = 30 * 60_000;
export interface Focus {
  deck_id: number;
  deck_name: string;
  selected: string[];
  age_minutes: number;
  stale: boolean;
  reported_at: number;
  expires_at: number;
}
export interface FocusReport { tab_id: string; sequence: number; claim?: boolean }
interface StoredFocus { deck_id: number | null; selected: string[]; at: number; tab_id?: string; sequence?: number }
function stored(db: Database, actor: Actor): StoredFocus | null {
  try { return JSON.parse(getSetting(db, actor, "focus") ?? "null"); } catch { return null; }
}

/** Legacy callers can report without a token, but cannot clear/steal a leased tab. */
export function setFocus(db: Database, actor: Actor, deckId: number | null, selected: string[] = [], report?: FocusReport): boolean {
  requireUi(actor, "report the deck in agent context");
  if (deckId != null) requireDeckOwner(db, actor, deckId);
  const old = stored(db, actor), now = Date.now();
  if (report) {
    if (typeof report.tab_id !== "string" || !report.tab_id || report.tab_id.length > 128 || !Number.isSafeInteger(report.sequence) || report.sequence < 0 || (report.claim !== undefined && typeof report.claim !== "boolean"))
      throw new ServiceError("Focus needs a tab ID and a nonnegative sequence");
    const own = old?.tab_id === report.tab_id;
    if (own && report.sequence <= (old.sequence ?? -1)) return false;
    if (deckId === null && !own) return false;
    const occupied = old?.deck_id != null && now - old.at < EXPIRE_MS;
    if (deckId !== null && !report.claim && occupied && !own) return false;
    // A late heartbeat cannot resurrect a tab after its clear.
    if (deckId !== null && !report.claim && own && old.deck_id === null) return false;
  } else if (old?.tab_id && old.deck_id !== null && now - old.at < EXPIRE_MS) return false;
  setSetting(db, actor, "focus", JSON.stringify({ deck_id: deckId, selected: selected.slice(0, 50), at: now, ...report }));
  return true;
}

export function getFocus(db: Database, actor: Actor): Focus | null {
  const parsed = stored(db, actor);
  if (parsed?.deck_id == null || !Number.isFinite(parsed.at) || Date.now() - parsed.at >= EXPIRE_MS) return null;
  const deck = db.prepare("SELECT id, name FROM decks WHERE id=? AND user_id=?").get(parsed.deck_id, actor.user_id) as { id: number; name: string } | undefined;
  if (!deck) return null;
  const nameOf = db.prepare("SELECT name FROM cards WHERE oracle_id=?");
  return { deck_id: deck.id, deck_name: deck.name,
    selected: (Array.isArray(parsed.selected) ? parsed.selected : []).map(id => (nameOf.get(id) as { name: string } | undefined)?.name).filter((n): n is string => !!n),
    age_minutes: Math.max(0, Math.round((Date.now() - parsed.at) / 60_000)),
    stale: Date.now() - parsed.at >= STALE_MS, reported_at: parsed.at, expires_at: parsed.at + EXPIRE_MS,
  };
}
