// Sharing (spec §18, v3 D8): who may read a deck through its link. The owner
// decides, in the app: publishing is outward-facing, so no agent can do it.
// What a viewer then sees is publicDeckState() in state.ts.

import { randomInt } from "node:crypto";
import type { Database } from "./sqlite.ts";
import { type Actor, requireDeckOwner, requireUi } from "./actor.ts";
import { withTransaction } from "./db.ts";
import { isUniqueViolation, ServiceError } from "./errors.ts";
import type { DeckCategory, Visibility } from "./deck/service.ts";

const VISIBILITIES: readonly Visibility[] = ["private", "unlisted", "public"];
const BASE62 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
export const SLUG_LENGTH = 10;
/** A slug as the app issues it; anything else is not a link to a deck. */
export const SLUG_PATTERN = /^[0-9A-Za-z]{10}$/;

/** 10 base62 characters, about 59.5 bits from the CSPRNG. */
export function newSlug(): string {
  let slug = "";
  for (let i = 0; i < SLUG_LENGTH; i++) slug += BASE62[randomInt(BASE62.length)];
  return slug;
}

/** A pasted slug, `/d/<slug>` path or full link, reduced to the slug; null if it is none. */
export function slugFrom(text: string): string | null {
  // As pasted: "<https://host/d/AbC…>", or ending a sentence with a full stop.
  const cleaned = text.trim().replace(/^<(.*)>$/, "$1").replace(/[.,;)]+$/, "");
  if (/^[0-9A-Za-z]{10}$/.test(cleaned)) return cleaned;
  // A link or path must be a deck link: /d/<slug>, nothing else.
  const match = cleaned.match(/^(?:https?:\/\/[^/\s]+)?\/d\/([0-9A-Za-z]{10})\/?(?:[?#]\S*)?$/i);
  return match ? match[1] : null;
}

export interface Sharing {
  visibility: Visibility;
  slug: string | null;
  publish_decisions: boolean;
}

/** Owner, in the app (§18.2). Works on Legacy decks: sharing is not an edit.
 * The slug is issued the first time the deck leaves private and never changes. */
export function setVisibility(db: Database, actor: Actor, deckId: number, visibility: Visibility): Sharing {
  const deck = requireDeckOwner(db, actor, deckId, "change who can see this deck");
  requireUi(actor, "change who can see a deck");
  if (!VISIBILITIES.includes(visibility))
    throw new ServiceError("Visibility must be 'private', 'unlisted' or 'public'");
  return withTransaction(db, () => {
    if (visibility !== "private" && !deck.slug) {
      for (let attempt = 0; ; attempt++) {
        try {
          db.prepare("UPDATE decks SET slug = ? WHERE id = ? AND slug IS NULL").run(newSlug(), deckId);
          break;
        } catch (e) {
          if (!isUniqueViolation(e) || attempt >= 5) throw e;
        }
      }
    }
    db.prepare("UPDATE decks SET visibility = ? WHERE id = ?").run(visibility, deckId);
    return sharingOf(db, actor, deckId);
  });
}

/** Owner, in the app. Publishing the decisions log is off by default (D8). */
export function setPublishDecisions(db: Database, actor: Actor, deckId: number, publish: boolean): Sharing {
  requireDeckOwner(db, actor, deckId, "change what this deck publishes");
  requireUi(actor, "change what a deck publishes");
  if (typeof publish !== "boolean") throw new ServiceError("publish_decisions must be true or false");
  db.prepare("UPDATE decks SET publish_decisions = ? WHERE id = ?").run(publish ? 1 : 0, deckId);
  return sharingOf(db, actor, deckId);
}

export function sharingOf(db: Database, actor: Actor, deckId: number): Sharing {
  const deck = requireDeckOwner(db, actor, deckId);
  return { visibility: deck.visibility, slug: deck.slug, publish_decisions: !!deck.publish_decisions };
}

export interface PublicProfile {
  handle: string;
  display_name: string;
  decks: Array<{
    slug: string;
    name: string;
    category: DeckCategory;
    color_identity: string;
    commanders: string[];
    card_count: number;
    parent: { owner_handle: string; slug: string | null; name: string } | null;
  }>;
}

/** A fork's parent as a viewer may see it (§18.4), looked up live: linked
 * while the parent is public, named without its link while unlisted (the
 * link is its owner's to give out), and absent once private or gone. The
 * forker's own deck keeps the full snapshot. */
export function publicParent(db: Database, snapshot: { owner_handle: string; slug: string; name: string } | null): { owner_handle: string; slug: string | null; name: string } | null {
  if (!snapshot) return null;
  const parent = db.prepare("SELECT d.name, d.visibility, u.handle FROM decks d JOIN users u ON u.id = d.user_id WHERE d.slug = ? AND u.deleted_at IS NULL")
    .get(snapshot.slug) as { name: string; visibility: string; handle: string } | undefined;
  if (!parent || parent.visibility === "private") return null;
  return { owner_handle: parent.handle, slug: parent.visibility === "public" ? snapshot.slug : null, name: parent.name };
}

/** `/u/<handle>` (§18.5): the display name and public decks, nothing else.
 * Unlisted decks are reachable only by their link, so they are not listed,
 * and a person with no public deck has no public page: their display name
 * (often a provider's real name) is not shown to anyone who guesses a handle. */
export function publicProfile(db: Database, _viewer: Actor | null, handle: string): PublicProfile {
  const user = db.prepare(`SELECT u.id, u.handle, u.display_name FROM users u WHERE u.handle = ? AND u.deleted_at IS NULL AND u.id > 0
      AND EXISTS (SELECT 1 FROM decks d WHERE d.user_id = u.id AND d.visibility = 'public' AND d.slug IS NOT NULL)`)
    .get(String(handle)) as { id: number; handle: string; display_name: string } | undefined;
  if (!user) throw new ServiceError("There is no one with that handle, or they have no public decks", 404);
  const decks = db.prepare(
    `SELECT d.id, d.slug, d.name, d.category, d.color_identity, d.parent_owner_handle, d.parent_slug, d.parent_name,
            COALESCE((SELECT SUM(quantity) FROM deck_cards dc WHERE dc.deck_id = d.id AND dc.role != 'companion' AND dc.maybe = 0), 0) AS card_count
     FROM decks d WHERE d.user_id = ? AND d.visibility = 'public' AND d.slug IS NOT NULL ORDER BY d.name`,
  ).all(user.id) as unknown as Array<{ id: number; slug: string; name: string; category: DeckCategory; color_identity: string; card_count: number; parent_owner_handle: string | null; parent_slug: string | null; parent_name: string | null }>;
  const commanders = db.prepare(
    `SELECT c.name FROM deck_cards dc JOIN cards c ON c.oracle_id = dc.oracle_id
     WHERE dc.deck_id = ? AND dc.role = 'commander' AND dc.maybe = 0 ORDER BY c.name`,
  );
  return {
    handle: user.handle,
    display_name: user.display_name,
    decks: decks.map(d => ({
      slug: d.slug, name: d.name, category: d.category, color_identity: d.color_identity, card_count: d.card_count,
      commanders: (commanders.all(d.id) as unknown as { name: string }[]).map(c => c.name),
      parent: publicParent(db, d.parent_slug && d.parent_owner_handle ? { owner_handle: d.parent_owner_handle, slug: d.parent_slug, name: d.parent_name ?? "" } : null),
    })),
  };
}

/** How many of the principal's decks are shared, for start_session (§18.7). */
export function sharedCounts(db: Database, actor: Actor): { public: number; unlisted: number } {
  const row = db.prepare(
    "SELECT SUM(visibility = 'public') AS public, SUM(visibility = 'unlisted') AS unlisted FROM decks WHERE user_id = ?",
  ).get(actor.user_id) as { public: number | null; unlisted: number | null };
  return { public: row.public ?? 0, unlisted: row.unlisted ?? 0 };
}
