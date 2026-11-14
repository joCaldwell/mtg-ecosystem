// Public/bootstrap reads do not pull the editor's API methods into a visit.
import { request } from "./transport.ts";
import type { Session, CardRow, DeckSummary, PublicDeckState, PublicProfile } from "./client.ts";
export { ApiError } from "./transport.ts";
export const api = {
  session: () => request<Session>("GET", "/api/session"),
  resolveCard: (name: string) => request<{ matches: CardRow[]; suggestions: string[] }>("GET", `/api/resolve?${new URLSearchParams({ name })}`),
  publicDeck: (slug: string) => request<{ deck: PublicDeckState }>("GET", `/api/public/decks/${encodeURIComponent(slug)}`),
  /** The reader's own decks; a shared page uses it only to link its owner back. */
  listDecks: () => request<DeckSummary[]>("GET", "/api/decks"),
  publicProfile: (handle: string) => request<{ profile: PublicProfile }>("GET", `/api/public/users/${encodeURIComponent(handle)}`),
};
